//! Direct peer download manager.
//!
//! A download fetches a bounded manifest from one verified source, then streams
//! one 1 MiB chunk at a time, verifying every chunk hash before writing it to
//! the managed partial file. Completed chunks advance a durable verified prefix,
//! so a pause, process restart, or source failure resumes from verified bytes
//! rather than from zero.
//!
//! The manager never trusts a source: metadata is checked against the signed
//! publication (or the requested content ID), chunk bytes are checked against
//! the manifest, and the promoted file is checked against the content ID before
//! it enters the library.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use nexus_crypto::Value;
use nexus_net::{FileMetadata, FileProvider, NetError, Network};
use nexus_protocol::{CHUNK_SIZE_V1, FileSelector, ManifestV1, SignedPublication, manifest_root};
use nexus_store::{ChunkMap, ContentState, DownloadRecord, DownloadState, Store, StoreError, safe_display_name};
use nexus_transfer::scheduler::{
    LeaseDecision, MAX_PARALLEL_SOURCES, MultiSourceScheduler, ScheduleFailure, SourceFailure, SourceStats,
};
use sha2::{Digest, Sha256};

use crate::engine::{Engine, MethodOutcome};
use crate::envelope::error_value;
use crate::error::{CoreError, DomainCode, DomainError};
use crate::event::{EventQueue, PendingEvent};
use crate::hosting::HostingState;
use crate::network::domain_error as network_domain_error;
use crate::safety::safety_value;

/// Number of worker threads dedicated to downloads.
pub const DOWNLOAD_WORKERS: usize = 2;
/// Maximum number of sources accepted in one request.
pub const MAX_SOURCES: usize = 8;
/// Rounds over the source list when fetching metadata.
const METADATA_ROUNDS: usize = 2;
/// Bounded backoff between metadata attempts.
const RETRY_DELAY: Duration = Duration::from_millis(150);
/// Minimum interval between progress events.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);
/// Maximum generated download ID length.
const MAX_DOWNLOAD_ID_BYTES: usize = 128;

/// Storage-backed source for inbound `/nexus/file/1` requests.
pub(crate) struct CoreFileProvider {
    store: Arc<Store>,
    hosting: Arc<HostingState>,
}

impl CoreFileProvider {
    pub(crate) fn new(store: Arc<Store>, hosting: Arc<HostingState>) -> Self {
        Self { store, hosting }
    }

    fn serveable(&self, content_id: &str, expected_size: u64) -> bool {
        self.hosting.serving_allowed()
            && self.store.content(content_id).is_ok_and(|record| {
                record.state == ContentState::Verified
                    && record.size_bytes == expected_size
                    && self.store.content_serveable(content_id).unwrap_or(false)
            })
    }

    fn record_served(&self, content_id: &str) {
        let _ = self.store.record_replica_seen(content_id, now_secs());
    }

    fn metadata_for_publication(&self, publication_id: &str) -> Option<FileMetadata> {
        let record = self.store.publication(publication_id).ok()?;
        let signed = SignedPublication::parse(&record.record_bytes).ok()?;
        if hex_lower(&signed.publication_id()) != publication_id {
            return None;
        }
        let root = hex_lower(&signed.publication().manifest_root);
        let manifest = self.store.manifest(&root).ok()?;
        let parsed = ManifestV1::parse_body(&manifest).ok()?;
        if parsed.validate_for_publication(signed.publication()).is_err() {
            return None;
        }
        if !self.serveable(&hex_lower(&parsed.content_id), parsed.total_size) {
            return None;
        }
        Some(FileMetadata {
            manifest,
            publication: Some(record.record_bytes),
        })
    }

    fn metadata_for_content(&self, content_id: &str) -> Option<FileMetadata> {
        let manifest = self.store.manifest_for_content(content_id).ok()?;
        let parsed = ManifestV1::parse_body(&manifest).ok()?;
        if hex_lower(&parsed.content_id) != content_id || !self.serveable(content_id, parsed.total_size) {
            return None;
        }
        let publication = self
            .store
            .publication_for_content(content_id)
            .ok()
            .flatten()
            .filter(|bytes| {
                SignedPublication::parse(bytes).is_ok_and(|signed| {
                    signed.publication().content_id == parsed.content_id
                        && parsed.validate_for_publication(signed.publication()).is_ok()
                })
            });
        Some(FileMetadata { manifest, publication })
    }
}

impl FileProvider for CoreFileProvider {
    fn metadata(&self, selector: FileSelector) -> Option<FileMetadata> {
        let id = hex_lower(&selector.id());
        let metadata = match selector {
            FileSelector::ContentId(_) => self.metadata_for_content(&id),
            FileSelector::PublicationId(_) => self.metadata_for_publication(&id),
        };
        if let Some(metadata) = &metadata
            && let Ok(parsed) = ManifestV1::parse_body(&metadata.manifest)
        {
            self.record_served(&hex_lower(&parsed.content_id));
        }
        metadata
    }

    fn chunk(&self, content_id: &[u8; 32], index: u32, offset: u32, length: u32) -> Option<Vec<u8>> {
        let content_id_text = hex_lower(content_id);
        let manifest = self.store.manifest_for_content(&content_id_text).ok()?;
        let parsed = ManifestV1::parse_body(&manifest).ok()?;
        if parsed.content_id != *content_id {
            return None;
        }
        let entry = parsed.chunks.get(usize::try_from(index).ok()?)?;
        let end = u64::from(offset).checked_add(u64::from(length))?;
        if length == 0 || end > entry.length {
            return None;
        }
        if !self.serveable(&content_id_text, parsed.total_size) {
            return None;
        }
        let chunk_start = u64::from(index).checked_mul(CHUNK_SIZE_V1)?;
        let absolute = chunk_start.checked_add(u64::from(offset))?;
        let data = self
            .store
            .read_content_slice(&content_id_text, absolute, usize::try_from(length).ok()?)
            .ok()?;
        self.record_served(&content_id_text);
        Some(data)
    }
}

/// Transport used by the download manager.
///
/// The production implementation forwards to the running network driver; tests
/// substitute a local transport to inject corruption and interruptions.
pub(crate) trait DownloadTransport: Send + Sync {
    fn metadata(&self, peer_id: &str, selector: FileSelector) -> Result<FileMetadata, NetError>;
    fn chunk(
        &self,
        peer_id: &str,
        content_id: &[u8; 32],
        index: u32,
        offset: u32,
        length: u32,
    ) -> Result<Vec<u8>, NetError>;
}

struct NetworkTransport {
    network: Arc<Network>,
}

impl DownloadTransport for NetworkTransport {
    fn metadata(&self, peer_id: &str, selector: FileSelector) -> Result<FileMetadata, NetError> {
        self.network.fetch_file_metadata(peer_id, selector)
    }

    fn chunk(
        &self,
        peer_id: &str,
        content_id: &[u8; 32],
        index: u32,
        offset: u32,
        length: u32,
    ) -> Result<Vec<u8>, NetError> {
        self.network
            .fetch_file_chunk(peer_id, content_id, index, offset, length)
    }
}

/// Shared worker context; deliberately does not borrow the engine.
pub(crate) struct DownloadContext {
    pub store: Arc<Store>,
    pub events: Arc<EventQueue>,
    pub transport: Arc<dyn DownloadTransport>,
}

/// One download job submitted to the download pool.
pub(crate) struct DownloadJob {
    pub download_id: String,
    pub selector: Option<FileSelector>,
    pub expected_publication: Option<String>,
    pub expected_content: Option<String>,
    pub sources: Vec<String>,
    pub resume: bool,
}

/// The user-visible controls of a running download.
pub(crate) struct DownloadControl {
    state: Mutex<ControlState>,
    changed: Condvar,
}

struct ControlState {
    pause: bool,
    cancel: Option<bool>,
    finished: bool,
}

enum ControlAction {
    Continue,
    Pause,
    Cancel(bool),
}

impl DownloadControl {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(ControlState {
                pause: false,
                cancel: None,
                finished: false,
            }),
            changed: Condvar::new(),
        }
    }

    pub(crate) fn request_pause(&self) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.finished || state.cancel.is_some() {
            return false;
        }
        state.pause = true;
        self.changed.notify_all();
        true
    }

    pub(crate) fn request_cancel(&self, keep_partial: bool) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.finished {
            return false;
        }
        state.cancel = Some(keep_partial);
        state.pause = false;
        self.changed.notify_all();
        true
    }

    fn action(&self) -> ControlAction {
        let state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(keep) = state.cancel {
            ControlAction::Cancel(keep)
        } else if state.pause {
            ControlAction::Pause
        } else {
            ControlAction::Continue
        }
    }

    fn wait(&self, duration: Duration) -> ControlAction {
        let state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let (state, _) = self
            .changed
            .wait_timeout_while(state, duration, |state| {
                !state.pause && state.cancel.is_none() && !state.finished
            })
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(keep) = state.cancel {
            ControlAction::Cancel(keep)
        } else if state.pause {
            ControlAction::Pause
        } else {
            ControlAction::Continue
        }
    }

    pub(crate) fn finish(&self) {
        let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        state.finished = true;
        self.changed.notify_all();
    }
}

impl Default for DownloadControl {
    fn default() -> Self {
        Self::new()
    }
}

enum TransferError {
    Failed(DomainError),
    Paused(&'static str),
    Cancelled(bool),
}

enum FetchError {
    Net(NetError),
    Stopped(TransferError),
}

struct VerifiedMetadata {
    manifest: ManifestV1,
    manifest_bytes: Vec<u8>,
    manifest_root: String,
    content_id: String,
    publication_id: Option<String>,
    display_name: String,
    source_peer: String,
    chunk_map: ChunkMap,
}

struct Progress {
    verified: u64,
    total: u64,
    source_peer: Option<String>,
    started: Instant,
    start_verified: u64,
    last_emit: Instant,
}

impl Progress {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            verified: 0,
            total: 0,
            source_peer: None,
            started: now,
            start_verified: 0,
            last_emit: now,
        }
    }

    fn rate_bps(&self) -> u64 {
        let elapsed = self.started.elapsed().as_secs_f64();
        if elapsed <= 0.0 {
            return 0;
        }
        let bytes = self.verified.saturating_sub(self.start_verified);
        (bytes as f64 / elapsed) as u64
    }
}

/// Shared state for the workers of one download.
struct TransferShared {
    scheduler: Mutex<MultiSourceScheduler>,
    work: Condvar,
    stop: Mutex<Option<TransferError>>,
    progress: Mutex<Progress>,
    last_failure: Mutex<Option<DomainError>>,
    abort: AtomicBool,
}

impl TransferShared {
    fn new(scheduler: MultiSourceScheduler, progress: Progress) -> Self {
        Self {
            scheduler: Mutex::new(scheduler),
            work: Condvar::new(),
            stop: Mutex::new(None),
            progress: Mutex::new(progress),
            last_failure: Mutex::new(None),
            abort: AtomicBool::new(false),
        }
    }

    fn request_stop(&self, error: TransferError) {
        let mut slot = lock(&self.stop);
        if slot.is_none() {
            *slot = Some(error);
            self.abort.store(true, Ordering::SeqCst);
            self.work.notify_all();
        }
    }

    fn is_aborted(&self) -> bool {
        self.abort.load(Ordering::SeqCst)
    }

    fn take_stop(&self) -> Option<TransferError> {
        lock(&self.stop).take()
    }

    fn record_net_failure(&self, error: &NetError) {
        *lock(&self.last_failure) = Some(network_domain_error(error));
    }

    fn schedule_failure(&self, failure: ScheduleFailure) -> TransferError {
        let saw_corruption = lock(&self.scheduler).saw_corruption();
        if saw_corruption {
            return verification_error("chunk_hash_mismatch");
        }
        if let Some(error) = lock(&self.last_failure).clone() {
            return TransferError::Failed(error);
        }
        match failure {
            ScheduleFailure::ChunkExhausted(_) | ScheduleFailure::NoUsableSource => {
                TransferError::Failed(map_source_failure(None))
            }
        }
    }

