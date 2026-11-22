//! Core lifecycle, method dispatch, event emission, and streaming orchestration.

use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nexus_crypto::{DomainTag, SignatureBytes, Value};
use nexus_identity::{Identity, IdentityVault, LocalEncryptionKey};
use nexus_net::{Network, NetworkConfig, NetworkState, NetworkStatus, PeerDiscovery, PeerSnapshot};
use nexus_store::{MAX_FILE_SIZE, QuotaPolicy, QuotaUsage, Store, StoreConfig, StoreError, safe_display_name};

use crate::config::CoreConfig;
use crate::downloads::{CoreFileProvider, DOWNLOAD_WORKERS};
use crate::envelope::{MAX_RESPONSE_BYTES, Request, encode_failure, encode_success, error_value};
use crate::error::{CoreError, DomainCode, DomainError};
use crate::event::{EventQueue, PendingEvent};
use crate::network::{
    CoreNetworkObserver, connected_peer_value, connectivity_value, domain_error as network_domain_error, peer_value,
    status_value, test_reply_value,
};
use crate::ops::{CancelOutcome, CancelToken, OPERATION_WORKERS, OperationRegistry, WorkerPool};
use crate::streaming::{ExportCursor, FinishedImport, ImportError, ImportSession, MAX_STREAM_CHUNK};

/// Lifecycle state of one core instance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleState {
    /// Handle created, not started.
    Created,
    /// Start in progress.
    Starting,
    /// Fully started.
    Running,
    /// Stop in progress.
    Stopping,
    /// Stopped but handle still valid.
    Stopped,
    /// Start failed; the handle can be stopped or destroyed.
    Failed,
}

impl LifecycleState {
    /// Stable wire representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Stopping => "stopping",
            Self::Stopped => "stopped",
            Self::Failed => "failed",
        }
    }
}

/// Identity availability for snapshots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdentityStatus {
    /// No identity exists under the app-private root.
    Absent,
    /// An identity exists but is not unlocked.
    Locked {
        /// Stable reason code.
        reason: String,
    },
    /// The identity is unlocked.
    Ready {
        /// Stable `nx1...` identity ID.
        identity_id: String,
    },
}

struct IdentitySlot {
    status: IdentityStatus,
    identity: Option<Identity>,
}

impl IdentitySlot {
    fn absent() -> Self {
        Self {
            status: IdentityStatus::Absent,
            identity: None,
        }
    }

    fn locked(reason: &str) -> Self {
        Self {
            status: IdentityStatus::Locked {
                reason: reason.to_owned(),
            },
            identity: None,
        }
    }
}

pub(crate) enum MethodOutcome {
    Result(Value),
    Domain(DomainError),
}

/// One core instance. The FFI registry owns it through an `Arc`.
pub struct Engine {
    config: CoreConfig,
    pub(crate) store: Arc<Store>,
    state: Mutex<LifecycleState>,
    identity: Mutex<IdentitySlot>,
    secret: zeroize::Zeroizing<Vec<u8>>,
    pub(crate) events: Arc<EventQueue>,
    pub(crate) ops: Arc<OperationRegistry>,
    pub(crate) pool: WorkerPool,
    pub(crate) download_pool: WorkerPool,
    pub(crate) downloads: Arc<Mutex<HashMap<String, Arc<crate::downloads::DownloadControl>>>>,
    imports: Mutex<HashMap<u64, ImportSession>>,
    exports: Mutex<HashMap<u64, ExportCursor>>,
    network: Mutex<Option<Arc<Network>>>,
    network_error: Mutex<Option<&'static str>>,
    next_import: AtomicU64,
    next_export: AtomicU64,
    pub(crate) next_download: AtomicU64,
    dropped_events: Arc<AtomicU64>,
    shutting_down: AtomicBool,
}

impl Engine {
    /// Validates the config and opens the database and managed store.
    ///
    /// The ABI version is checked in [`CoreConfig::parse`] before any
    /// filesystem or database work begins.
    pub fn create(config: CoreConfig, secret: &[u8]) -> Result<Self, CoreError> {
        if secret.len() != nexus_identity::LOCAL_ENCRYPTION_KEY_LENGTH {
            return Err(CoreError::InvalidArgument("local secret must be 32 bytes"));
        }
        let quota = QuotaPolicy::new(config.quota_bytes)?;
        let store_config = StoreConfig {
            quota,
            max_file_size: MAX_FILE_SIZE,
            ..StoreConfig::default()
        };
        let store = Store::open_with_config(&config.data_root, store_config)?;
        store.pause_interrupted_downloads()?;
        let pool = WorkerPool::new("nexus-core-op", OPERATION_WORKERS)?;
        let download_pool = WorkerPool::new("nexus-core-download", DOWNLOAD_WORKERS)?;
        Ok(Self {
            config,
            store: Arc::new(store),
            state: Mutex::new(LifecycleState::Created),
            identity: Mutex::new(IdentitySlot::absent()),
            secret: zeroize::Zeroizing::new(secret.to_vec()),
            events: Arc::new(EventQueue::new()),
            ops: Arc::new(OperationRegistry::new()),
            pool,
            download_pool,
            downloads: Arc::new(Mutex::new(HashMap::new())),
            imports: Mutex::new(HashMap::new()),
            exports: Mutex::new(HashMap::new()),
            network: Mutex::new(None),
            network_error: Mutex::new(None),
            next_import: AtomicU64::new(1),
            next_export: AtomicU64::new(1),
            next_download: AtomicU64::new(1),
            dropped_events: Arc::new(AtomicU64::new(0)),
            shutting_down: AtomicBool::new(false),
        })
    }

    /// Active configuration.
    #[must_use]
    pub const fn config(&self) -> &CoreConfig {
        &self.config
    }