    fn wait_for_work(&self) {
        let scheduler = lock(&self.scheduler);
        let _ = self
            .work
            .wait_timeout_while(scheduler, Duration::from_millis(100), |_| {
                !self.abort.load(Ordering::SeqCst)
            })
            .unwrap_or_else(|poisoned| poisoned.into_inner());
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Runs one download job to a terminal event.
pub(crate) fn run_download(context: DownloadContext, control: Arc<DownloadControl>, job: DownloadJob) {
    let mut progress = Progress::new();
    let result = catch_unwind(AssertUnwindSafe(|| transfer(&context, &control, &job, &mut progress)));
    match result {
        Ok(Ok((content_id, size))) => {
            emit_completed(&context, &job.download_id, &content_id, size);
        }
        Ok(Err(TransferError::Paused(reason))) => {
            finish_pause(&context, &job.download_id, reason, progress.verified);
        }
        Ok(Err(TransferError::Cancelled(keep))) => {
            finish_cancel(&context, &job.download_id, keep);
        }
        Ok(Err(TransferError::Failed(error))) => {
            finish_failure(&context, &job.download_id, &control, &error, progress.verified);
        }
        Err(_) => {
            let error = DomainError::new(DomainCode::Internal, "The download stopped unexpectedly.");
            finish_failure(&context, &job.download_id, &control, &error, progress.verified);
        }
    }
}

/// A failure that raced with a pause or cancel request is reported as the
/// user's action, so the persisted state always matches the terminal event.
fn finish_failure(
    context: &DownloadContext,
    download_id: &str,
    control: &DownloadControl,
    error: &DomainError,
    verified: u64,
) {
    match control.action() {
        ControlAction::Cancel(keep) => finish_cancel(context, download_id, keep),
        ControlAction::Pause => {
            // A pause requested before metadata registration has no row to
            // persist yet. If metadata then fails, report the failure instead
            // of a paused state the user could never resume.
            if !finish_pause(context, download_id, "paused by user", verified) {
                fail_download(context, download_id, error, verified);
            }
        }
        ControlAction::Continue => fail_download(context, download_id, error, verified),
    }
}

fn fail_download(context: &DownloadContext, download_id: &str, error: &DomainError, verified: u64) {
    let _ = context.store.fail_download(download_id, error.code.as_str());
    emit_failure(context, download_id, error, verified);
}

fn finish_pause(context: &DownloadContext, download_id: &str, reason: &str, verified: u64) -> bool {
    if context.store.pause_download(download_id, Some(reason)).is_err() {
        return false;
    }
    let event = PendingEvent::durable(
        "download.paused",
        Value::Map(vec![
            (
                Value::Text("download_id".to_owned()),
                Value::Text(download_id.to_owned()),
            ),
            (Value::Text("verified_bytes".to_owned()), Value::Unsigned(verified)),
            (Value::Text("reason".to_owned()), Value::Text(reason.to_owned())),
        ]),
    )
    .with_download(download_id.to_owned());
    let _ = context.events.push(event);
    true
}

fn finish_cancel(context: &DownloadContext, download_id: &str, keep: bool) {
    if keep {
        let _ = context.store.cancel_download_keep(download_id);
    } else {
        let _ = context.store.cancel_download(download_id);
    }
    let event = PendingEvent::durable(
        "download.cancelled",
        Value::Map(vec![
            (
                Value::Text("download_id".to_owned()),
                Value::Text(download_id.to_owned()),
            ),
            (Value::Text("partial_kept".to_owned()), Value::Bool(keep)),
        ]),
    )
    .with_download(download_id.to_owned());
    let _ = context.events.push(event);
}

fn emit_completed(context: &DownloadContext, download_id: &str, content_id: &str, size: u64) {
    let display_name = context
        .store
        .download(download_id)
        .map_or_else(|_| String::new(), |record| record.display_name);
    let event = PendingEvent::durable(
        "download.completed",
        Value::Map(vec![
            (
                Value::Text("download_id".to_owned()),
                Value::Text(download_id.to_owned()),
            ),
            (Value::Text("content_id".to_owned()), Value::Text(content_id.to_owned())),
            (Value::Text("size".to_owned()), Value::Unsigned(size)),
            (
                Value::Text("display_name".to_owned()),
                Value::Text(display_name.clone()),
            ),
            (Value::Text("safety".to_owned()), safety_value(&display_name)),
        ]),
    )
    .with_download(download_id.to_owned());
    let _ = context.events.push(event);
}

fn transfer(
    context: &DownloadContext,
    control: &DownloadControl,
    job: &DownloadJob,
    progress: &mut Progress,
) -> Result<(String, u64), TransferError> {
    let (metadata, prefix) = prepare(context, control, job)?;
    progress.total = metadata.manifest.total_size;
    progress.verified = prefix;
    progress.start_verified = prefix;
    progress.source_peer = Some(metadata.source_peer.clone());
    if job.resume {
        let event = PendingEvent::durable(
            "download.resumed",
            Value::Map(vec![
                (
                    Value::Text("download_id".to_owned()),
                    Value::Text(job.download_id.clone()),
                ),
                (
                    Value::Text("content_id".to_owned()),
                    Value::Text(metadata.content_id.clone()),
                ),
                (Value::Text("verified_bytes".to_owned()), Value::Unsigned(prefix)),
                (
                    Value::Text("total_bytes".to_owned()),
                    Value::Unsigned(metadata.manifest.total_size),
                ),
                (
                    Value::Text("verified_chunks".to_owned()),
                    Value::Unsigned(u64::from(metadata.chunk_map.completed_count())),
                ),
                (
                    Value::Text("total_chunks".to_owned()),
                    Value::Unsigned(u64::from(metadata.chunk_map.total())),
                ),
                (Value::Text("sources".to_owned()), peer_list_value(&job.sources)),
                (
                    Value::Text("display_name".to_owned()),
                    Value::Text(metadata.display_name.clone()),
                ),
                (Value::Text("safety".to_owned()), safety_value(&metadata.display_name)),
            ]),
        )
        .with_download(job.download_id.clone());
        let _ = context.events.push(event);
    } else {
        let mut data = vec![
            (
                Value::Text("download_id".to_owned()),
                Value::Text(job.download_id.clone()),
            ),
            (
                Value::Text("content_id".to_owned()),
                Value::Text(metadata.content_id.clone()),
            ),
            (
                Value::Text("total_bytes".to_owned()),
                Value::Unsigned(metadata.manifest.total_size),
            ),
            (
                Value::Text("total_chunks".to_owned()),
                Value::Unsigned(u64::from(metadata.chunk_map.total())),
            ),
            (
                Value::Text("source_peer".to_owned()),
                Value::Text(metadata.source_peer.clone()),
            ),
            (Value::Text("sources".to_owned()), peer_list_value(&job.sources)),
        ];
        let publication_id = metadata.publication_id.clone().map_or(Value::Null, Value::Text);
        data.push((Value::Text("publication_id".to_owned()), publication_id));
        data.push((
            Value::Text("display_name".to_owned()),
            Value::Text(metadata.display_name.clone()),
        ));
        data.push((Value::Text("safety".to_owned()), safety_value(&metadata.display_name)));
        let event = PendingEvent::durable("download.started", Value::Map(data)).with_download(job.download_id.clone());
        let _ = context.events.push(event);
    }

    let content = decode_content_id(&metadata.content_id).ok_or_else(|| verification_error("content_id_invalid"))?;
    let size = download_chunks(context, control, job, &metadata, prefix, &content, progress)?;
    Ok((metadata.content_id, size))
}

fn prepare(
    context: &DownloadContext,
    control: &DownloadControl,
    job: &DownloadJob,
) -> Result<(VerifiedMetadata, u64), TransferError> {
    if job.resume {
        return prepare_resume(context, control, job);
    }
    let selector = job
        .selector
        .ok_or_else(|| TransferError::Failed(invalid_argument("a content selector is required")))?;
    let (metadata, source_peer) = fetch_metadata(context, control, job, selector)?;
    let verified = verify_metadata(
        metadata,
        selector,
        job.expected_publication.as_deref(),
        job.expected_content.as_deref(),
        &source_peer,
    )?;
    context
        .store
        .put_manifest(&verified.manifest_root, &verified.manifest_bytes)
        .map_err(store_failure)?;
    let record = context
        .store
        .begin_download_with_source(
            &job.download_id,
            &verified.content_id,
            verified.manifest.total_size,
            &verified.display_name,
            Some(&verified.manifest_root),
            Some(&verified.source_peer),
        )
        .map_err(store_failure)?;
    Ok((verified, record.verified_bytes))
}

fn prepare_resume(
    context: &DownloadContext,
    control: &DownloadControl,
    job: &DownloadJob,
) -> Result<(VerifiedMetadata, u64), TransferError> {
    let record = context.store.download(&job.download_id).map_err(store_failure)?;
    if record.state == DownloadState::Completed {
        return Err(TransferError::Failed(DomainError::new(
            DomainCode::Conflict,
            "The download is already complete.",
        )));
    }
    let (manifest_bytes, manifest_root, source_peer) = if let Some(root) = &record.manifest_root {
        let bytes = context.store.manifest(root).map_err(store_failure)?;
        let source = job
            .sources
            .first()
            .cloned()
            .or_else(|| record.source_peer.clone())
            .ok_or_else(|| TransferError::Failed(invalid_argument("a source peer is required to resume")))?;
        (bytes, root.clone(), source)
    } else {
        let selector = match job.selector {
            Some(selector) => selector,
            None => FileSelector::ContentId(
                decode_content_id(&record.content_id).ok_or_else(|| verification_error("content_id_invalid"))?,
            ),
        };
        let (metadata, source) = fetch_metadata(context, control, job, selector)?;
        let verified = verify_metadata(metadata, selector, None, Some(&record.content_id), &source)?;
        context
            .store
            .put_manifest(&verified.manifest_root, &verified.manifest_bytes)
            .map_err(store_failure)?;
        (verified.manifest_bytes, verified.manifest_root, verified.source_peer)
    };
    let manifest = ManifestV1::parse_body(&manifest_bytes).map_err(|_| verification_error("manifest_invalid"))?;
    if hex_lower(&manifest.content_id) != record.content_id || manifest.total_size != record.expected_size {
        return Err(verification_error("manifest_mismatch"));
    }
    if usize::try_from(record.expected_size.div_ceil(CHUNK_SIZE_V1)).ok() != Some(manifest.chunks.len()) {
        return Err(verification_error("manifest_mismatch"));
    }
    if record.verified_prefix > manifest.total_size {
        return Err(verification_error("resume_prefix_invalid"));
    }
    if record.verified_prefix != manifest.total_size && record.verified_prefix % CHUNK_SIZE_V1 != 0 {
        // A pre-Stage-17 record could only checkpoint whole chunks. A mid-chunk
        // prefix is not resumable without re-verifying and rewriting the chunk.
        return Err(verification_error("resume_alignment"));
    }
    let mut map = record
        .chunk_map
        .as_deref()
        .and_then(|bytes| ChunkMap::from_stored(record.expected_size, bytes))
        .unwrap_or_else(|| ChunkMap::from_prefix(record.expected_size, record.verified_prefix));
    if usize::try_from(map.total()).ok() != Some(manifest.chunks.len()) {
        return Err(verification_error("manifest_mismatch"));
    }

    // Re-verify every chunk the database believes is complete. This catches a
    // corrupted or truncated partial file before any peer is trusted again.
    let partial_path = context.store.partial_path(&job.download_id).map_err(store_failure)?;
    for index in 0..map.total() {
        if !map.is_set(index) {
            continue;
        }
        let entry = manifest
            .chunks
            .get(usize::try_from(index).unwrap_or(usize::MAX))
            .ok_or_else(|| verification_error("chunk_index_invalid"))?;
        if !chunk_matches(&partial_path, index, entry) {
            context
                .store
                .clear_chunk(&job.download_id, index)
                .map_err(store_failure)?;
            map.clear(index);
        }
    }

    context.store.resume_download(&job.download_id).map_err(store_failure)?;
    context
        .store
        .set_download_source(&job.download_id, Some(&manifest_root), Some(&source_peer))
        .map_err(store_failure)?;
    let record = context
        .store
        .trim_unverified_tail(&job.download_id)
        .map_err(store_failure)?;
    let prefix = record.verified_bytes.min(manifest.total_size);
    Ok((
        VerifiedMetadata {
            manifest,
            manifest_bytes,
            manifest_root,
            content_id: record.content_id,
            publication_id: None,
            display_name: record.display_name,
            source_peer,
            chunk_map: map,
        },
        prefix,
    ))
}

fn chunk_matches(path: &std::path::Path, index: u32, entry: &nexus_protocol::ChunkEntry) -> bool {
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    let start = u64::from(index).saturating_mul(CHUNK_SIZE_V1);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return false;
    }
    let Ok(length) = usize::try_from(entry.length) else {
        return false;
    };
    let mut bytes = vec![0_u8; length];
    if file.read_exact(&mut bytes).is_err() {
        return false;
    }
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    digest == entry.sha256
}

fn verify_metadata(
    metadata: FileMetadata,
    selector: FileSelector,
    expected_publication: Option<&str>,
    expected_content: Option<&str>,
    source_peer: &str,
) -> Result<VerifiedMetadata, TransferError> {
    let FileMetadata {
        manifest: manifest_bytes,
        publication,
    } = metadata;
    let manifest = ManifestV1::parse_body(&manifest_bytes).map_err(|_| verification_error("manifest_invalid"))?;
    let canonical = manifest
        .encode_body()
        .map_err(|_| verification_error("manifest_invalid"))?;
    if canonical != manifest_bytes {
        return Err(verification_error("manifest_not_canonical"));
    }
    let mut publication_id = None;
    let display_name;
    if let Some(bytes) = publication {
        let signed = SignedPublication::parse(&bytes).map_err(|_| verification_error("publication_invalid"))?;
        let actual = signed.publication_id();
        if let FileSelector::PublicationId(expected) = selector
            && actual != expected
        {
            return Err(verification_error("publication_id_mismatch"));
        }
        if let Some(expected) = expected_publication
            && decode_content_id(expected) != Some(actual)
        {
            return Err(verification_error("publication_id_mismatch"));
        }
        manifest
            .validate_for_publication(signed.publication())
            .map_err(|_| verification_error("manifest_mismatch"))?;
        display_name = signed.publication().display_filename.clone();
        publication_id = Some(hex_lower(&actual));
    } else {
        if matches!(selector, FileSelector::PublicationId(_)) {
            return Err(verification_error("publication_missing"));
        }
        display_name = format!("download-{}.bin", &hex_lower(&manifest.content_id)[..16]);
    }
    if let FileSelector::ContentId(expected) = selector
        && manifest.content_id != expected
    {
        return Err(verification_error("content_id_mismatch"));
    }
    let content_id = hex_lower(&manifest.content_id);
    if let Some(expected) = expected_content
        && decode_content_id(expected) != Some(manifest.content_id)
    {
        return Err(verification_error("content_id_mismatch"));
    }
    let root = hex_lower(&manifest_root(&manifest_bytes));
    let chunk_map = ChunkMap::new(manifest.total_size);
    Ok(VerifiedMetadata {
        manifest,
        manifest_bytes,
        manifest_root: root,
        content_id,
        publication_id,
        display_name: safe_display_name(&display_name),
        source_peer: source_peer.to_owned(),
        chunk_map,
    })
}

fn fetch_metadata(
    context: &DownloadContext,
    control: &DownloadControl,
    job: &DownloadJob,
    selector: FileSelector,
) -> Result<(FileMetadata, String), TransferError> {
    let mut last_error: Option<NetError> = None;
    for _ in 0..METADATA_ROUNDS {
        for peer in &job.sources {
            check_control_metadata(control)?;
            match context.transport.metadata(peer, selector) {
                Ok(metadata) => return Ok((metadata, peer.clone())),
                Err(error) => {
                    if matches!(
                        error,
                        NetError::InvalidPeerId | NetError::InvalidAddress | NetError::MissingPeerId
                    ) {
                        return Err(TransferError::Failed(invalid_argument("the source peer ID is invalid")));
                    }
                    last_error = Some(error);
                    backoff_metadata(control)?;
                }
            }
        }
    }
    Err(TransferError::Failed(map_source_failure(last_error.as_ref())))
}

fn download_chunks(
    context: &DownloadContext,
    control: &DownloadControl,
    job: &DownloadJob,
    metadata: &VerifiedMetadata,
    start_prefix: u64,
    content_id: &[u8; 32],
    progress: &mut Progress,
) -> Result<u64, TransferError> {
    let completed = metadata.chunk_map.completed();
    let pending = completed.iter().filter(|done| !**done).count();
    if pending == 0 {
        emit_progress(
            context,
            &job.download_id,
            progress,
            true,
            completed.len(),
            completed.len(),
            &[],
        );
        let imported = context
            .store
            .complete_download(&job.download_id)
            .map_err(store_failure)?;
        return Ok(imported.size_bytes);
    }

    let partial_path = context.store.partial_path(&job.download_id).map_err(store_failure)?;
    let scheduler = MultiSourceScheduler::new(job.sources.clone(), &completed);
    let total_chunks = scheduler.total_chunks();
    let shared = TransferShared::new(
        scheduler,
        Progress {
            verified: start_prefix,
            total: metadata.manifest.total_size,
            source_peer: progress.source_peer.clone(),
            started: progress.started,
            start_verified: progress.start_verified,
            last_emit: progress.last_emit,
        },
    );
    let workers = job.sources.len().min(MAX_PARALLEL_SOURCES).min(pending).max(1);
    let result = (|| -> Result<u64, TransferError> {
        std::thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| {
                    transfer_worker(context, control, job, metadata, content_id, &partial_path, &shared);
                });
            }
        });

        if let Some(stop) = shared.take_stop() {
            return Err(stop);
        }
        let (is_complete, completed_chunks, stats) = {
            let scheduler = lock(&shared.scheduler);
            (
                scheduler.is_complete(),
                scheduler.completed_count(),
                scheduler.stats().to_vec(),
            )
        };
        if !is_complete {
            return Err(verification_error("transfer_incomplete"));
        }
        {
            let mut state = lock(&shared.progress);
            emit_progress(
                context,
                &job.download_id,
                &mut state,
                true,
                completed_chunks,
                usize::try_from(total_chunks).unwrap_or(usize::MAX),
                &stats,
            );
        }
        let imported = context
            .store
            .complete_download(&job.download_id)
            .map_err(store_failure)?;
        Ok(imported.size_bytes)
    })();
    {
        let state = lock(&shared.progress);
        progress.verified = state.verified;
        progress.source_peer = state.source_peer.clone();
        progress.last_emit = state.last_emit;
    }
    result
}