    /// Current lifecycle state.
    #[must_use]
    pub fn state(&self) -> LifecycleState {
        *self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Starts the core. Idempotent.
    pub fn start(&self) -> Result<(), CoreError> {
        if self.shutting_down.load(Ordering::Acquire) {
            return Err(CoreError::NotRunning);
        }
        {
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            match *state {
                LifecycleState::Running => return Ok(()),
                LifecycleState::Created | LifecycleState::Stopped | LifecycleState::Failed => {
                    *state = LifecycleState::Starting;
                }
                LifecycleState::Starting | LifecycleState::Stopping => {
                    return Err(CoreError::InvalidArgument(
                        "the core is already changing lifecycle state",
                    ));
                }
            }
        }
        self.emit_state("starting");

        let result = (|| -> Result<(), CoreError> {
            let slot = self.load_identity();
            let ready = matches!(slot.status, IdentityStatus::Ready { .. });
            let locked_reason = match &slot.status {
                IdentityStatus::Locked { reason } => Some(reason.clone()),
                _ => None,
            };
            *self.identity.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = slot;
            self.store.checkpoint()?;
            *self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = LifecycleState::Running;
            self.emit_state("running");
            if ready {
                let identity_id = match self.identity_status() {
                    IdentityStatus::Ready { identity_id } => identity_id,
                    _ => String::new(),
                };
                self.try_emit(PendingEvent::durable(
                    "identity.ready",
                    Value::Map(vec![(Value::Text("identity_id".to_owned()), Value::Text(identity_id))]),
                ));
            } else if let Some(reason) = locked_reason {
                self.try_emit(PendingEvent::durable(
                    "identity.locked",
                    Value::Map(vec![(Value::Text("reason".to_owned()), Value::Text(reason))]),
                ));
            }
            self.start_network();
            Ok(())
        })();

        if let Err(error) = &result {
            *self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = LifecycleState::Failed;
            self.emit_state_with_error("failed", Some(&error.to_string()));
        }
        result
    }

    /// Stops the core. Idempotent and safe from the event-pump shutdown path.
    pub fn stop(&self) -> Result<(), CoreError> {
        {
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            match *state {
                LifecycleState::Created | LifecycleState::Stopped => return Ok(()),
                LifecycleState::Stopping => return Ok(()),
                LifecycleState::Starting => {
                    return Err(CoreError::InvalidArgument("the core is still starting"));
                }
                LifecycleState::Running | LifecycleState::Failed => {
                    *state = LifecycleState::Stopping;
                }
            }
        }
        self.emit_state("stopping");
        self.ops.cancel_all();
        self.imports
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
        self.exports
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
        self.pause_active_downloads();

        let network = self
            .network
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        *self
            .network_error
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
        if let Some(network) = network {
            network.shutdown();
        }
        self.wait_for_downloads_to_stop(Duration::from_secs(5));
        let pause_result = self.store.pause_interrupted_downloads();

        let checkpoint = self.store.checkpoint();
        {
            let mut identity = self.identity.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            let was_ready = matches!(identity.status, IdentityStatus::Ready { .. });
            identity.identity = None;
            if was_ready {
                identity.status = IdentityStatus::Locked {
                    reason: "stopped".to_owned(),
                };
            }
        }
        *self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = LifecycleState::Stopped;
        self.emit_state("stopped");
        self.try_emit(PendingEvent::durable(
            "core.stopped",
            Value::Map(vec![(
                Value::Text("reason".to_owned()),
                Value::Text("requested".to_owned()),
            )]),
        ));
        checkpoint?;
        pause_result?;
        Ok(())
    }

    /// Asks every running download to pause at its next safe boundary.
    fn pause_active_downloads(&self) {
        let controls: Vec<_> = self
            .downloads
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .values()
            .cloned()
            .collect();
        for control in controls {
            control.request_pause();
        }
    }

    /// Waits a bounded time for download workers to checkpoint and exit.
    fn wait_for_downloads_to_stop(&self, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if self
                .downloads
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .is_empty()
            {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Stops the core and closes the event queue for final handle release.
    pub fn shutdown(&self) {
        self.shutting_down.store(true, Ordering::Release);
        // Close first so a full queue or a stopped event pump can never block
        // destroy. Queued events are still drained by the host until empty.
        self.events.close();
        let _ = self.stop();
    }

    /// Invokes one method with a request envelope and returns a response
    /// envelope. Domain failures are encoded as successful ABI calls.
    pub fn invoke(&self, method_id: u32, request_bytes: &[u8]) -> Result<Vec<u8>, CoreError> {
        let request = Request::decode(request_bytes)?;
        match self.dispatch(method_id, request.params)? {
            MethodOutcome::Result(result) => Ok(encode_success(result)?),
            MethodOutcome::Domain(error) => Ok(encode_failure(&error)?),
        }
    }

    /// Waits for the next encoded event. `None` means timeout.
    pub fn next_event(&self, timeout_ms: u32) -> Result<Option<Vec<u8>>, CoreError> {
        let event = self.events.next_event(Duration::from_millis(u64::from(timeout_ms)))?;
        event.map(|event| event.encode().map_err(CoreError::Event)).transpose()
    }

    /// Requests cancellation of an asynchronous operation.
    pub fn cancel(&self, op_id: u64) -> Result<(), CoreError> {
        match self.ops.cancel(op_id) {
            CancelOutcome::Requested | CancelOutcome::AlreadyTerminal => Ok(()),
            CancelOutcome::Unknown => Err(CoreError::InvalidArgument("operation ID is not known to this core")),
        }
    }

    /// Current event queue depth.
    #[must_use]
    pub fn queue_depth(&self) -> usize {
        self.events.depth()
    }

    /// Starts a streaming import.
    pub fn import_begin(&self, display_name: &str, expected_size: u64) -> Result<u64, CoreError> {
        self.require_running()?;
        if expected_size > MAX_FILE_SIZE {
            return Err(CoreError::Store(StoreError::FileTooLarge {
                actual: expected_size,
                limit: MAX_FILE_SIZE,
            }));
        }
        self.store.ensure_capacity(expected_size)?;
        let import_id = self.next_import.fetch_add(1, Ordering::Relaxed);
        let name = safe_display_name(display_name);
        let path = self
            .store
            .temporary_directory()
            .join(format!("import-{import_id}-{}.part", std::process::id()));
        let session = ImportSession::create(import_id, path, name, expected_size)?;
        self.imports
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(import_id, session);
        Ok(import_id)
    }

    /// Writes one bounded chunk to an import.
    pub fn import_write(&self, import_id: u64, data: &[u8]) -> Result<(), CoreError> {
        self.require_running()?;
        if data.len() > MAX_STREAM_CHUNK {
            return Err(CoreError::Import(ImportError::ChunkTooLarge));
        }
        let mut imports = self.imports.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let session = imports
            .get_mut(&import_id)
            .ok_or(CoreError::InvalidArgument("import ID is not active"))?;
        session.write(data)?;
        Ok(())
    }

    /// Finalizes an import, stores its manifest, and adopts the blob.
    pub fn import_finish(&self, import_id: u64) -> Result<Vec<u8>, CoreError> {
        self.require_running()?;
        let session = self
            .imports
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&import_id)
            .ok_or(CoreError::InvalidArgument("import ID is not active"))?;
        let finished = session.finish()?;
        self.complete_import(import_id, finished)
    }

    /// Aborts an import and deletes its temporary file.
    pub fn import_abort(&self, import_id: u64) -> Result<(), CoreError> {
        let session = self
            .imports
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&import_id)
            .ok_or(CoreError::InvalidArgument("import ID is not active"))?;
        session.abort()?;
        Ok(())
    }

    /// Opens a bounded export cursor for a verified content ID.
    pub fn export_open(&self, content_id: &[u8]) -> Result<u64, CoreError> {
        if content_id.len() != 32 {
            return Err(CoreError::InvalidArgument("content ID must be exactly 32 bytes"));
        }
        let content_id = nexus_crypto::to_hex(content_id);
        let path = self.store.content_path(&content_id)?;
        let size = self.store.content(&content_id)?.size_bytes;
        let cursor = ExportCursor::open(path, size)?;
        let export_id = self.next_export.fetch_add(1, Ordering::Relaxed);
        self.exports
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(export_id, cursor);
        Ok(export_id)
    }

    /// Reads at most 1 MiB from an export cursor.
    pub fn export_read(&self, export_id: u64, max_len: usize) -> Result<Vec<u8>, CoreError> {
        if max_len > MAX_STREAM_CHUNK {
            return Err(CoreError::InvalidArgument("export reads are limited to 1 MiB"));
        }
        let mut exports = self.exports.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let cursor = exports
            .get_mut(&export_id)
            .ok_or(CoreError::InvalidArgument("export ID is not active"))?;
        Ok(cursor.read(max_len)?)
    }

    /// Closes an export cursor. Repeated closes are harmless.
    pub fn export_close(&self, export_id: u64) {
        self.exports
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&export_id);
    }