enum ChunkFetchError {
    Stop(TransferError),
    Net(NetError),
    Corrupt,
}

#[allow(clippy::too_many_arguments)]
fn transfer_worker(
    context: &DownloadContext,
    control: &DownloadControl,
    job: &DownloadJob,
    metadata: &VerifiedMetadata,
    content_id: &[u8; 32],
    partial_path: &std::path::Path,
    shared: &TransferShared,
) {
    loop {
        if shared.is_aborted() {
            return;
        }
        if let Err(stop) = check_control(control) {
            shared.request_stop(stop);
            return;
        }
        let decision = lock(&shared.scheduler).next_lease();
        let lease = match decision {
            LeaseDecision::Done => return,
            LeaseDecision::Failed(failure) => {
                shared.request_stop(shared.schedule_failure(failure));
                return;
            }
            LeaseDecision::Wait => {
                shared.wait_for_work();
                continue;
            }
            LeaseDecision::Lease(lease) => lease,
        };
        let Some(entry) = metadata
            .manifest
            .chunks
            .get(usize::try_from(lease.index).unwrap_or(usize::MAX))
        else {
            shared.request_stop(verification_error("chunk_index_invalid"));
            return;
        };
        let started = Instant::now();
        match fetch_chunk(
            context,
            control,
            &lease.peer,
            content_id,
            lease.index,
            entry.length,
            entry.sha256,
        ) {
            Ok(bytes) => {
                if let Err(stop) = write_chunk(partial_path, lease.index, &bytes) {
                    shared.request_stop(stop);
                    return;
                }
                if let Err(error) = context.store.checkpoint_chunk(&job.download_id, lease.index) {
                    shared.request_stop(store_failure(error));
                    return;
                }
                let (completed_chunks, stats) = {
                    let mut scheduler = lock(&shared.scheduler);
                    scheduler.record_success(&lease, entry.length, started.elapsed());
                    (scheduler.completed_count(), scheduler.stats().to_vec())
                };
                {
                    let mut state = lock(&shared.progress);
                    state.verified = state.verified.saturating_add(entry.length);
                    state.source_peer = Some(lease.peer.clone());
                    emit_progress(
                        context,
                        &job.download_id,
                        &mut state,
                        false,
                        completed_chunks,
                        usize::try_from(metadata.chunk_map.total()).unwrap_or(usize::MAX),
                        &stats,
                    );
                }
                shared.work.notify_all();
            }
            Err(ChunkFetchError::Stop(stop)) => {
                shared.request_stop(stop);
                return;
            }
            Err(ChunkFetchError::Corrupt) => {
                lock(&shared.scheduler).record_failure(&lease, SourceFailure::Corrupt);
                shared.work.notify_all();
                if !retry_backoff(control, shared) {
                    return;
                }
            }
            Err(ChunkFetchError::Net(error)) => {
                if matches!(
                    error,
                    NetError::InvalidPeerId | NetError::InvalidAddress | NetError::MissingPeerId
                ) {
                    shared.request_stop(TransferError::Failed(invalid_argument("the source peer ID is invalid")));
                    return;
                }
                shared.record_net_failure(&error);
                lock(&shared.scheduler).record_failure(&lease, SourceFailure::Unavailable);
                shared.work.notify_all();
                if !retry_backoff(control, shared) {
                    return;
                }
            }
        }
    }
}

/// Waits before retrying a failed chunk. Returns false when the worker should stop.
fn retry_backoff(control: &DownloadControl, shared: &TransferShared) -> bool {
    match control.wait(RETRY_DELAY) {
        ControlAction::Continue => true,
        ControlAction::Pause => {
            shared.request_stop(TransferError::Paused("paused by user"));
            false
        }
        ControlAction::Cancel(keep) => {
            shared.request_stop(TransferError::Cancelled(keep));
            false
        }
    }
}

fn fetch_chunk(
    context: &DownloadContext,
    control: &DownloadControl,
    peer: &str,
    content_id: &[u8; 32],
    index: u32,
    length: u64,
    expected: [u8; 32],
) -> Result<Vec<u8>, ChunkFetchError> {
    let bytes = fetch_slices(context, control, peer, content_id, index, length).map_err(|error| match error {
        FetchError::Stopped(stop) => ChunkFetchError::Stop(stop),
        FetchError::Net(error) => ChunkFetchError::Net(error),
    })?;
    if usize::try_from(length).ok() != Some(bytes.len()) {
        return Err(ChunkFetchError::Corrupt);
    }
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    if digest != expected {
        return Err(ChunkFetchError::Corrupt);
    }
    Ok(bytes)
}

fn write_chunk(path: &std::path::Path, index: u32, bytes: &[u8]) -> Result<(), TransferError> {
    let start = u64::from(index)
        .checked_mul(CHUNK_SIZE_V1)
        .ok_or_else(|| verification_error("chunk_index_invalid"))?;
    let mut file = OpenOptions::new().write(true).open(path).map_err(io_failure)?;
    file.seek(SeekFrom::Start(start)).map_err(io_failure)?;
    file.write_all(bytes).map_err(io_failure)?;
    file.flush().map_err(io_failure)?;
    Ok(())
}

fn fetch_slices(
    context: &DownloadContext,
    control: &DownloadControl,
    peer: &str,
    content_id: &[u8; 32],
    index: u32,
    length: u64,
) -> Result<Vec<u8>, FetchError> {
    let length_usize = usize::try_from(length).map_err(|_| FetchError::Net(NetError::MessageTooLarge))?;
    let mut data = Vec::with_capacity(length_usize);
    let mut offset = 0_u64;
    while offset < length {
        check_control(control).map_err(FetchError::Stopped)?;
        let slice = (length - offset).min(u64::try_from(nexus_protocol::MAX_CHUNK_SLICE_BYTES).unwrap_or(u64::MAX));
        let offset_u32 = u32::try_from(offset).map_err(|_| FetchError::Net(NetError::MessageTooLarge))?;
        let slice_u32 = u32::try_from(slice).map_err(|_| FetchError::Net(NetError::MessageTooLarge))?;
        let bytes = context
            .transport
            .chunk(peer, content_id, index, offset_u32, slice_u32)
            .map_err(FetchError::Net)?;
        if bytes.len() != usize::try_from(slice).unwrap_or(usize::MAX) {
            return Err(FetchError::Net(NetError::Rejected));
        }
        data.extend_from_slice(&bytes);
        offset += slice;
    }
    Ok(data)
}

fn check_control(control: &DownloadControl) -> Result<(), TransferError> {
    match control.action() {
        ControlAction::Continue => Ok(()),
        ControlAction::Pause => Err(TransferError::Paused("paused by user")),
        ControlAction::Cancel(keep) => Err(TransferError::Cancelled(keep)),
    }
}

/// Metadata work is not interrupted by pause. A pause requested before the
/// download row exists must still leave a resumable record, so it is applied
/// after registration; cancellation remains immediate.
fn check_control_metadata(control: &DownloadControl) -> Result<(), TransferError> {
    match control.action() {
        ControlAction::Continue | ControlAction::Pause => Ok(()),
        ControlAction::Cancel(keep) => Err(TransferError::Cancelled(keep)),
    }
}

fn backoff_metadata(control: &DownloadControl) -> Result<(), TransferError> {
    if let ControlAction::Cancel(keep) = control.action() {
        return Err(TransferError::Cancelled(keep));
    }
    std::thread::sleep(RETRY_DELAY);
    if let ControlAction::Cancel(keep) = control.action() {
        return Err(TransferError::Cancelled(keep));
    }
    Ok(())
}

fn peer_list_value(peers: &[String]) -> Value {
    Value::Array(peers.iter().map(|peer| Value::Text(peer.clone())).collect())
}

fn source_stats_value(stats: &SourceStats) -> Value {
    Value::Map(vec![
        (Value::Text("peer_id".to_owned()), Value::Text(stats.peer.clone())),
        (Value::Text("attempts".to_owned()), Value::Unsigned(stats.attempts)),
        (Value::Text("successes".to_owned()), Value::Unsigned(stats.successes)),
        (Value::Text("failures".to_owned()), Value::Unsigned(stats.failures)),
        (
            Value::Text("corruptions".to_owned()),
            Value::Unsigned(stats.corruptions),
        ),
        (Value::Text("bytes".to_owned()), Value::Unsigned(stats.bytes)),
        (Value::Text("latency_ms".to_owned()), Value::Unsigned(stats.latency_ms)),
        (Value::Text("proven".to_owned()), Value::Bool(stats.proven)),
        (Value::Text("quarantined".to_owned()), Value::Bool(stats.quarantined)),
    ])
}

#[allow(clippy::too_many_arguments)]
fn emit_progress(
    context: &DownloadContext,
    download_id: &str,
    progress: &mut Progress,
    force: bool,
    completed_chunks: usize,
    total_chunks: usize,
    stats: &[SourceStats],
) {
    if !force && progress.last_emit.elapsed() < PROGRESS_INTERVAL {
        return;
    }
    progress.last_emit = Instant::now();
    let mut data = vec![
        (
            Value::Text("download_id".to_owned()),
            Value::Text(download_id.to_owned()),
        ),
        (
            Value::Text("verified_bytes".to_owned()),
            Value::Unsigned(progress.verified),
        ),
        (Value::Text("total_bytes".to_owned()), Value::Unsigned(progress.total)),
        (Value::Text("rate_bps".to_owned()), Value::Unsigned(progress.rate_bps())),
        (
            Value::Text("verified_chunks".to_owned()),
            Value::Unsigned(u64::try_from(completed_chunks).unwrap_or(0)),
        ),
        (
            Value::Text("total_chunks".to_owned()),
            Value::Unsigned(u64::try_from(total_chunks).unwrap_or(0)),
        ),
    ];
    if let Some(peer) = &progress.source_peer {
        data.push((Value::Text("source_peer".to_owned()), Value::Text(peer.clone())));
    }
    if !stats.is_empty() {
        data.push((
            Value::Text("sources".to_owned()),
            Value::Array(stats.iter().map(source_stats_value).collect()),
        ));
    }
    let event = PendingEvent::progress("download.progress", Value::Map(data)).with_download(download_id.to_owned());
    let _ = context.events.try_push(event);
}

fn emit_failure(context: &DownloadContext, download_id: &str, error: &DomainError, verified: u64) {
    let event = PendingEvent::durable(
        "download.failed",
        Value::Map(vec![
            (
                Value::Text("download_id".to_owned()),
                Value::Text(download_id.to_owned()),
            ),
            (Value::Text("error".to_owned()), error_value(error)),
            (Value::Text("verified_bytes".to_owned()), Value::Unsigned(verified)),
        ]),
    )
    .with_download(download_id.to_owned());
    let _ = context.events.push(event);
}

fn store_failure(error: StoreError) -> TransferError {
    TransferError::Failed(DomainError::from_store(&error))
}

fn io_failure(_error: std::io::Error) -> TransferError {
    TransferError::Failed(DomainError::new(
        DomainCode::IoError,
        "The partial file could not be written.",
    ))
}

fn verification_error(reason: &'static str) -> TransferError {
    let mut error = DomainError::new(
        DomainCode::VerificationFailed,
        "The download failed integrity verification.",
    );
    error.details = Some(Value::Map(vec![(
        Value::Text("reason".to_owned()),
        Value::Text(reason.to_owned()),
    )]));
    TransferError::Failed(error)
}

fn invalid_argument(message: &'static str) -> DomainError {
    DomainError::new(DomainCode::InvalidArgument, message)
}

fn map_source_failure(error: Option<&NetError>) -> DomainError {
    match error {
        Some(NetError::FileUnavailable) | None => {
            DomainError::new(DomainCode::NotFound, "No verified source has the requested content.")
        }
        Some(error) => network_domain_error(error),
    }
}

fn decode_content_id(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let mut bytes = [0_u8; 32];
    for (index, pair) in text.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let high = (pair[0] as char).to_digit(16)?;
        let low = (pair[1] as char).to_digit(16)?;
        bytes[index] = u8::try_from(high * 16 + low).ok()?;
    }
    Some(bytes)
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[usize::from(byte >> 4)] as char);
        out.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    out
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

// ---------------------------------------------------------------------------
// Engine-facing methods
// ---------------------------------------------------------------------------

impl Engine {
    pub(crate) fn download_start(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let publication_id = optional_text(params, "publication_id")?;
        let content_id = optional_text(params, "content_id")?;
        if publication_id.is_none() && content_id.is_none() {
            return Err(CoreError::InvalidArgument("publication_id or content_id is required"));
        }
        let selector = if let Some(publication_id) = &publication_id {
            FileSelector::PublicationId(
                decode_content_id(publication_id)
                    .ok_or(CoreError::InvalidArgument("publication_id must be 32-byte hex"))?,
            )
        } else {
            FileSelector::ContentId(
                decode_content_id(content_id.as_deref().unwrap_or_default())
                    .ok_or(CoreError::InvalidArgument("content_id must be 32-byte hex"))?,
            )
        };
        let sources = parse_sources(params)?;
        let sources = self.without_blocked_sources(sources)?;
        if sources.is_empty() {
            return Ok(MethodOutcome::Domain(DomainError::new(
                DomainCode::PermissionDenied,
                "Every selected source is blocked on this device.",
            )));
        }
        let network = match self.require_network() {
            Ok(network) => network,
            Err(error) => return Ok(MethodOutcome::Domain(error)),
        };
        let download_id = match optional_text(params, "download_id")? {
            Some(id) => {
                validate_download_id(&id)?;
                id
            }
            None => self.new_download_id(),
        };
        if self.find_download(&download_id)?.is_some() {
            return Err(CoreError::InvalidArgument("a download with this ID already exists"));
        }
        let control = Arc::new(DownloadControl::new());
        {
            let mut running = lock_downloads(&self.downloads);
            if running.contains_key(&download_id) {
                return Err(CoreError::InvalidArgument("a download with this ID is already running"));
            }
            running.insert(download_id.clone(), Arc::clone(&control));
        }
        let job = DownloadJob {
            download_id: download_id.clone(),
            selector: Some(selector),
            expected_publication: publication_id,
            expected_content: content_id,
            sources,
            resume: false,
        };
        self.submit_download(network, job, control)?;
        Ok(MethodOutcome::Result(Value::Map(vec![
            (Value::Text("download_id".to_owned()), Value::Text(download_id)),
            (Value::Text("accepted".to_owned()), Value::Bool(true)),
        ])))
    }

    pub(crate) fn download_list(&self, params: &Value) -> Result<Value, CoreError> {
        self.require_running()?;
        let filter = match optional_text(params, "state")? {
            Some(state) => {
                Some(DownloadState::parse(&state).ok_or(CoreError::InvalidArgument("state is not a download state"))?)
            }
            None => None,
        };
        let limit = optional_unsigned(params, "limit")?.unwrap_or(50);
        if !(1..=100).contains(&limit) {
            return Err(CoreError::InvalidArgument("limit must be between 1 and 100"));
        }
        let cursor = optional_unsigned(params, "cursor")?.unwrap_or(0);
        let mut records = self.store.downloads()?;
        if let Some(filter) = filter {
            records.retain(|record| record.state == filter);
        }
        let start = usize::try_from(cursor).unwrap_or(usize::MAX);
        let page: Vec<_> = records
            .iter()
            .skip(start)
            .take(usize::try_from(limit).unwrap_or(100))
            .cloned()
            .collect();
        let next = start.saturating_add(page.len());
        let mut result = vec![(
            Value::Text("items".to_owned()),
            Value::Array(page.iter().map(download_record_value).collect()),
        )];
        if next < records.len() {
            result.push((
                Value::Text("next_cursor".to_owned()),
                Value::Unsigned(u64::try_from(next).unwrap_or(u64::MAX)),
            ));
        }
        Ok(Value::Map(result))
    }

    pub(crate) fn download_pause(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let download_id = required_text(params, "download_id")?;
        validate_download_id(&download_id)?;
        let existing = self.find_download(&download_id)?;
        if existing
            .as_ref()
            .is_some_and(|record| record.state == DownloadState::Completed)
        {
            return Ok(MethodOutcome::Domain(DomainError::new(
                DomainCode::Conflict,
                "A completed download cannot be paused.",
            )));
        }
        let control = lock_downloads(&self.downloads).get(&download_id).cloned();
        if let Some(control) = control
            && control.request_pause()
        {
            if let Some(record) = existing {
                return Ok(MethodOutcome::Result(download_record_value(&record)));
            }
            return Ok(MethodOutcome::Result(Value::Map(vec![
                (Value::Text("download_id".to_owned()), Value::Text(download_id)),
                (Value::Text("state".to_owned()), Value::Text("pending".to_owned())),
            ])));
        }
        let record = existing
            .ok_or_else(|| CoreError::Store(StoreError::RecordNotFound(format!("unknown download {download_id}"))))?;
        match record.state {
            DownloadState::Paused => Ok(MethodOutcome::Result(download_record_value(&record))),
            DownloadState::Downloading | DownloadState::Pending => {
                let record = self.store.pause_download(&download_id, Some("paused by user"))?;
                self.emit_pause_event(&record, "paused by user");
                Ok(MethodOutcome::Result(download_record_value(&record)))
            }
            _ => Ok(MethodOutcome::Domain(DomainError::new(
                DomainCode::Conflict,
                "The download cannot be paused in its current state.",
            ))),
        }
    }

    pub(crate) fn download_resume(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let download_id = required_text(params, "download_id")?;
        let record = self.store.download(&download_id)?;
        if record.state == DownloadState::Completed {
            return Ok(MethodOutcome::Domain(DomainError::new(
                DomainCode::Conflict,
                "The download is already complete.",
            )));
        }
        let publication_id = optional_text(params, "publication_id")?;
        let content_id = optional_text(params, "content_id")?;
        let selector = match (&publication_id, &content_id) {
            (Some(value), _) => Some(FileSelector::PublicationId(
                decode_content_id(value).ok_or(CoreError::InvalidArgument("publication_id must be 32-byte hex"))?,
            )),
            (None, Some(value)) => Some(FileSelector::ContentId(
                decode_content_id(value).ok_or(CoreError::InvalidArgument("content_id must be 32-byte hex"))?,
            )),
            _ => None,
        };
        let mut sources = parse_sources_optional(params)?;
        if sources.is_empty()
            && let Some(peer) = &record.source_peer
        {
            sources.push(peer.clone());
        }
        if sources.is_empty() {
            return Err(CoreError::InvalidArgument(
                "a source peer is required to resume this download",
            ));
        }
        let sources = self.without_blocked_sources(sources)?;
        if sources.is_empty() {
            return Ok(MethodOutcome::Domain(DomainError::new(
                DomainCode::PermissionDenied,
                "Every selected source is blocked on this device.",
            )));
        }
        let network = match self.require_network() {
            Ok(network) => network,
            Err(error) => return Ok(MethodOutcome::Domain(error)),
        };
        let control = Arc::new(DownloadControl::new());
        {
            let mut running = lock_downloads(&self.downloads);
            if running.contains_key(&download_id) {
                return Ok(MethodOutcome::Domain(DomainError::new(
                    DomainCode::Conflict,
                    "The download is already running.",
                )));
            }
            running.insert(download_id.clone(), Arc::clone(&control));
        }
        let job = DownloadJob {
            download_id: download_id.clone(),
            selector,
            expected_publication: publication_id,
            expected_content: content_id,
            sources,
            resume: true,
        };
        self.submit_download(network, job, control)?;
        Ok(MethodOutcome::Result(download_record_value(&record)))
    }