    fn complete_import(&self, import_id: u64, finished: FinishedImport) -> Result<Vec<u8>, CoreError> {
        let result = (|| -> Result<Vec<u8>, CoreError> {
            self.store
                .put_manifest(&finished.manifest_root, &finished.manifest_bytes)?;
            self.store.import_local_staged(
                &finished.path,
                &finished.content_id,
                finished.size,
                &format!("import-{import_id}"),
            )?;
            let summary = Value::Map(vec![
                (
                    Value::Text("content_id".to_owned()),
                    Value::Text(finished.content_id.clone()),
                ),
                (Value::Text("size".to_owned()), Value::Unsigned(finished.size)),
                (
                    Value::Text("manifest_root".to_owned()),
                    Value::Text(finished.manifest_root.clone()),
                ),
                (
                    Value::Text("chunk_size".to_owned()),
                    Value::Unsigned(finished.chunk_size),
                ),
                (
                    Value::Text("display_name".to_owned()),
                    Value::Text(finished.display_name.clone()),
                ),
            ])
            .to_canonical_bytes_with_limits(crate::envelope::RESPONSE_LIMITS)
            .map_err(crate::envelope::EnvelopeError::from)?;
            if summary.len() > MAX_RESPONSE_BYTES {
                return Err(CoreError::Envelope(crate::envelope::EnvelopeError::TooLarge {
                    actual: summary.len(),
                    max: MAX_RESPONSE_BYTES,
                }));
            }
            Ok(summary)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&finished.path);
        }
        result
    }

    fn dispatch(&self, method_id: u32, params: Value) -> Result<MethodOutcome, CoreError> {
        if (0x0B00..=0x0BFF).contains(&method_id) && !self.config.enable_test_methods {
            return Err(CoreError::Unsupported(method_id));
        }
        match method_id {
            0x0101 => self.system_info().map(MethodOutcome::Result),
            0x0102 => self.system_health().map(MethodOutcome::Result),
            0x0103 => self.system_snapshot(&params).map(MethodOutcome::Result),
            0x0301 => self.profile_get_local(),
            0x0302 => self.profile_publish(&params),
            0x0303 => self.profile_lookup(&params),
            0x0401 => self.store_usage().map(MethodOutcome::Result),
            0x0501 => self.publication_create(&params),
            0x0502 => self.publication_unpublish(&params),
            0x0503 => self.publication_get(&params),
            0x0504 => self.publication_list_local(&params),
            0x0505 => self.publication_lookup(&params),
            0x0601 => Ok(MethodOutcome::Result(self.network_status_method()?)),
            0x0602 => self.network_peers(&params).map(MethodOutcome::Result),
            0x0603 => self.network_bootstrap_list().map(MethodOutcome::Result),
            0x0604 => self.network_bootstrap_add(&params),
            0x0605 => self.network_bootstrap_remove(&params),
            0x0606 => self.network_reconnect(&params),
            0x0607 => self.network_connect(&params),
            0x0608 => self.network_send_test(&params),
            0x0609 => self.network_discover(&params),
            0x060A => self.network_relay_list().map(MethodOutcome::Result),
            0x060B => self.network_relay_add(&params),
            0x060C => self.network_relay_remove(&params),
            0x060D => self.network_connectivity().map(MethodOutcome::Result),
            0x0801 => self.download_start(&params),
            0x0802 => self.download_list(&params).map(MethodOutcome::Result),
            0x0803 => self.download_pause(&params),
            0x0804 => self.download_resume(&params),
            0x0805 => self.download_cancel(&params),
            0x0806 => self.download_remove(&params),
            0x0B01 => Ok(MethodOutcome::Result(self.test_echo(&params)?)),
            0x0B02 => self.test_fault(&params),
            _ => Err(CoreError::Unsupported(method_id)),
        }
    }

    fn system_info(&self) -> Result<Value, CoreError> {
        let capabilities: Vec<Value> = [
            "system.info",
            "system.health",
            "system.snapshot",
            "store.usage",
            "profile.get_local",
            "profile.publish",
            "profile.lookup",
            "publication.create",
            "publication.unpublish",
            "publication.get",
            "publication.list_local",
            "publication.lookup",
            "network.status",
            "network.peers",
            "network.bootstrap_list",
            "network.bootstrap_add",
            "network.bootstrap_remove",
            "network.reconnect",
            "network.connect",
            "network.send_test",
            "network.discover",
            "network.relay_list",
            "network.relay_add",
            "network.relay_remove",
            "network.connectivity",
            "download.start",
            "download.list",
            "download.pause",
            "download.resume",
            "download.cancel",
            "download.remove",
        ]
        .into_iter()
        .chain(
            self.config
                .enable_test_methods
                .then_some(["test.echo", "test.fault"])
                .into_iter()
                .flatten(),
        )
        .map(|name| Value::Text(name.to_owned()))
        .collect();
        Ok(Value::Map(vec![
            (
                Value::Text("core_version".to_owned()),
                Value::Text(env!("CARGO_PKG_VERSION").to_owned()),
            ),
            (
                Value::Text("abi_version".to_owned()),
                Value::Unsigned(u64::from(crate::ABI_VERSION)),
            ),
            (
                Value::Text("protocol_version".to_owned()),
                Value::Text(crate::PROTOCOL_VERSION.to_owned()),
            ),
            (
                Value::Text("app_version".to_owned()),
                Value::Text(self.config.app_version.clone()),
            ),
            (
                Value::Text("locale".to_owned()),
                Value::Text(self.config.locale.clone()),
            ),
            (
                Value::Text("platform".to_owned()),
                Value::Text(crate::platform().to_owned()),
            ),
            (
                Value::Text("schema_version".to_owned()),
                Value::Unsigned(u64::try_from(self.store.schema_version()?).unwrap_or(0)),
            ),
            (
                Value::Text("debug_methods".to_owned()),
                Value::Bool(self.config.enable_test_methods),
            ),
            (Value::Text("capabilities".to_owned()), Value::Array(capabilities)),
        ]))
    }