    pub(crate) fn download_cancel(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let download_id = required_text(params, "download_id")?;
        validate_download_id(&download_id)?;
        let keep = optional_bool(params, "keep_partial")?.unwrap_or(false);
        let existing = self.find_download(&download_id)?;
        if existing
            .as_ref()
            .is_some_and(|record| record.state == DownloadState::Completed)
        {
            return Ok(MethodOutcome::Domain(DomainError::new(
                DomainCode::Conflict,
                "A completed download stays in the library.",
            )));
        }
        let control = lock_downloads(&self.downloads).get(&download_id).cloned();
        if let Some(control) = control
            && control.request_cancel(keep)
        {
            return Ok(MethodOutcome::Result(Value::Map(vec![
                (Value::Text("download_id".to_owned()), Value::Text(download_id)),
                (Value::Text("cancelling".to_owned()), Value::Bool(true)),
                (Value::Text("keep_partial".to_owned()), Value::Bool(keep)),
            ])));
        }
        let record = existing
            .ok_or_else(|| CoreError::Store(StoreError::RecordNotFound(format!("unknown download {download_id}"))))?;
        if keep {
            let record = self.store.cancel_download_keep(&download_id)?;
            self.emit_cancel_event(&record, true);
            Ok(MethodOutcome::Result(download_record_value(&record)))
        } else {
            self.store.cancel_download(&download_id)?;
            self.emit_cancel_event(&record, false);
            Ok(MethodOutcome::Result(Value::Map(vec![
                (Value::Text("download_id".to_owned()), Value::Text(download_id)),
                (Value::Text("cancelled".to_owned()), Value::Bool(true)),
                (Value::Text("partial_kept".to_owned()), Value::Bool(false)),
            ])))
        }
    }

    pub(crate) fn download_remove(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let download_id = required_text(params, "download_id")?;
        validate_download_id(&download_id)?;
        if optional_bool(params, "confirmation")? != Some(true) {
            return Err(CoreError::InvalidArgument(
                "confirmation must be true to remove a download",
            ));
        }
        if lock_downloads(&self.downloads).contains_key(&download_id) {
            return Ok(MethodOutcome::Domain(DomainError::new(
                DomainCode::Conflict,
                "Cancel or pause the download before removing it.",
            )));
        }
        let record = self.store.download(&download_id)?;
        if record.state == DownloadState::Completed {
            return Ok(MethodOutcome::Domain(DomainError::new(
                DomainCode::Conflict,
                "A completed download stays in the library.",
            )));
        }
        let removed = self.store.cancel_download(&download_id)?;
        let event = PendingEvent::durable(
            "download.removed",
            Value::Map(vec![
                (Value::Text("download_id".to_owned()), Value::Text(download_id.clone())),
                (Value::Text("removed".to_owned()), Value::Bool(removed)),
            ]),
        )
        .with_download(download_id.clone());
        self.try_emit(event);
        Ok(MethodOutcome::Result(Value::Map(vec![
            (Value::Text("download_id".to_owned()), Value::Text(download_id)),
            (Value::Text("removed".to_owned()), Value::Bool(removed)),
        ])))
    }

    fn find_download(&self, download_id: &str) -> Result<Option<DownloadRecord>, CoreError> {
        match self.store.download(download_id) {
            Ok(record) => Ok(Some(record)),
            Err(StoreError::RecordNotFound(_) | StoreError::InvalidId(_)) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn submit_download(
        &self,
        network: Arc<Network>,
        job: DownloadJob,
        control: Arc<DownloadControl>,
    ) -> Result<(), CoreError> {
        let context = DownloadContext {
            store: Arc::clone(&self.store),
            events: Arc::clone(&self.events),
            transport: Arc::new(NetworkTransport { network }),
        };
        let download_id = job.download_id.clone();
        let cleanup_id = download_id.clone();
        let downloads = Arc::clone(&self.downloads);
        let worker_control = Arc::clone(&control);
        let submit = self.download_pool.submit(move || {
            let result = catch_unwind(AssertUnwindSafe(|| {
                run_download(context, Arc::clone(&worker_control), job);
            }));
            if result.is_err() {
                // `run_download` has its own panic guard; reaching here means the
                // guard itself failed, so leave the control cleanly finished.
            }
            worker_control.finish();
            lock_downloads(&downloads).remove(&download_id);
        });
        if submit.is_err() {
            control.finish();
            lock_downloads(&self.downloads).remove(&cleanup_id);
        }
        submit
    }

    fn new_download_id(&self) -> String {
        let counter = self.next_download.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis());
        format!("dl-{}-{millis}-{counter}", std::process::id())
    }

    fn emit_pause_event(&self, record: &DownloadRecord, reason: &str) {
        let event = PendingEvent::durable(
            "download.paused",
            Value::Map(vec![
                (
                    Value::Text("download_id".to_owned()),
                    Value::Text(record.download_id.clone()),
                ),
                (
                    Value::Text("verified_bytes".to_owned()),
                    Value::Unsigned(record.verified_prefix),
                ),
                (Value::Text("reason".to_owned()), Value::Text(reason.to_owned())),
            ]),
        )
        .with_download(record.download_id.clone());
        self.try_emit(event);
    }

    fn emit_cancel_event(&self, record: &DownloadRecord, partial_kept: bool) {
        let event = PendingEvent::durable(
            "download.cancelled",
            Value::Map(vec![
                (
                    Value::Text("download_id".to_owned()),
                    Value::Text(record.download_id.clone()),
                ),
                (Value::Text("partial_kept".to_owned()), Value::Bool(partial_kept)),
            ]),
        )
        .with_download(record.download_id.clone());
        self.try_emit(event);
    }

    /// Drops locally blocked peers from a source list before any request.
    fn without_blocked_sources(&self, sources: Vec<String>) -> Result<Vec<String>, CoreError> {
        let blocked = self.store.blocked_peers()?;
        Ok(sources
            .into_iter()
            .filter(|source| !blocked.iter().any(|entry| &entry.peer_id == source))
            .collect())
    }
}

fn download_record_value(record: &DownloadRecord) -> Value {
    Value::Map(vec![
        (
            Value::Text("download_id".to_owned()),
            Value::Text(record.download_id.clone()),
        ),
        (
            Value::Text("content_id".to_owned()),
            Value::Text(record.content_id.clone()),
        ),
        (
            Value::Text("display_name".to_owned()),
            Value::Text(record.display_name.clone()),
        ),
        (
            Value::Text("expected_size".to_owned()),
            Value::Unsigned(record.expected_size),
        ),
        (
            Value::Text("verified_prefix".to_owned()),
            Value::Unsigned(record.verified_prefix),
        ),
        (
            Value::Text("state".to_owned()),
            Value::Text(record.state.as_str().to_owned()),
        ),
        (
            Value::Text("source_peer".to_owned()),
            record.source_peer.clone().map_or(Value::Null, Value::Text),
        ),
        (
            Value::Text("manifest_root".to_owned()),
            record.manifest_root.clone().map_or(Value::Null, Value::Text),
        ),
        (
            Value::Text("last_error".to_owned()),
            record.last_error.clone().map_or(Value::Null, Value::Text),
        ),
        (Value::Text("created_at".to_owned()), Value::Unsigned(record.created_at)),
        (Value::Text("updated_at".to_owned()), Value::Unsigned(record.updated_at)),
        (Value::Text("safety".to_owned()), safety_value(&record.display_name)),
    ])
}

fn lock_downloads(
    downloads: &Mutex<HashMap<String, Arc<DownloadControl>>>,
) -> std::sync::MutexGuard<'_, HashMap<String, Arc<DownloadControl>>> {
    downloads.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn validate_download_id(download_id: &str) -> Result<(), CoreError> {
    if download_id.is_empty()
        || download_id.len() > MAX_DOWNLOAD_ID_BYTES
        || !download_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(CoreError::InvalidArgument("download_id is not valid"));
    }
    Ok(())
}

fn parse_sources(params: &Value) -> Result<Vec<String>, CoreError> {
    let sources = parse_sources_optional(params)?;
    if sources.is_empty() {
        return Err(CoreError::InvalidArgument("peer_id or sources is required"));
    }
    Ok(sources)
}

fn parse_sources_optional(params: &Value) -> Result<Vec<String>, CoreError> {
    let mut sources = Vec::new();
    if let Some(peer) = optional_text(params, "peer_id")? {
        sources.push(peer);
    }
    if let Some(value) = map_get(params, "sources") {
        let Value::Array(items) = value else {
            return Err(CoreError::InvalidArgument("sources must be an array"));
        };
        for item in items {
            let peer = item
                .as_text()
                .ok_or(CoreError::InvalidArgument("sources must contain peer IDs"))?;
            sources.push(peer.to_owned());
        }
    }
    let mut deduped = Vec::new();
    for peer in sources {
        if peer.is_empty() || peer.len() > 128 || !peer.is_ascii() {
            return Err(CoreError::InvalidArgument("peer_id is out of bounds"));
        }
        if !deduped.contains(&peer) {
            deduped.push(peer);
        }
    }
    if deduped.len() > MAX_SOURCES {
        return Err(CoreError::InvalidArgument("at most 8 source peers are supported"));
    }
    Ok(deduped)
}

fn optional_text(value: &Value, key: &str) -> Result<Option<String>, CoreError> {
    match map_get(value, key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Text(text)) => Ok(Some(text.clone())),
        Some(_) => Err(CoreError::InvalidArgument("expected a text field")),
    }
}

fn required_text(value: &Value, key: &str) -> Result<String, CoreError> {
    optional_text(value, key)?.ok_or(CoreError::InvalidArgument("a required text field is missing"))
}

fn optional_bool(value: &Value, key: &str) -> Result<Option<bool>, CoreError> {
    match map_get(value, key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(flag)) => Ok(Some(*flag)),
        Some(_) => Err(CoreError::InvalidArgument("expected a boolean field")),
    }
}

fn optional_unsigned(value: &Value, key: &str) -> Result<Option<u64>, CoreError> {
    match map_get(value, key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_unsigned()
            .map(Some)
            .ok_or(CoreError::InvalidArgument("expected an unsigned integer field")),
    }
}