    fn system_health(&self) -> Result<Value, CoreError> {
        let usage = self.store.usage()?;
        let available = self.store.available_space()?;
        let quota = self.store.config().quota;
        Ok(Value::Map(vec![
            (
                Value::Text("state".to_owned()),
                Value::Text(self.state().as_str().to_owned()),
            ),
            (
                Value::Text("network_state".to_owned()),
                Value::Text(self.network_state()),
            ),
            (
                Value::Text("network_enabled".to_owned()),
                Value::Bool(self.config.network_enabled),
            ),
            (
                Value::Text("queue_depth".to_owned()),
                Value::Unsigned(u64::try_from(self.events.depth()).unwrap_or(u64::MAX)),
            ),
            (
                Value::Text("event_queue_closed".to_owned()),
                Value::Bool(self.events.is_closed()),
            ),
            (
                Value::Text("running_operations".to_owned()),
                Value::Unsigned(u64::try_from(self.ops.running_count()).unwrap_or(u64::MAX)),
            ),
            (
                Value::Text("storage".to_owned()),
                storage_value(usage, available, quota.quota_bytes(), quota.reserve_bytes()),
            ),
            (
                Value::Text("identity".to_owned()),
                identity_value(&self.identity_status()),
            ),
        ]))
    }

    fn system_snapshot(&self, params: &Value) -> Result<Value, CoreError> {
        let requested = match map_get(params, "sections") {
            None => None,
            Some(value) => {
                let items = value
                    .as_array()
                    .ok_or(CoreError::InvalidArgument("sections must be an array"))?;
                let mut names = Vec::with_capacity(items.len());
                for item in items {
                    let name = item
                        .as_text()
                        .ok_or(CoreError::InvalidArgument("section names must be text"))?;
                    names.push(name.to_owned());
                }
                Some(names)
            }
        };
        let include = |name: &str| {
            requested
                .as_ref()
                .is_none_or(|names| names.iter().any(|item| item == name || item == "all"))
        };

        let mut entries = Vec::new();
        if include("core") {
            let mut core = vec![
                (
                    Value::Text("state".to_owned()),
                    Value::Text(self.state().as_str().to_owned()),
                ),
                (
                    Value::Text("abi_version".to_owned()),
                    Value::Unsigned(u64::from(crate::ABI_VERSION)),
                ),
                (
                    Value::Text("protocol_version".to_owned()),
                    Value::Text(crate::PROTOCOL_VERSION.to_owned()),
                ),
                (
                    Value::Text("core_version".to_owned()),
                    Value::Text(env!("CARGO_PKG_VERSION").to_owned()),
                ),
                (
                    Value::Text("app_version".to_owned()),
                    Value::Text(self.config.app_version.clone()),
                ),
            ];
            core.push((
                Value::Text("schema_version".to_owned()),
                Value::Unsigned(u64::try_from(self.store.schema_version()?).unwrap_or(0)),
            ));
            entries.push((Value::Text("core".to_owned()), Value::Map(core)));
        }
        if include("storage") {
            let usage = self.store.usage()?;
            let available = self.store.available_space()?;
            let quota = self.store.config().quota;
            entries.push((
                Value::Text("storage".to_owned()),
                storage_value(usage, available, quota.quota_bytes(), quota.reserve_bytes()),
            ));
        }
        if include("identity") {
            entries.push((
                Value::Text("identity".to_owned()),
                identity_value(&self.identity_status()),
            ));
        }
        if include("network") {
            entries.push((Value::Text("network".to_owned()), status_value(&self.network_status())));
        }
        if include("event_queue") {
            entries.push((
                Value::Text("event_queue".to_owned()),
                Value::Map(vec![
                    (
                        Value::Text("depth".to_owned()),
                        Value::Unsigned(u64::try_from(self.events.depth()).unwrap_or(u64::MAX)),
                    ),
                    (Value::Text("closed".to_owned()), Value::Bool(self.events.is_closed())),
                ]),
            ));
        }
        if let Some(names) = &requested {
            for name in names {
                if !matches!(
                    name.as_str(),
                    "core" | "storage" | "identity" | "network" | "event_queue" | "all"
                ) {
                    return Err(CoreError::InvalidArgument("unknown snapshot section"));
                }
            }
        }
        Ok(Value::Map(entries))
    }

    fn store_usage(&self) -> Result<Value, CoreError> {
        let usage = self.store.usage()?;
        let available = self.store.available_space()?;
        let quota = self.store.config().quota;
        Ok(storage_value(
            usage,
            available,
            quota.quota_bytes(),
            quota.reserve_bytes(),
        ))
    }

    fn test_echo(&self, params: &Value) -> Result<Value, CoreError> {
        let payload =
            map_get(params, "payload").ok_or(CoreError::InvalidArgument("test.echo requires a payload field"))?;
        Ok(Value::Map(vec![(Value::Text("payload".to_owned()), payload.clone())]))
    }

    fn test_fault(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        let fault = map_get(params, "fault")
            .and_then(Value::as_text)
            .ok_or(CoreError::InvalidArgument("test.fault requires a fault field"))?;
        if fault == "panic" {
            panic!("forced test panic");
        }
        let delay_ms = optional_unsigned(params, "delay_ms")?.unwrap_or(200);
        if delay_ms > 60_000 {
            return Err(CoreError::InvalidArgument("delay_ms is limited to 60000"));
        }
        let steps = optional_unsigned(params, "steps")?.unwrap_or(3);
        if !(1..=100).contains(&steps) {
            return Err(CoreError::InvalidArgument("steps must be between 1 and 100"));
        }
        let fail_code = match map_get(params, "fail_code").and_then(Value::as_text) {
            None => None,
            Some(code) => Some(
                DomainCode::parse(code).ok_or(CoreError::InvalidArgument("fail_code is not a known domain code"))?,
            ),
        };

        let asynchronous = matches!(fault, "async" | "async_progress" | "async_fail");
        if asynchronous {
            self.require_running()?;
            let (op_id, token) = self.ops.begin();
            let events = Arc::clone(&self.events);
            let ops = Arc::clone(&self.ops);
            let failure = if fault == "async_fail" {
                Some(fail_code.unwrap_or(DomainCode::Internal))
            } else {
                fail_code
            };
            self.pool.submit(move || {
                run_test_operation(&events, &ops, op_id, token, delay_ms, steps, failure);
            })?;
            return Ok(MethodOutcome::Result(Value::Map(vec![
                (Value::Text("op_id".to_owned()), Value::Unsigned(op_id)),
                (Value::Text("accepted".to_owned()), Value::Bool(true)),
            ])));
        }

        if let Some(code) = DomainCode::parse(fault) {
            return Ok(MethodOutcome::Domain(DomainError::new(
                code,
                default_domain_message(code),
            )));
        }
        Err(CoreError::InvalidArgument("unknown fault name"))
    }

    fn load_identity(&self) -> IdentitySlot {
        let vault = match IdentityVault::new(self.config.data_root.clone()) {
            Ok(vault) => vault,
            Err(_) => return IdentitySlot::locked("identity_path_unavailable"),
        };
        match vault.exists() {
            Ok(false) => IdentitySlot::absent(),
            Ok(true) => {
                let key = match LocalEncryptionKey::from_slice(&self.secret) {
                    Ok(key) => key,
                    Err(_) => return IdentitySlot::locked("local_secret_invalid"),
                };
                match vault.load(&key) {
                    Ok(identity) => IdentitySlot {
                        status: IdentityStatus::Ready {
                            identity_id: identity.identity_id().to_string(),
                        },
                        identity: Some(identity),
                    },
                    Err(_) => IdentitySlot::locked("identity_locked"),
                }
            }
            Err(_) => IdentitySlot::locked("identity_unavailable"),
        }
    }

    pub(crate) fn identity_status(&self) -> IdentityStatus {
        self.identity
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .status
            .clone()
    }

    /// Signs a domain-separated message with the unlocked identity.
    ///
    /// The keypair never leaves the identity slot; profile signing only needs
    /// the resulting signature bytes.
    pub(crate) fn sign_identity_domain(&self, tag: DomainTag, message: &[u8]) -> Option<SignatureBytes> {
        self.identity
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .identity
            .as_ref()
            .map(|identity| identity.sign_domain(tag, message))
    }

    pub(crate) fn network_snapshot(&self) -> Option<Arc<Network>> {
        self.network
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn network_status(&self) -> NetworkStatus {
        if let Some(network) = self.network_snapshot() {
            return network.status();
        }
        if self.config.network_enabled {
            let mut status = NetworkStatus::starting(String::new());
            status.state = NetworkState::Unavailable;
            status.last_error = self
                .network_error
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .map(str::to_owned);
            status
        } else {
            NetworkStatus::disabled()
        }
    }

    fn network_state(&self) -> String {
        self.network_status().state.as_str().to_owned()
    }

    /// Starts networking after identity and storage work. A networking failure
    /// never fails core start: the core remains usable offline and the failure
    /// is reported through `network.state` and `network.status`.
    fn start_network(&self) {
        if !self.config.network_enabled {
            return;
        }
        let key = match LocalEncryptionKey::from_slice(&self.secret) {
            Ok(key) => key,
            Err(_) => {
                self.record_network_start_error("local_secret_invalid");
                return;
            }
        };
        let observer = Arc::new(CoreNetworkObserver::new(
            Arc::clone(&self.events),
            Arc::clone(&self.dropped_events),
        ));
        let mut config = NetworkConfig::new(self.config.data_root.clone());
        config.dht_mode = self.config.dht_mode;
        config.relay_fallback = self.config.relay_fallback;
        config.relay_server_enabled = self.config.relay_server;
        let provider = Arc::new(CoreFileProvider::new(Arc::clone(&self.store)));
        match Network::start_with_provider(config, &key, observer, provider) {
            Ok(network) => {
                let status = network.status();
                *self.network.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Arc::new(network));
                *self
                    .network_error
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
                self.emit_network_state(status.state.as_str(), status.peer_count, status.transports, None);
            }
            Err(error) => self.record_network_start_error(error.code()),
        }
    }

    fn record_network_start_error(&self, code: &'static str) {
        *self
            .network_error
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(code);
        let transports = NetworkStatus::starting(String::new()).transports;
        self.emit_network_state("unavailable", 0, transports, Some(code));
    }

    fn emit_network_state(&self, state: &str, peer_count: usize, transports: Vec<String>, reason: Option<&str>) {
        let mut entries = vec![
            (Value::Text("state".to_owned()), Value::Text(state.to_owned())),
            (
                Value::Text("peer_count".to_owned()),
                Value::Unsigned(u64::try_from(peer_count).unwrap_or(u64::MAX)),
            ),
            (
                Value::Text("transports".to_owned()),
                Value::Array(transports.into_iter().map(Value::Text).collect()),
            ),
        ];
        if let Some(reason) = reason {
            entries.push((
                Value::Text("reason".to_owned()),
                Value::Text(nexus_observability::text(reason)),
            ));
        }
        self.try_emit(PendingEvent::durable("network.state", Value::Map(entries)));
    }

    pub(crate) fn require_network(&self) -> Result<Arc<Network>, DomainError> {
        if !self.config.network_enabled {
            return Err(network_domain_error(&nexus_net::NetError::Disabled));
        }
        self.network_snapshot()
            .ok_or_else(|| network_domain_error(&nexus_net::NetError::Unavailable))
    }

    fn network_status_method(&self) -> Result<Value, CoreError> {
        Ok(status_value(&self.network_status()))
    }

    fn network_peers(&self, params: &Value) -> Result<Value, CoreError> {
        let limit = optional_unsigned(params, "limit")?.unwrap_or(64);
        if !(1..=128).contains(&limit) {
            return Err(CoreError::InvalidArgument("limit must be between 1 and 128"));
        }
        let cursor = match map_get(params, "cursor") {
            None | Some(Value::Null) => None,
            Some(value) => Some(
                value
                    .as_text()
                    .ok_or(CoreError::InvalidArgument("cursor must be text"))?,
            ),
        };
        let mut peers = self.network_snapshot().map_or_else(Vec::new, |network| network.peers());
        peers.sort_by(|left, right| left.peer_id.cmp(&right.peer_id));
        let total = peers.len();
        let start = cursor.map_or(0, |cursor| {
            peers
                .iter()
                .position(|peer| peer.peer_id.as_str() > cursor)
                .unwrap_or(total)
        });
        let selected: Vec<&PeerSnapshot> = peers
            .iter()
            .skip(start)
            .take(usize::try_from(limit).unwrap_or(usize::MAX))
            .collect();
        let next_cursor = if start + selected.len() < total {
            selected
                .last()
                .map_or(Value::Null, |peer| Value::Text(peer.peer_id.clone()))
        } else {
            Value::Null
        };
        Ok(Value::Map(vec![
            (
                Value::Text("peers".to_owned()),
                Value::Array(selected.into_iter().map(peer_value).collect()),
            ),
            (Value::Text("next_cursor".to_owned()), next_cursor),
            (
                Value::Text("total".to_owned()),
                Value::Unsigned(u64::try_from(total).unwrap_or(u64::MAX)),
            ),
        ]))
    }

    fn network_bootstrap_list(&self) -> Result<Value, CoreError> {
        let addresses = self
            .network_snapshot()
            .map_or_else(Vec::new, |network| network.bootstrap_list());
        Ok(bootstrap_value(&addresses))
    }

    fn network_bootstrap_add(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let multiaddr = required_multiaddr(params)?;
        let network = match self.require_network() {
            Ok(network) => network,
            Err(error) => return Ok(MethodOutcome::Domain(error)),
        };
        match network.bootstrap_add(&multiaddr) {
            Ok(()) => Ok(MethodOutcome::Result(bootstrap_value(&network.bootstrap_list()))),
            Err(error) => Ok(MethodOutcome::Domain(network_domain_error(&error))),
        }
    }