fn map_get<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    let Value::Map(entries) = value else {
        return None;
    };
    entries
        .iter()
        .find_map(|(candidate, value)| (candidate.as_text() == Some(key)).then_some(value))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::fs;
    use std::io::Cursor;
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use nexus_crypto::{IdentityId, KeyPair, to_hex};
    use nexus_protocol::{Category, PublicationV1, SignedPublication};
    use nexus_transfer::build_manifest;

    use crate::event::Event;

    use super::*;

    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    struct TempRoot {
        path: std::path::PathBuf,
    }

    impl TempRoot {
        fn new(label: &str) -> Self {
            let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("nexus-core-stage12-{label}-{}-{sequence}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("create test root");
            Self { path }
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    struct Fixture {
        store: Arc<Store>,
        root: TempRoot,
        content: Vec<u8>,
        content_id: String,
        manifest_root: String,
        manifest_bytes: Vec<u8>,
        publication: Option<Vec<u8>>,
        publication_id: Option<[u8; 32]>,
    }

    impl Fixture {
        fn new(label: &str, size: usize) -> Self {
            Self::build(label, size, None, false)
        }

        fn with_publication(label: &str, size: usize, name: &str, wrong_root: bool) -> Self {
            Self::build(label, size, Some(name), wrong_root)
        }

        fn build(label: &str, size: usize, name: Option<&str>, wrong_root: bool) -> Self {
            let content: Vec<u8> = (0..size)
                .map(|index| u8::try_from((index * 37 + 11) % 251).unwrap_or(0))
                .collect();
            let built = build_manifest(&mut Cursor::new(content.clone())).expect("build manifest");
            let content_id = built.content_hash().to_hex();
            let manifest_root = to_hex(&built.manifest_root().expect("manifest root"));
            let manifest_bytes = built.manifest_bytes().expect("manifest bytes");
            let content_id_bytes = *built.content_hash().as_bytes();
            let manifest_root_bytes = built.manifest_root().expect("manifest root");

            let (publication, publication_id) = match name {
                Some(name) => {
                    let keypair = KeyPair::from_seed([7_u8; 32]);
                    let identity = IdentityId::from_public_key(&keypair.public_key());
                    let root = if wrong_root { [9_u8; 32] } else { manifest_root_bytes };
                    let record = PublicationV1::new(
                        identity,
                        content_id_bytes,
                        u64::try_from(size).expect("size"),
                        name,
                        "stage 12 fixture",
                        Category::Data,
                        root,
                        1,
                        1_700_000_000_000,
                        "0.1.0-test",
                    );
                    let signed = SignedPublication::sign(&record, &keypair).expect("sign");
                    (Some(signed.raw().to_vec()), Some(signed.publication_id()))
                }
                None => (None, None),
            };

            let root = TempRoot::new(label);
            let store = Arc::new(Store::open(&root.path).expect("open store"));

            Self {
                store,
                root,
                content,
                content_id,
                manifest_root,
                manifest_bytes,
                publication,
                publication_id,
            }
        }

        fn content_bytes(&self) -> Vec<u8> {
            fs::read(self.store.content_path(&self.content_id).expect("content path")).expect("read content")
        }
    }

    #[derive(Default)]
    struct CorruptPlan {
        first: AtomicBool,
        all: AtomicBool,
    }

    struct MockSource {
        manifest: Vec<u8>,
        publication: Option<Vec<u8>>,
        content: Vec<u8>,
        chunk_lengths: Vec<u64>,
        fail_metadata: Mutex<HashSet<String>>,
        fail_chunks: Mutex<HashSet<String>>,
        corrupt: CorruptPlan,
        pause_after: Mutex<Option<(usize, Arc<DownloadControl>)>>,
        cancel_after: Mutex<Option<(usize, bool, Arc<DownloadControl>)>>,
        metadata_calls: AtomicUsize,
        chunk_calls: AtomicUsize,
        max_slice: AtomicUsize,
        requested: Mutex<Vec<(String, u32, u32, u32)>>,
    }

    impl MockSource {
        fn new(fixture: &Fixture) -> Self {
            let manifest = ManifestV1::parse_body(&fixture.manifest_bytes).expect("manifest");
            Self {
                manifest: fixture.manifest_bytes.clone(),
                publication: fixture.publication.clone(),
                content: fixture.content.clone(),
                chunk_lengths: manifest.chunks.iter().map(|entry| entry.length).collect(),
                fail_metadata: Mutex::new(HashSet::new()),
                fail_chunks: Mutex::new(HashSet::new()),
                corrupt: CorruptPlan::default(),
                pause_after: Mutex::new(None),
                cancel_after: Mutex::new(None),
                metadata_calls: AtomicUsize::new(0),
                chunk_calls: AtomicUsize::new(0),
                max_slice: AtomicUsize::new(0),
                requested: Mutex::new(Vec::new()),
            }
        }

        fn requested_indexes(&self) -> Vec<u32> {
            let mut indexes: Vec<u32> = self
                .requested
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .iter()
                .map(|(_, index, _, _)| *index)
                .collect();
            indexes.dedup();
            indexes
        }

        fn requested_peers(&self) -> Vec<String> {
            self.requested
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .iter()
                .map(|(peer, _, _, _)| peer.clone())
                .collect()
        }

        fn finish_callback(&self, index: u32, offset: u32, length: u32) {
            let end = u64::from(offset) + u64::from(length);
            let chunk_end = self
                .chunk_lengths
                .get(usize::try_from(index).unwrap_or(usize::MAX))
                .copied();
            if chunk_end != Some(end) {
                return;
            }
            let completed = usize::try_from(index).unwrap_or(usize::MAX) + 1;
            if let Some((threshold, control)) = self
                .pause_after
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .as_ref()
                && *threshold == completed
            {
                control.request_pause();
            }
            if let Some((threshold, keep, control)) = self
                .cancel_after
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .as_ref()
                && *threshold == completed
            {
                control.request_cancel(*keep);
            }
        }
    }

    impl DownloadTransport for MockSource {
        fn metadata(&self, peer_id: &str, _selector: FileSelector) -> Result<FileMetadata, NetError> {
            self.metadata_calls.fetch_add(1, Ordering::Relaxed);
            if self
                .fail_metadata
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .contains(peer_id)
            {
                return Err(NetError::NotConnected);
            }
            Ok(FileMetadata {
                manifest: self.manifest.clone(),
                publication: self.publication.clone(),
            })
        }

        fn chunk(
            &self,
            peer_id: &str,
            _content_id: &[u8; 32],
            index: u32,
            offset: u32,
            length: u32,
        ) -> Result<Vec<u8>, NetError> {
            let call = self.chunk_calls.fetch_add(1, Ordering::Relaxed);
            self.max_slice
                .fetch_max(usize::try_from(length).unwrap_or(usize::MAX), Ordering::Relaxed);
            if self
                .fail_chunks
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .contains(peer_id)
            {
                return Err(NetError::NotConnected);
            }
            let start = usize::try_from(u64::from(index) * CHUNK_SIZE_V1 + u64::from(offset))
                .map_err(|_| NetError::Rejected)?;
            let end = start
                .checked_add(usize::try_from(length).map_err(|_| NetError::Rejected)?)
                .ok_or(NetError::Rejected)?;
            let mut bytes = self.content.get(start..end).ok_or(NetError::Rejected)?.to_vec();
            if (self.corrupt.all.load(Ordering::Relaxed) || (call == 0 && self.corrupt.first.load(Ordering::Relaxed)))
                && let Some(first) = bytes.first_mut()
            {
                *first ^= 0xff;
            }
            self.requested
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push((peer_id.to_owned(), index, offset, length));
            self.finish_callback(index, offset, length);
            Ok(bytes)
        }
    }

    fn job(
        download_id: &str,
        selector: FileSelector,
        expected_publication: Option<String>,
        expected_content: Option<String>,
        sources: &[&str],
        resume: bool,
    ) -> DownloadJob {
        DownloadJob {
            download_id: download_id.to_owned(),
            selector: Some(selector),
            expected_publication,
            expected_content,
            sources: sources.iter().map(|peer| (*peer).to_owned()).collect(),
            resume,
        }
    }

    /// Routes each peer to its own mock so one download can observe several
    /// independent sources.
    struct PeerRouter {
        peers: Vec<(String, Arc<MockSource>)>,
    }

    impl PeerRouter {
        fn new(peers: Vec<(&str, Arc<MockSource>)>) -> Self {
            Self {
                peers: peers
                    .into_iter()
                    .map(|(peer, source)| (peer.to_owned(), source))
                    .collect(),
            }
        }

        fn source(&self, peer: &str) -> Result<&Arc<MockSource>, NetError> {
            self.peers
                .iter()
                .find_map(|(candidate, source)| (candidate == peer).then_some(source))
                .ok_or(NetError::NotConnected)
        }
    }

    impl DownloadTransport for PeerRouter {
        fn metadata(&self, peer_id: &str, selector: FileSelector) -> Result<FileMetadata, NetError> {
            self.source(peer_id)?.metadata(peer_id, selector)
        }

        fn chunk(
            &self,
            peer_id: &str,
            content_id: &[u8; 32],
            index: u32,
            offset: u32,
            length: u32,
        ) -> Result<Vec<u8>, NetError> {
            self.source(peer_id)?.chunk(peer_id, content_id, index, offset, length)
        }
    }

    fn run(
        fixture: &Fixture,
        events: &Arc<EventQueue>,
        transport: Arc<dyn DownloadTransport>,
        control: &Arc<DownloadControl>,
        job: DownloadJob,
    ) {
        let context = DownloadContext {
            store: Arc::clone(&fixture.store),
            events: Arc::clone(events),
            transport,
        };
        run_download(context, Arc::clone(control), job);
    }

    fn drain(events: &EventQueue) -> Vec<Event> {
        let mut collected = Vec::new();
        while let Some(event) = events.next_event(Duration::from_millis(1)).expect("read event") {
            collected.push(event);
        }
        collected
    }

    fn find<'a>(events: &'a [Event], kind: &str) -> &'a Event {
        events
            .iter()
            .find(|event| event.event_type == kind)
            .unwrap_or_else(|| panic!("missing {kind}: {:?}", event_types(events)))
    }

    fn event_types(events: &[Event]) -> Vec<&str> {
        events.iter().map(|event| event.event_type.as_str()).collect()
    }

    fn field<'a>(value: &'a Value, key: &str) -> &'a Value {
        map_get(value, key).unwrap_or_else(|| panic!("missing field {key}"))
    }

    fn unsigned(value: &Value, key: &str) -> u64 {
        field(value, key).as_unsigned().expect("unsigned field")
    }

    #[test]
    fn downloads_a_real_file_and_reports_progress() {
        let fixture = Fixture::new("complete", 2 * 1024 * 1024 + 777);
        let events = Arc::new(EventQueue::new());
        let control = Arc::new(DownloadControl::new());
        let transport = Arc::new(MockSource::new(&fixture));
        let download_id = "dl-stage12-complete";
        run(
            &fixture,
            &events,
            transport.clone(),
            &control,
            job(
                download_id,
                FileSelector::ContentId(decode_content_id(&fixture.content_id).expect("content id")),
                None,
                None,
                &["peer-a"],
                false,
            ),
        );

        assert_eq!(fixture.content_bytes(), fixture.content);
        let collected = drain(&events);
        assert!(event_types(&collected).contains(&"download.started"));
        assert!(event_types(&collected).contains(&"download.progress"));
        let completed = find(&collected, "download.completed");
        assert_eq!(completed.download_id.as_deref(), Some(download_id));
        assert_eq!(
            field(&completed.data, "content_id").as_text(),
            Some(fixture.content_id.as_str())
        );
        assert_eq!(
            unsigned(&completed.data, "size"),
            u64::try_from(fixture.content.len()).expect("size")
        );
        assert_eq!(transport.metadata_calls.load(Ordering::Relaxed), 1);
        assert!(transport.requested_indexes().contains(&0));
    }

    #[test]
    fn chunk_slices_stay_bounded_for_large_files() {
        let fixture = Fixture::new("bounded", 3 * 1024 * 1024 + 123);
        let events = Arc::new(EventQueue::new());
        let control = Arc::new(DownloadControl::new());
        let transport = Arc::new(MockSource::new(&fixture));
        run(
            &fixture,
            &events,
            transport.clone(),
            &control,
            job(
                "dl-stage12-bounded",
                FileSelector::ContentId(decode_content_id(&fixture.content_id).expect("content id")),
                None,
                None,
                &["peer-a"],
                false,
            ),
        );

        assert_eq!(fixture.content_bytes(), fixture.content);
        let max_slice = transport.max_slice.load(Ordering::Relaxed);
        assert!(max_slice > 0);
        assert!(
            max_slice <= nexus_protocol::MAX_CHUNK_SLICE_BYTES,
            "a single slice grew to {max_slice} bytes"
        );
        let chunks = fixture
            .content
            .len()
            .div_ceil(usize::try_from(CHUNK_SIZE_V1).expect("chunk"));
        assert_eq!(transport.requested_indexes().len(), chunks);
    }

    #[test]
    fn a_corrupt_chunk_is_rejected_and_refetched() {
        let fixture = Fixture::new("corrupt", 200 * 1024);
        let events = Arc::new(EventQueue::new());
        let control = Arc::new(DownloadControl::new());
        let transport = Arc::new(MockSource::new(&fixture));
        transport.corrupt.first.store(true, Ordering::Relaxed);
        run(
            &fixture,
            &events,
            transport.clone(),
            &control,
            job(
                "dl-stage12-corrupt",
                FileSelector::ContentId(decode_content_id(&fixture.content_id).expect("content id")),
                None,
                None,
                &["peer-a"],
                false,
            ),
        );

        assert_eq!(fixture.content_bytes(), fixture.content);
        let collected = drain(&events);
        assert!(event_types(&collected).contains(&"download.completed"));
        assert!(!event_types(&collected).contains(&"download.failed"));
        let slices_per_chunk = (200 * 1024_usize).div_ceil(nexus_protocol::MAX_CHUNK_SLICE_BYTES);
        assert!(
            transport.chunk_calls.load(Ordering::Relaxed) > slices_per_chunk,
            "the corrupt chunk was not refetched"
        );
    }

    #[test]
    fn a_persistently_corrupt_source_fails_without_importing_content() {
        let fixture = Fixture::new("corrupt-all", 200 * 1024);
        let events = Arc::new(EventQueue::new());
        let control = Arc::new(DownloadControl::new());
        let transport = Arc::new(MockSource::new(&fixture));
        transport.corrupt.all.store(true, Ordering::Relaxed);
        let download_id = "dl-stage12-corrupt-all";
        run(
            &fixture,
            &events,
            transport,
            &control,
            job(
                download_id,
                FileSelector::ContentId(decode_content_id(&fixture.content_id).expect("content id")),
                None,
                None,
                &["peer-a"],
                false,
            ),
        );

        let record = fixture.store.download(download_id).expect("download record");
        assert_eq!(record.state, DownloadState::Failed);
        assert_eq!(record.last_error.as_deref(), Some("verification_failed"));
        assert!(!fixture.store.content_exists(&fixture.content_id).unwrap_or(true));
        let collected = drain(&events);
        let failed = find(&collected, "download.failed");
        assert_eq!(
            field(field(&failed.data, "error"), "code").as_text(),
            Some(DomainCode::VerificationFailed.as_str())
        );
    }

    #[test]
    fn a_mismatched_publication_manifest_root_is_rejected() {
        let fixture = Fixture::with_publication("wrong-root", 200 * 1024, "fixture.bin", true);
        let events = Arc::new(EventQueue::new());
        let control = Arc::new(DownloadControl::new());
        let transport = Arc::new(MockSource::new(&fixture));
        let publication_id = fixture.publication_id.expect("publication id");
        run(
            &fixture,
            &events,
            transport,
            &control,
            job(
                "dl-stage12-wrong-root",
                FileSelector::PublicationId(publication_id),
                Some(to_hex(&publication_id)),
                None,
                &["peer-a"],
                false,
            ),
        );

        assert!(!fixture.store.content_exists(&fixture.content_id).unwrap_or(true));
        let collected = drain(&events);
        let failed = find(&collected, "download.failed");
        assert_eq!(
            field(field(&failed.data, "error"), "code").as_text(),
            Some(DomainCode::VerificationFailed.as_str())
        );
        assert_eq!(
            field(field(field(&failed.data, "error"), "details"), "reason").as_text(),
            Some("manifest_mismatch")
        );
    }

    #[test]
    fn pause_stops_after_a_verified_chunk_and_resume_fetches_only_the_rest() {
        let fixture = Fixture::new("pause", usize::try_from(CHUNK_SIZE_V1).expect("chunk") + 50_000);
        let events = Arc::new(EventQueue::new());
        let control = Arc::new(DownloadControl::new());
        let transport = Arc::new(MockSource::new(&fixture));
        *transport
            .pause_after
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some((1, Arc::clone(&control)));
        let download_id = "dl-stage12-pause";
        run(
            &fixture,
            &events,
            transport.clone(),
            &control,
            job(
                download_id,
                FileSelector::ContentId(decode_content_id(&fixture.content_id).expect("content id")),
                None,
                None,
                &["peer-a"],
                false,
            ),
        );

        let paused = fixture.store.download(download_id).expect("paused record");
        assert_eq!(paused.state, DownloadState::Paused);
        assert_eq!(paused.verified_prefix, CHUNK_SIZE_V1);
        let collected = drain(&events);
        let pause_event = find(&collected, "download.paused");
        assert_eq!(unsigned(&pause_event.data, "verified_bytes"), CHUNK_SIZE_V1);
        assert_eq!(
            transport.requested_indexes(),
            vec![0],
            "pause must stop before fetching the next chunk"
        );

        let resume_transport = Arc::new(MockSource::new(&fixture));
        let resume_control = Arc::new(DownloadControl::new());
        run(
            &fixture,
            &events,
            resume_transport.clone(),
            &resume_control,
            job(
                download_id,
                FileSelector::ContentId(decode_content_id(&fixture.content_id).expect("content id")),
                None,
                Some(fixture.content_id.clone()),
                &["peer-a"],
                true,
            ),
        );

        assert_eq!(fixture.content_bytes(), fixture.content);
        assert_eq!(
            resume_transport.metadata_calls.load(Ordering::Relaxed),
            0,
            "a resume with a stored manifest must not refetch metadata"
        );
        assert_eq!(resume_transport.requested_indexes(), vec![1]);
        let collected = drain(&events);
        let resumed = find(&collected, "download.resumed");
        assert_eq!(unsigned(&resumed.data, "verified_bytes"), CHUNK_SIZE_V1);
        assert!(event_types(&collected).contains(&"download.completed"));
    }

    #[test]
    fn pause_before_metadata_registration_keeps_a_resumable_row() {
        let fixture = Fixture::new("pause-early", 200 * 1024);
        let events = Arc::new(EventQueue::new());
        let control = Arc::new(DownloadControl::new());
        assert!(control.request_pause());
        let transport = Arc::new(MockSource::new(&fixture));
        let download_id = "dl-stage12-pause-early";
        run(
            &fixture,
            &events,
            transport.clone(),
            &control,
            job(
                download_id,
                FileSelector::ContentId(decode_content_id(&fixture.content_id).expect("content id")),
                None,
                None,
                &["peer-a"],
                false,
            ),
        );

        let paused = fixture
            .store
            .download(download_id)
            .expect("a paused record must exist after metadata registration");
        assert_eq!(paused.state, DownloadState::Paused);
        assert_eq!(paused.verified_prefix, 0);
        assert!(
            transport.requested_indexes().is_empty(),
            "no chunk is requested once the deferred pause is applied"
        );
        let collected = drain(&events);
        assert!(event_types(&collected).contains(&"download.paused"));

        let resume_transport = Arc::new(MockSource::new(&fixture));
        let resume_control = Arc::new(DownloadControl::new());
        run(
            &fixture,
            &events,
            resume_transport.clone(),
            &resume_control,
            job(
                download_id,
                FileSelector::ContentId(decode_content_id(&fixture.content_id).expect("content id")),
                None,
                Some(fixture.content_id.clone()),
                &["peer-a"],
                true,
            ),
        );
        assert_eq!(fixture.content_bytes(), fixture.content);
        assert_eq!(resume_transport.requested_indexes(), vec![0]);
    }

    #[test]
    fn pause_before_metadata_failure_reports_the_failure() {
        let fixture = Fixture::new("pause-early-fail", 200 * 1024);
        let events = Arc::new(EventQueue::new());
        let control = Arc::new(DownloadControl::new());
        assert!(control.request_pause());
        let transport = Arc::new(MockSource::new(&fixture));
        transport
            .fail_metadata
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert("peer-a".to_owned());
        let download_id = "dl-stage12-pause-early-fail";
        run(
            &fixture,
            &events,
            transport,
            &control,
            job(
                download_id,
                FileSelector::ContentId(decode_content_id(&fixture.content_id).expect("content id")),
                None,
                None,
                &["peer-a"],
                false,
            ),
        );

        assert!(
            fixture.store.download(download_id).is_err(),
            "no row is created when metadata never succeeds"
        );
        let collected = drain(&events);
        assert!(
            !event_types(&collected).contains(&"download.paused"),
            "a pause with no row must not claim a resumable state"
        );
        assert!(event_types(&collected).contains(&"download.failed"));
    }

    #[test]
    fn cancel_can_keep_or_delete_the_verified_partial() {
        let fixture = Fixture::new("cancel", usize::try_from(CHUNK_SIZE_V1).expect("chunk") + 4_096);
        let events = Arc::new(EventQueue::new());
        let control = Arc::new(DownloadControl::new());
        let transport = Arc::new(MockSource::new(&fixture));
        *transport
            .cancel_after
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some((1, true, Arc::clone(&control)));
        let download_id = "dl-stage12-cancel-keep";
        run(
            &fixture,
            &events,
            transport,
            &control,
            job(
                download_id,
                FileSelector::ContentId(decode_content_id(&fixture.content_id).expect("content id")),
                None,
                None,
                &["peer-a"],
                false,
            ),
        );

        let record = fixture.store.download(download_id).expect("cancelled record");
        assert_eq!(record.state, DownloadState::Cancelled);
        let partial_path = fixture.store.partial_path(download_id).expect("partial path");
        assert_eq!(fs::metadata(&partial_path).expect("partial file").len(), CHUNK_SIZE_V1);
        let collected = drain(&events);
        let cancelled = find(&collected, "download.cancelled");
        assert_eq!(field(&cancelled.data, "partial_kept").as_bool(), Some(true));

        let resume_transport = Arc::new(MockSource::new(&fixture));
        let resume_control = Arc::new(DownloadControl::new());
        run(
            &fixture,
            &events,
            resume_transport.clone(),
            &resume_control,
            job(
                download_id,
                FileSelector::ContentId(decode_content_id(&fixture.content_id).expect("content id")),
                None,
                Some(fixture.content_id.clone()),
                &["peer-a"],
                true,
            ),
        );
        assert_eq!(fixture.content_bytes(), fixture.content);
        assert_eq!(resume_transport.requested_indexes(), vec![1]);

        let delete_fixture = Fixture::new("cancel-delete", 200 * 1024);
        let delete_events = Arc::new(EventQueue::new());
        let delete_control = Arc::new(DownloadControl::new());
        let delete_transport = Arc::new(MockSource::new(&delete_fixture));
        *delete_transport
            .cancel_after
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some((1, false, Arc::clone(&delete_control)));
        let delete_id = "dl-stage12-cancel-delete";
        run(
            &delete_fixture,
            &delete_events,
            delete_transport,
            &delete_control,
            job(
                delete_id,
                FileSelector::ContentId(decode_content_id(&delete_fixture.content_id).expect("content id")),
                None,
                None,
                &["peer-a"],
                false,
            ),
        );
        assert!(delete_fixture.store.download(delete_id).is_err());
        assert!(!delete_fixture.store.partial_path(delete_id).expect("path").exists());
    }

    #[test]
    fn a_restart_pauses_interrupted_downloads_and_resume_continues_from_verified_bytes() {
        let fixture = Fixture::new("restart", usize::try_from(CHUNK_SIZE_V1).expect("chunk") + 8_000);
        let download_id = "dl-stage12-restart";
        let content_id = decode_content_id(&fixture.content_id).expect("content id");
        fixture
            .store
            .put_manifest(&fixture.manifest_root, &fixture.manifest_bytes)
            .expect("put manifest");
        let record = fixture
            .store
            .begin_download_with_source(
                download_id,
                &fixture.content_id,
                u64::try_from(fixture.content.len()).expect("size"),
                "restart.bin",
                Some(&fixture.manifest_root),
                Some("peer-a"),
            )
            .expect("begin download");
        assert_eq!(record.state, DownloadState::Downloading);
        let partial_path = fixture.store.partial_path(download_id).expect("partial path");
        fs::write(
            &partial_path,
            &fixture.content[..usize::try_from(CHUNK_SIZE_V1).expect("chunk")],
        )
        .expect("write partial");
        fixture
            .store
            .checkpoint_partial(download_id, CHUNK_SIZE_V1)
            .expect("checkpoint");

        let root = fixture.root.path.clone();
        let content = fixture.content.clone();
        let content_id_text = fixture.content_id.clone();
        let transport = MockSource::new(&fixture);
        drop(fixture.store);
        let store = Arc::new(Store::open(&root).expect("reopen store"));
        assert_eq!(store.pause_interrupted_downloads().expect("pause interrupted"), 1);
        let interrupted = store.download(download_id).expect("interrupted record");
        assert_eq!(interrupted.state, DownloadState::Paused);
        assert_eq!(interrupted.verified_prefix, CHUNK_SIZE_V1);
        assert_eq!(interrupted.last_error.as_deref(), Some("interrupted by restart"));

        let events = Arc::new(EventQueue::new());
        let control = Arc::new(DownloadControl::new());
        let transport = Arc::new(transport);
        let context = DownloadContext {
            store: Arc::clone(&store),
            events: Arc::clone(&events),
            transport: transport.clone(),
        };
        run_download(
            context,
            control,
            DownloadJob {
                download_id: download_id.to_owned(),
                selector: Some(FileSelector::ContentId(content_id)),
                expected_publication: None,
                expected_content: Some(content_id_text.clone()),
                sources: vec!["peer-a".to_owned()],
                resume: true,
            },
        );

        let path = store.content_path(&content_id_text).expect("content path");
        assert_eq!(fs::read(path).expect("read content"), content);
        assert_eq!(transport.requested_indexes(), vec![1]);
        let collected = drain(&events);
        let resumed = find(&collected, "download.resumed");
        assert_eq!(unsigned(&resumed.data, "verified_bytes"), CHUNK_SIZE_V1);
        assert!(event_types(&collected).contains(&"download.completed"));
    }

    #[test]
    fn a_disconnected_source_fails_over_to_the_next_verified_peer() {
        let fixture = Fixture::new("failover", usize::try_from(CHUNK_SIZE_V1).expect("chunk") + 10_000);
        let events = Arc::new(EventQueue::new());
        let control = Arc::new(DownloadControl::new());
        let transport = Arc::new(MockSource::new(&fixture));
        transport
            .fail_chunks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert("peer-dead".to_owned());
        run(
            &fixture,
            &events,
            transport.clone(),
            &control,
            job(
                "dl-stage12-failover",
                FileSelector::ContentId(decode_content_id(&fixture.content_id).expect("content id")),
                None,
                None,
                &["peer-dead", "peer-live"],
                false,
            ),
        );

        assert_eq!(fixture.content_bytes(), fixture.content);
        let peers = transport.requested_peers();
        assert!(!peers.is_empty());
        assert!(peers.iter().all(|peer| peer == "peer-live"));
        let collected = drain(&events);
        let progress = collected
            .iter()
            .find(|event| event.event_type == "download.progress")
            .expect("progress");
        assert_eq!(field(&progress.data, "source_peer").as_text(), Some("peer-live"));
        assert!(event_types(&collected).contains(&"download.completed"));
    }

    #[test]
    fn a_total_peer_disconnect_reports_a_failed_event() {
        let fixture = Fixture::new("disconnect", 200 * 1024);
        let events = Arc::new(EventQueue::new());
        let control = Arc::new(DownloadControl::new());
        let transport = Arc::new(MockSource::new(&fixture));
        transport
            .fail_metadata
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert("peer-a".to_owned());
        run(
            &fixture,
            &events,
            transport,
            &control,
            job(
                "dl-stage12-disconnect",
                FileSelector::ContentId(decode_content_id(&fixture.content_id).expect("content id")),
                None,
                None,
                &["peer-a"],
                false,
            ),
        );

        assert!(!fixture.store.content_exists(&fixture.content_id).unwrap_or(true));
        let collected = drain(&events);
        let failed = find(&collected, "download.failed");
        assert_eq!(
            field(field(&failed.data, "error"), "code").as_text(),
            Some(DomainCode::NetworkUnavailable.as_str())
        );
    }

    #[test]
    fn a_publication_display_name_cannot_escape_the_managed_store() {
        let fixture = Fixture::with_publication(
            "unsafe-name",
            usize::try_from(CHUNK_SIZE_V1).expect("chunk") + 4_096,
            "..\\..\\evil/name?.bin",
            false,
        );
        let events = Arc::new(EventQueue::new());
        let control = Arc::new(DownloadControl::new());
        let transport = Arc::new(MockSource::new(&fixture));
        *transport
            .pause_after
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some((1, Arc::clone(&control)));
        let download_id = "dl-stage12-unsafe-name";
        run(
            &fixture,
            &events,
            transport,
            &control,
            job(
                download_id,
                FileSelector::PublicationId(fixture.publication_id.expect("publication id")),
                None,
                None,
                &["peer-a"],
                false,
            ),
        );

        let record = fixture.store.download(download_id).expect("download record");
        assert!(!record.display_name.contains('/'));
        assert!(!record.display_name.contains('\\'));
        assert!(!record.display_name.contains('?'));
        assert!(Path::new(&record.display_name).components().count() == 1);
    }

    fn source_successes(event: &Event) -> Vec<(String, u64)> {
        match map_get(&event.data, "sources") {
            Some(Value::Array(items)) => items
                .iter()
                .map(|item| {
                    (
                        field(item, "peer_id").as_text().unwrap_or_default().to_owned(),
                        unsigned(item, "successes"),
                    )
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    fn selector(fixture: &Fixture) -> FileSelector {
        FileSelector::ContentId(decode_content_id(&fixture.content_id).expect("content id"))
    }

    #[test]
    fn chunks_from_two_peers_combine_into_one_verified_file() {
        let size = usize::try_from(CHUNK_SIZE_V1).expect("chunk") * 3 + 4_096;
        let fixture = Fixture::new("multi-peer", size);
        let events = Arc::new(EventQueue::new());
        let control = Arc::new(DownloadControl::new());
        let peer_a = Arc::new(MockSource::new(&fixture));
        let peer_b = Arc::new(MockSource::new(&fixture));
        let transport = Arc::new(PeerRouter::new(vec![
            ("peer-a", Arc::clone(&peer_a)),
            ("peer-b", Arc::clone(&peer_b)),
        ]));
        let download_id = "dl-stage17-multi-peer";
        run(
            &fixture,
            &events,
            transport,
            &control,
            job(
                download_id,
                selector(&fixture),
                None,
                None,
                &["peer-a", "peer-b"],
                false,
            ),
        );

        assert_eq!(fixture.content_bytes(), fixture.content);
        assert!(!peer_a.requested_peers().is_empty(), "peer-a served no slices");
        assert!(!peer_b.requested_peers().is_empty(), "peer-b served no slices");

        let collected = drain(&events);
        assert!(event_types(&collected).contains(&"download.completed"));
        assert!(!event_types(&collected).contains(&"download.failed"));
        let progress = collected
            .iter()
            .rev()
            .find(|event| event.event_type == "download.progress")
            .expect("a progress event");
        assert_eq!(unsigned(&progress.data, "verified_chunks"), 4);
        assert_eq!(unsigned(&progress.data, "total_chunks"), 4);
        let successes = source_successes(progress);
        assert_eq!(successes.len(), 2, "both sources should be reported: {successes:?}");
        assert!(
            successes.iter().all(|(_, count)| *count > 0),
            "both sources should hold authenticated chunks: {successes:?}"
        );
    }

    #[test]
    fn a_malicious_peer_cannot_corrupt_the_combined_file() {
        let size = usize::try_from(CHUNK_SIZE_V1).expect("chunk") * 2 + 5_000;
        let fixture = Fixture::new("malicious-peer", size);
        let events = Arc::new(EventQueue::new());
        let control = Arc::new(DownloadControl::new());
        let hostile = Arc::new(MockSource::new(&fixture));
        hostile.corrupt.all.store(true, Ordering::Relaxed);
        let honest = Arc::new(MockSource::new(&fixture));
        let transport = Arc::new(PeerRouter::new(vec![
            ("peer-a", Arc::clone(&hostile)),
            ("peer-b", Arc::clone(&honest)),
        ]));
        run(
            &fixture,
            &events,
            transport,
            &control,
            job(
                "dl-stage17-malicious",
                selector(&fixture),
                None,
                None,
                &["peer-a", "peer-b"],
                false,
            ),
        );

        assert_eq!(
            fixture.content_bytes(),
            fixture.content,
            "a hostile source must not reach the managed file"
        );
        assert!(
            !hostile.requested_peers().is_empty(),
            "the hostile source should have been tried"
        );
        let collected = drain(&events);
        assert!(event_types(&collected).contains(&"download.completed"));
        assert!(!event_types(&collected).contains(&"download.failed"));
    }

    #[test]
    fn a_source_that_stops_answering_does_not_stop_the_download() {
        let size = usize::try_from(CHUNK_SIZE_V1).expect("chunk") * 2 + 5_000;
        let fixture = Fixture::new("source-loss", size);
        let events = Arc::new(EventQueue::new());
        let control = Arc::new(DownloadControl::new());
        let vanished = Arc::new(MockSource::new(&fixture));
        vanished
            .fail_chunks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert("peer-a".to_owned());
        let survivor = Arc::new(MockSource::new(&fixture));
        let transport = Arc::new(PeerRouter::new(vec![
            ("peer-a", Arc::clone(&vanished)),
            ("peer-b", Arc::clone(&survivor)),
        ]));
        run(
            &fixture,
            &events,
            transport,
            &control,
            job(
                "dl-stage17-source-loss",
                selector(&fixture),
                None,
                None,
                &["peer-a", "peer-b"],
                false,
            ),
        );

        assert_eq!(fixture.content_bytes(), fixture.content);
        assert!(
            vanished.chunk_calls.load(Ordering::Relaxed) > 0,
            "the scheduler should have tried the vanished source"
        );
        let mut delivered = survivor.requested_indexes();
        delivered.sort_unstable();
        delivered.dedup();
        assert_eq!(
            delivered,
            vec![0, 1, 2],
            "the surviving source should deliver every chunk"
        );
        let collected = drain(&events);
        assert!(event_types(&collected).contains(&"download.completed"));
        assert!(!event_types(&collected).contains(&"download.failed"));
    }

    #[test]
    fn a_paused_download_resumes_from_a_different_peer() {
        let size = usize::try_from(CHUNK_SIZE_V1).expect("chunk") * 2 + 1_234;
        let fixture = Fixture::new("cross-peer-resume", size);
        let events = Arc::new(EventQueue::new());
        let control = Arc::new(DownloadControl::new());
        let peer_a = Arc::new(MockSource::new(&fixture));
        let peer_b = Arc::new(MockSource::new(&fixture));
        *peer_a
            .pause_after
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some((1, Arc::clone(&control)));
        let transport = Arc::new(PeerRouter::new(vec![
            ("peer-a", Arc::clone(&peer_a)),
            ("peer-b", Arc::clone(&peer_b)),
        ]));
        let download_id = "dl-stage17-cross-peer";
        run(
            &fixture,
            &events,
            transport,
            &control,
            job(
                download_id,
                selector(&fixture),
                None,
                None,
                &["peer-a", "peer-b"],
                false,
            ),
        );

        let paused = fixture.store.download(download_id).expect("paused record");
        assert_eq!(paused.state, DownloadState::Paused);
        let map = paused
            .chunk_map
            .as_deref()
            .and_then(|bytes| ChunkMap::from_stored(paused.expected_size, bytes))
            .expect("chunk map");
        assert!(map.is_set(0), "the first chunk was verified before the pause");
        let expected: Vec<u32> = (0..map.total()).filter(|index| !map.is_set(*index)).collect();
        assert!(
            !expected.is_empty() && expected.len() < usize::try_from(map.total()).unwrap_or(usize::MAX),
            "the pause should leave some chunks pending"
        );
        assert_eq!(paused.verified_bytes, map.verified_bytes());

        let resume_peer = Arc::new(MockSource::new(&fixture));
        let resume_transport = Arc::new(PeerRouter::new(vec![("peer-b", Arc::clone(&resume_peer))]));
        let resume_control = Arc::new(DownloadControl::new());
        run(
            &fixture,
            &events,
            resume_transport,
            &resume_control,
            job(
                download_id,
                selector(&fixture),
                None,
                Some(fixture.content_id.clone()),
                &["peer-b"],
                true,
            ),
        );

        assert_eq!(fixture.content_bytes(), fixture.content);
        assert_eq!(
            resume_peer.requested_indexes(),
            expected,
            "the resume should only fetch chunks the first peers did not deliver"
        );
        let collected = drain(&events);
        let resumed = find(&collected, "download.resumed");
        assert_eq!(unsigned(&resumed.data, "verified_bytes"), map.verified_bytes());
        assert!(event_types(&collected).contains(&"download.completed"));
    }

    #[test]
    fn a_resume_reverifies_and_refetches_a_corrupted_local_chunk() {
        let size = usize::try_from(CHUNK_SIZE_V1).expect("chunk") * 2 + 99;
        let fixture = Fixture::new("resume-repair", size);
        let events = Arc::new(EventQueue::new());
        let control = Arc::new(DownloadControl::new());
        let source = Arc::new(MockSource::new(&fixture));
        *source
            .pause_after
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some((2, Arc::clone(&control)));
        let download_id = "dl-stage17-resume-repair";
        run(
            &fixture,
            &events,
            source,
            &control,
            job(download_id, selector(&fixture), None, None, &["peer-a"], false),
        );

        let paused = fixture.store.download(download_id).expect("paused record");
        assert_eq!(paused.state, DownloadState::Paused);
        assert_eq!(paused.verified_prefix, CHUNK_SIZE_V1 * 2);

        let partial = fixture.store.partial_path(download_id).expect("partial path");
        {
            use std::io::{Read as _, Seek as _, Write as _};
            let mut file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&partial)
                .expect("open partial");
            let mut byte = [0_u8; 1];
            file.seek(std::io::SeekFrom::Start(10)).expect("seek");
            file.read_exact(&mut byte).expect("read byte");
            byte[0] ^= 0xff;
            file.seek(std::io::SeekFrom::Start(10)).expect("seek back");
            file.write_all(&byte).expect("corrupt byte");
            file.flush().expect("flush");
        }

        let resume_source = Arc::new(MockSource::new(&fixture));
        let resume_control = Arc::new(DownloadControl::new());
        run(
            &fixture,
            &events,
            resume_source.clone(),
            &resume_control,
            job(
                download_id,
                selector(&fixture),
                None,
                Some(fixture.content_id.clone()),
                &["peer-a"],
                true,
            ),
        );

        assert_eq!(
            fixture.content_bytes(),
            fixture.content,
            "the corrupted chunk must be repaired from the source"
        );
        assert_eq!(
            resume_source.requested_indexes(),
            vec![0, 2],
            "only the corrupted and missing chunks are fetched"
        );
        let collected = drain(&events);
        let resumed = find(&collected, "download.resumed");
        assert_eq!(
            unsigned(&resumed.data, "verified_chunks"),
            1,
            "the intact chunk stays verified across a resume"
        );
        assert!(event_types(&collected).contains(&"download.completed"));
    }
}