    fn network_bootstrap_remove(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let multiaddr = required_multiaddr(params)?;
        let network = match self.require_network() {
            Ok(network) => network,
            Err(error) => return Ok(MethodOutcome::Domain(error)),
        };
        match network.bootstrap_remove(&multiaddr) {
            Ok(()) => Ok(MethodOutcome::Result(bootstrap_value(&network.bootstrap_list()))),
            Err(error) => Ok(MethodOutcome::Domain(network_domain_error(&error))),
        }
    }

    fn network_relay_list(&self) -> Result<Value, CoreError> {
        let addresses = self
            .network_snapshot()
            .map_or_else(Vec::new, |network| network.relay_list());
        Ok(relay_value(&addresses))
    }

    fn network_relay_add(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let multiaddr = required_multiaddr(params)?;
        let network = match self.require_network() {
            Ok(network) => network,
            Err(error) => return Ok(MethodOutcome::Domain(error)),
        };
        match network.relay_add(&multiaddr) {
            Ok(()) => Ok(MethodOutcome::Result(relay_value(&network.relay_list()))),
            Err(error) => Ok(MethodOutcome::Domain(network_domain_error(&error))),
        }
    }

    fn network_relay_remove(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let multiaddr = required_multiaddr(params)?;
        let network = match self.require_network() {
            Ok(network) => network,
            Err(error) => return Ok(MethodOutcome::Domain(error)),
        };
        match network.relay_remove(&multiaddr) {
            Ok(()) => Ok(MethodOutcome::Result(relay_value(&network.relay_list()))),
            Err(error) => Ok(MethodOutcome::Domain(network_domain_error(&error))),
        }
    }

    fn network_connectivity(&self) -> Result<Value, CoreError> {
        Ok(connectivity_value(&self.network_status()))
    }

    fn network_reconnect(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let peer_id = match map_get(params, "peer_id") {
            None | Some(Value::Null) => None,
            Some(value) => {
                let peer_id = value
                    .as_text()
                    .ok_or(CoreError::InvalidArgument("peer_id must be text"))?;
                if peer_id.is_empty() || peer_id.len() > 128 {
                    return Err(CoreError::InvalidArgument("peer_id is out of bounds"));
                }
                Some(peer_id.to_owned())
            }
        };
        let network = match self.require_network() {
            Ok(network) => network,
            Err(error) => return Ok(MethodOutcome::Domain(error)),
        };
        let (op_id, token) = self.ops.begin();
        let events = Arc::clone(&self.events);
        let ops = Arc::clone(&self.ops);
        self.pool
            .submit(move || run_network_reconnect(&events, &ops, op_id, token, network, peer_id))?;
        Ok(MethodOutcome::Result(op_accepted(op_id)))
    }

    fn network_connect(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let multiaddr = required_multiaddr(params)?;
        let network = match self.require_network() {
            Ok(network) => network,
            Err(error) => return Ok(MethodOutcome::Domain(error)),
        };
        let (op_id, token) = self.ops.begin();
        let events = Arc::clone(&self.events);
        let ops = Arc::clone(&self.ops);
        self.pool.submit(move || {
            run_network_connect(&events, &ops, op_id, token, network, multiaddr);
        })?;
        Ok(MethodOutcome::Result(op_accepted(op_id)))
    }

    fn network_send_test(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let peer_id = map_get(params, "peer_id")
            .and_then(Value::as_text)
            .ok_or(CoreError::InvalidArgument("peer_id is required"))?;
        if peer_id.is_empty() || peer_id.len() > 128 {
            return Err(CoreError::InvalidArgument("peer_id is out of bounds"));
        }
        let payload = match map_get(params, "payload") {
            Some(Value::Bytes(payload)) => payload.clone(),
            Some(_) => return Err(CoreError::InvalidArgument("payload must be a byte string")),
            None => return Err(CoreError::InvalidArgument("payload is required")),
        };
        if payload.len() > nexus_protocol::MAX_TEST_PAYLOAD_BYTES {
            return Ok(MethodOutcome::Domain(network_domain_error(
                &nexus_net::NetError::MessageTooLarge,
            )));
        }
        let peer_id = peer_id.to_owned();
        let network = match self.require_network() {
            Ok(network) => network,
            Err(error) => return Ok(MethodOutcome::Domain(error)),
        };
        let (op_id, token) = self.ops.begin();
        let events = Arc::clone(&self.events);
        let ops = Arc::clone(&self.ops);
        self.pool.submit(move || {
            run_network_send_test(&events, &ops, op_id, token, network, peer_id, payload);
        })?;
        Ok(MethodOutcome::Result(op_accepted(op_id)))
    }

    fn network_discover(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let target = match map_get(params, "target") {
            None | Some(Value::Null) => None,
            Some(Value::Bytes(bytes)) => {
                let key: [u8; nexus_protocol::DHT_KEY_BYTES] = bytes
                    .as_slice()
                    .try_into()
                    .map_err(|_| CoreError::InvalidArgument("target must be a 32-byte key"))?;
                Some(key)
            }
            Some(_) => return Err(CoreError::InvalidArgument("target must be a byte string")),
        };
        let network = match self.require_network() {
            Ok(network) => network,
            Err(error) => return Ok(MethodOutcome::Domain(error)),
        };
        let (op_id, token) = self.ops.begin();
        let events = Arc::clone(&self.events);
        let ops = Arc::clone(&self.ops);
        self.pool.submit(move || {
            run_network_discover(&events, &ops, op_id, token, network, target);
        })?;
        Ok(MethodOutcome::Result(op_accepted(op_id)))
    }

    pub(crate) fn require_running(&self) -> Result<(), CoreError> {
        if self.state() == LifecycleState::Running {
            Ok(())
        } else {
            Err(CoreError::NotRunning)
        }
    }

    /// Emits a lifecycle or identity event without ever blocking the caller.
    pub(crate) fn try_emit(&self, event: PendingEvent) {
        if self.events.try_push(event).is_err() {
            self.dropped_events.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn emit_state(&self, state: &str) {
        self.try_emit(PendingEvent::durable(
            "core.state",
            Value::Map(vec![(Value::Text("state".to_owned()), Value::Text(state.to_owned()))]),
        ));
    }

    fn emit_state_with_error(&self, state: &str, reason: Option<&str>) {
        let mut entries = vec![(Value::Text("state".to_owned()), Value::Text(state.to_owned()))];
        if let Some(reason) = reason {
            entries.push((
                Value::Text("reason".to_owned()),
                Value::Text(nexus_observability::text(reason)),
            ));
        }
        self.try_emit(PendingEvent::durable("core.state", Value::Map(entries)));
    }
}

fn run_test_operation(
    events: &EventQueue,
    ops: &OperationRegistry,
    op_id: u64,
    token: Arc<CancelToken>,
    delay_ms: u64,
    steps: u64,
    failure: Option<DomainCode>,
) {
    let delay = Duration::from_millis(delay_ms);
    let mut completed = 0_u64;
    for step in 1..=steps {
        if token.wait(delay) {
            let _ = events.push(
                PendingEvent::durable(
                    "test.cancelled",
                    Value::Map(vec![(Value::Text("steps".to_owned()), Value::Unsigned(completed))]),
                )
                .with_op(op_id),
            );
            ops.finish(op_id);
            return;
        }
        completed = step;
        let _ = events.push(
            PendingEvent::progress(
                "test.progress",
                Value::Map(vec![
                    (Value::Text("step".to_owned()), Value::Unsigned(step)),
                    (Value::Text("total".to_owned()), Value::Unsigned(steps)),
                ]),
            )
            .with_op(op_id),
        );
    }
    if let Some(code) = failure {
        let error = DomainError::new(code, default_domain_message(code));
        let _ = events.push(
            PendingEvent::durable(
                "test.failed",
                Value::Map(vec![(Value::Text("error".to_owned()), error_value(&error))]),
            )
            .with_op(op_id),
        );
    } else {
        let _ = events.push(
            PendingEvent::durable(
                "test.completed",
                Value::Map(vec![(Value::Text("steps".to_owned()), Value::Unsigned(steps))]),
            )
            .with_op(op_id),
        );
    }
    ops.finish(op_id);
}

fn op_accepted(op_id: u64) -> Value {
    Value::Map(vec![
        (Value::Text("op_id".to_owned()), Value::Unsigned(op_id)),
        (Value::Text("accepted".to_owned()), Value::Bool(true)),
    ])
}

fn bootstrap_value(addresses: &[String]) -> Value {
    Value::Map(vec![(
        Value::Text("bootstrap".to_owned()),
        Value::Array(addresses.iter().cloned().map(Value::Text).collect()),
    )])
}

fn relay_value(addresses: &[String]) -> Value {
    Value::Map(vec![(
        Value::Text("relays".to_owned()),
        Value::Array(addresses.iter().cloned().map(Value::Text).collect()),
    )])
}

fn required_multiaddr(params: &Value) -> Result<String, CoreError> {
    let multiaddr = map_get(params, "multiaddr")
        .and_then(Value::as_text)
        .ok_or(CoreError::InvalidArgument("multiaddr is required"))?;
    if multiaddr.is_empty() || multiaddr.len() > 512 {
        return Err(CoreError::InvalidArgument("multiaddr is out of bounds"));
    }
    Ok(multiaddr.to_owned())
}

fn emit_network_terminal(events: &EventQueue, op_id: u64, event_type: &str, data: Value) {
    let _ = events.push(PendingEvent::durable(event_type, data).with_op(op_id));
}

fn run_network_connect(
    events: &EventQueue,
    ops: &OperationRegistry,
    op_id: u64,
    token: Arc<CancelToken>,
    network: Arc<Network>,
    multiaddr: String,
) {
    if token.is_cancelled() {
        emit_network_terminal(
            events,
            op_id,
            "network.connect_cancelled",
            Value::Map(vec![(Value::Text("multiaddr".to_owned()), Value::Text(multiaddr))]),
        );
        ops.finish(op_id);
        return;
    }
    let result = network.connect(&multiaddr);
    if token.is_cancelled() {
        emit_network_terminal(
            events,
            op_id,
            "network.connect_cancelled",
            Value::Map(vec![(Value::Text("multiaddr".to_owned()), Value::Text(multiaddr))]),
        );
        ops.finish(op_id);
        return;
    }
    let (event_type, data) = match result {
        Ok(peer) => ("network.connect_completed", connected_peer_value(&peer, &multiaddr)),
        Err(error) => (
            "network.connect_failed",
            Value::Map(vec![
                (Value::Text("multiaddr".to_owned()), Value::Text(multiaddr.clone())),
                (
                    Value::Text("error".to_owned()),
                    error_value(&network_domain_error(&error)),
                ),
            ]),
        ),
    };
    emit_network_terminal(events, op_id, event_type, data);
    ops.finish(op_id);
}

fn run_network_send_test(
    events: &EventQueue,
    ops: &OperationRegistry,
    op_id: u64,
    token: Arc<CancelToken>,
    network: Arc<Network>,
    peer_id: String,
    payload: Vec<u8>,
) {
    if token.is_cancelled() {
        emit_network_terminal(
            events,
            op_id,
            "network.test_cancelled",
            Value::Map(vec![(Value::Text("peer_id".to_owned()), Value::Text(peer_id))]),
        );
        ops.finish(op_id);
        return;
    }
    let result = network.send_test(&peer_id, &payload);
    if token.is_cancelled() {
        emit_network_terminal(
            events,
            op_id,
            "network.test_cancelled",
            Value::Map(vec![(Value::Text("peer_id".to_owned()), Value::Text(peer_id))]),
        );
        ops.finish(op_id);
        return;
    }
    let (event_type, data) = match result {
        Ok(reply) => ("network.test_message", test_reply_value(&reply)),
        Err(error) => (
            "network.test_failed",
            Value::Map(vec![
                (Value::Text("peer_id".to_owned()), Value::Text(peer_id.clone())),
                (
                    Value::Text("error".to_owned()),
                    error_value(&network_domain_error(&error)),
                ),
            ]),
        ),
    };
    emit_network_terminal(events, op_id, event_type, data);
    ops.finish(op_id);
}

fn run_network_reconnect(
    events: &EventQueue,
    ops: &OperationRegistry,
    op_id: u64,
    token: Arc<CancelToken>,
    network: Arc<Network>,
    peer_id: Option<String>,
) {
    let peer_value = || peer_id.as_ref().map_or(Value::Null, |id| Value::Text(id.clone()));
    if token.is_cancelled() {
        emit_network_terminal(
            events,
            op_id,
            "network.reconnect_cancelled",
            Value::Map(vec![(Value::Text("peer_id".to_owned()), peer_value())]),
        );
        ops.finish(op_id);
        return;
    }
    let result = network.reconnect(peer_id.as_deref());
    if token.is_cancelled() {
        emit_network_terminal(
            events,
            op_id,
            "network.reconnect_cancelled",
            Value::Map(vec![(Value::Text("peer_id".to_owned()), peer_value())]),
        );
        ops.finish(op_id);
        return;
    }
    let (event_type, data) = match result {
        Ok(()) => (
            "network.reconnect_completed",
            Value::Map(vec![(Value::Text("peer_id".to_owned()), peer_value())]),
        ),
        Err(error) => (
            "network.reconnect_failed",
            Value::Map(vec![
                (Value::Text("peer_id".to_owned()), peer_value()),
                (
                    Value::Text("error".to_owned()),
                    error_value(&network_domain_error(&error)),
                ),
            ]),
        ),
    };
    emit_network_terminal(events, op_id, event_type, data);
    ops.finish(op_id);
}

fn run_network_discover(
    events: &EventQueue,
    ops: &OperationRegistry,
    op_id: u64,
    token: Arc<CancelToken>,
    network: Arc<Network>,
    target: Option<[u8; 32]>,
) {
    let target_value = || target.map_or(Value::Null, |key| Value::Bytes(key.to_vec()));
    if token.is_cancelled() {
        emit_network_terminal(
            events,
            op_id,
            "network.discovery_cancelled",
            Value::Map(vec![(Value::Text("target".to_owned()), target_value())]),
        );
        ops.finish(op_id);
        return;
    }
    let result = network.discover_peers(target.as_ref().map(|key| key.as_slice()));
    if token.is_cancelled() {
        emit_network_terminal(
            events,
            op_id,
            "network.discovery_cancelled",
            Value::Map(vec![(Value::Text("target".to_owned()), target_value())]),
        );
        ops.finish(op_id);
        return;
    }
    let (event_type, data) = match result {
        Ok(discovery) => (
            "network.discovery_completed",
            discovery_value(&discovery, target_value()),
        ),
        Err(error) => (
            "network.discovery_failed",
            Value::Map(vec![
                (Value::Text("target".to_owned()), target_value()),
                (
                    Value::Text("error".to_owned()),
                    error_value(&network_domain_error(&error)),
                ),
            ]),
        ),
    };
    emit_network_terminal(events, op_id, event_type, data);
    ops.finish(op_id);
}

fn discovery_value(discovery: &PeerDiscovery, target: Value) -> Value {
    let peers = discovery
        .peers
        .iter()
        .map(|peer| {
            Value::Map(vec![
                (Value::Text("peer_id".to_owned()), Value::Text(peer.peer_id.clone())),
                (
                    Value::Text("addresses".to_owned()),
                    Value::Array(peer.addresses.iter().cloned().map(Value::Text).collect()),
                ),
            ])
        })
        .collect();
    Value::Map(vec![
        (Value::Text("target".to_owned()), target),
        (Value::Text("peers".to_owned()), Value::Array(peers)),
        (
            Value::Text("routing_peers".to_owned()),
            Value::Unsigned(u64::try_from(discovery.routing_peers).unwrap_or(u64::MAX)),
        ),
        (
            Value::Text("elapsed_ms".to_owned()),
            Value::Unsigned(discovery.elapsed_ms),
        ),
    ])
}

fn default_domain_message(code: DomainCode) -> String {
    match code {
        DomainCode::InvalidArgument => "The request is invalid.",
        DomainCode::NotFound => "The requested item was not found.",
        DomainCode::Conflict => "The operation conflicts with the current state.",
        DomainCode::QuotaExceeded => "Not enough managed storage for this file.",
        DomainCode::VerificationFailed => "The content failed integrity verification.",
        DomainCode::UnsupportedVersion => "The data version is not supported by this build.",
        DomainCode::NetworkUnavailable => "No network path is available.",
        DomainCode::Timeout => "The operation timed out.",
        DomainCode::Cancelled => "The operation was cancelled.",
        DomainCode::IoError => "A local storage operation failed.",
        DomainCode::PermissionDenied => "The required permission was not granted.",
        DomainCode::Locked => "The identity is locked.",
        DomainCode::Internal => "The core could not complete the operation.",
    }
    .to_owned()
}

fn storage_value(usage: QuotaUsage, available: u64, quota_bytes: u64, reserve_bytes: u64) -> Value {
    let total = usage.total_bytes();
    let health = if total >= quota_bytes {
        "quota_exceeded"
    } else if available < reserve_bytes {
        "low_space"
    } else {
        "ok"
    };
    Value::Map(vec![
        (Value::Text("quota_bytes".to_owned()), Value::Unsigned(quota_bytes)),
        (
            Value::Text("managed_bytes".to_owned()),
            Value::Unsigned(usage.managed_bytes),
        ),
        (
            Value::Text("partial_bytes".to_owned()),
            Value::Unsigned(usage.partial_bytes),
        ),
        (Value::Text("total_bytes".to_owned()), Value::Unsigned(total)),
        (Value::Text("available_bytes".to_owned()), Value::Unsigned(available)),
        (Value::Text("reserve_bytes".to_owned()), Value::Unsigned(reserve_bytes)),
        (Value::Text("health".to_owned()), Value::Text(health.to_owned())),
    ])
}

fn identity_value(status: &IdentityStatus) -> Value {
    match status {
        IdentityStatus::Absent => Value::Map(vec![(
            Value::Text("state".to_owned()),
            Value::Text("absent".to_owned()),
        )]),
        IdentityStatus::Locked { reason } => Value::Map(vec![
            (Value::Text("state".to_owned()), Value::Text("locked".to_owned())),
            (Value::Text("reason".to_owned()), Value::Text(reason.clone())),
        ]),
        IdentityStatus::Ready { identity_id } => Value::Map(vec![
            (Value::Text("state".to_owned()), Value::Text("ready".to_owned())),
            (Value::Text("identity_id".to_owned()), Value::Text(identity_id.clone())),
        ]),
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

fn optional_unsigned(value: &Value, key: &str) -> Result<Option<u64>, CoreError> {
    match map_get(value, key) {
        None => Ok(None),
        Some(value) => value
            .as_unsigned()
            .map(Some)
            .ok_or(CoreError::InvalidArgument("expected an unsigned integer field")),
    }
}

#[cfg(test)]
mod stage10_tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::event::EventQueue;
    use crate::ops::OperationRegistry;

    #[test]
    fn discovery_failure_emits_a_terminal_event() {
        let suffix = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_nanos();
        let root = std::env::temp_dir().join(format!("nexus-core-stage10-failure-{}-{suffix}", std::process::id()));
        fs::create_dir_all(&root).expect("test root");

        // A stopped driver cannot accept the DHT query; the worker must turn
        // that into a terminal failure event instead of panicking.
        let key = LocalEncryptionKey::new([10_u8; 32]);
        let network = Arc::new(Network::start(NetworkConfig::new(root.clone()), &key, Arc::new(())).expect("network"));
        network.shutdown();

        let events = Arc::new(EventQueue::new());
        let ops = Arc::new(OperationRegistry::new());
        let (op_id, token) = ops.begin();
        run_network_discover(&events, &ops, op_id, token, network, Some([9_u8; 32]));

        let event = events
            .next_event(Duration::from_secs(2))
            .expect("event read")
            .expect("terminal event");
        assert_eq!(event.event_type, "network.discovery_failed");
        assert_eq!(event.op_id, Some(op_id));
        match map_get(&event.data, "target") {
            Some(Value::Bytes(bytes)) => assert_eq!(bytes, &[9_u8; 32]),
            other => panic!("missing target: {other:?}"),
        }
        let error = map_get(&event.data, "error").expect("error field");
        assert_eq!(
            map_get(error, "code").and_then(Value::as_text),
            Some(DomainCode::NetworkUnavailable.as_str())
        );
        let details = map_get(error, "details").expect("details field");
        assert_eq!(
            map_get(details, "net_code").and_then(Value::as_text),
            Some("not_running")
        );

        let _ = fs::remove_dir_all(root);
    }
}
