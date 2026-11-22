//! Explicit file publishing, signed publication lookup, and withdrawal.
//!
//! A publication is an exact signed record that binds an uploader identity to a
//! content ID, size, manifest root, and metadata. Publishing never moves or
//! re-encrypts the file: the bytes already live in the managed store and the
//! record only makes them discoverable and serveable.
//!
//! Three facts are reported separately so the UI cannot promise more than the
//! network can deliver:
//!
//! * `signature_verified` means the exact record bytes verify.
//! * `serving` means this device is willing to serve the content now.
//! * `availability` means the last observed ability to reach the content.
//!
//! Unpublishing signs a tombstone, withdraws this device as a provider, and
//! stops local serving. It never deletes copies held by other peers and never
//! deletes the local bytes without a separate cleanup confirmation.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use nexus_crypto::{DomainTag, IdentityId, Value, to_hex};
use nexus_net::{DiscoveredPeer, NetError, Network};
use nexus_protocol::{
    Category, MAX_TAGS, ProtocolError, PublicationV1, SignedPublication, SignedRecord, SignedTombstone, TombstoneV1,
    publication_dht_key, tombstone_dht_key,
};
use nexus_store::{PublicationAvailability, PublicationCacheOutcome, PublicationRecord, Store, TombstoneRecord};

use crate::engine::{Engine, IdentityStatus, MethodOutcome};
use crate::envelope::error_value;
use crate::error::{CoreError, DomainCode, DomainError};
use crate::event::{EventQueue, PendingEvent};
use crate::network::domain_error as network_domain_error;
use crate::ops::{CancelToken, OperationRegistry};

/// A provider observation older than this is reported as seen recently rather
/// than available now.
pub const PUBLICATION_AVAILABILITY_STALE_MS: u64 = 24 * 60 * 60 * 1_000;

/// Plain-text formats that are safe to preview as inert text.
///
/// Executable script extensions are deliberately absent even though they are
/// text on disk: a preview must never normalize opening a program.
const TEXT_EXTENSIONS: &[&str] = &[
    "txt", "text", "md", "markdown", "log", "csv", "tsv", "json", "yaml", "yml", "toml", "ini", "cfg", "conf", "xml",
    "rs", "kt", "kts", "java", "py", "js", "mjs", "cjs", "ts", "tsx", "jsx", "c", "h", "cc", "cpp", "hpp", "go", "rb",
    "php", "sql", "css", "scss",
];

/// Raster image formats that are safe to preview without executing content.
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff", "avif", "ico"];

/// User-facing limits repeated by every unpublish result.
pub const UNPUBLISH_LIMITS: &str = "Unpublishing stops this device from serving the file and announces a signed withdrawal. Copies held by other peers are not deleted, and local bytes are kept.";

/// A bounded publication transport used by workers and unit tests.
pub(crate) trait PublicationTransport: Send + Sync {
    /// Publishes exact signed bytes under a derived key.
    fn publish(&self, key: &[u8], value: &[u8]) -> Result<(), NetError>;
    /// Looks up exact signed bytes under a derived key.
    fn lookup(&self, key: &[u8]) -> Result<Option<Vec<u8>>, NetError>;
    /// Announces this node as a provider for a content ID.
    fn start_providing(&self, content_id: &[u8]) -> Result<(), NetError>;
    /// Withdraws this node as a provider for a content ID.
    fn stop_providing(&self, content_id: &[u8]) -> Result<(), NetError>;
    /// Finds providers for a content ID.
    fn find_providers(&self, content_id: &[u8]) -> Result<Vec<DiscoveredPeer>, NetError>;
}

/// The live networking implementation of [`PublicationTransport`].
pub(crate) struct NetworkPublicationTransport {
    network: Arc<Network>,
}

impl NetworkPublicationTransport {
    pub(crate) fn new(network: Arc<Network>) -> Self {
        Self { network }
    }
}

impl PublicationTransport for NetworkPublicationTransport {
    fn publish(&self, key: &[u8], value: &[u8]) -> Result<(), NetError> {
        self.network.publish_record(key, value)
    }

    fn lookup(&self, key: &[u8]) -> Result<Option<Vec<u8>>, NetError> {
        self.network
            .lookup_record(key)
            .map(|value| value.map(|value| value.value))
    }

    fn start_providing(&self, content_id: &[u8]) -> Result<(), NetError> {
        self.network.start_providing(content_id)
    }

    fn stop_providing(&self, content_id: &[u8]) -> Result<(), NetError> {
        self.network.stop_providing(content_id)
    }

    fn find_providers(&self, content_id: &[u8]) -> Result<Vec<DiscoveredPeer>, NetError> {
        self.network.find_providers(content_id)
    }
}

/// Shared state for a publication worker.
#[derive(Clone)]
pub(crate) struct PublicationContext {
    pub(crate) store: Arc<Store>,
    pub(crate) events: Arc<EventQueue>,
    pub(crate) transport: Option<Arc<dyn PublicationTransport>>,
}

struct PublicationCreate {
    content_id: String,
    display_filename: String,
    description: String,
    category: Category,
    tags: Vec<String>,
    parent_content_id: Option<[u8; 32]>,
    parent_publication_id: Option<[u8; 32]>,
}

impl PublicationCreate {
    fn parse(params: &Value) -> Result<Self, CoreError> {
        let content_id =
            optional_text(params, "content_id")?.ok_or(CoreError::InvalidArgument("content_id is required"))?;
        if decode_content_id(&content_id).is_none() {
            return Err(CoreError::InvalidArgument("content_id must be 32-byte hex"));
        }
        let display_filename = optional_text(params, "display_filename")?
            .or(optional_text(params, "filename")?)
            .ok_or(CoreError::InvalidArgument("display_filename is required"))?;
        if display_filename.trim().is_empty() {
            return Err(CoreError::InvalidArgument("display_filename must not be empty"));
        }
        let description = optional_text(params, "description")?.unwrap_or_default();
        let category = match optional_text(params, "category")? {
            None => Category::Other,
            Some(text) => Category::parse(&text).ok_or(CoreError::InvalidArgument(
                "category is not a known publication category",
            ))?,
        };
        let tags = match map_get(params, "tags") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(items)) => {
                if items.len() > MAX_TAGS {
                    return Err(CoreError::InvalidArgument("too many tags"));
                }
                let mut tags = Vec::with_capacity(items.len());
                for item in items {
                    let tag = item
                        .as_text()
                        .ok_or(CoreError::InvalidArgument("tags must contain text"))?;
                    if tag.is_empty() || tag.len() > nexus_protocol::MAX_TAG_BYTES {
                        return Err(CoreError::InvalidArgument("a tag is empty or too long"));
                    }
                    if !tags.iter().any(|existing| existing == tag) {
                        tags.push(tag.to_owned());
                    }
                }
                tags
            }
            Some(_) => return Err(CoreError::InvalidArgument("tags must be an array or null")),
        };
        let parent_content_id = optional_content_id(params, "parent_content_id")?;
        let parent_publication_id = optional_content_id(params, "parent_publication_id")?;
        Ok(Self {
            content_id,
            display_filename,
            description,
            category,
            tags,
            parent_content_id,
            parent_publication_id,
        })
    }
}

impl Engine {
    /// Signs and stores a publication, then announces it in the background.
    pub(crate) fn publication_create(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let identity_id = match self.identity_status() {
            IdentityStatus::Ready { identity_id } => identity_id,
            IdentityStatus::Absent => {
                return Ok(MethodOutcome::Domain(DomainError::new(
                    DomainCode::NotFound,
                    "No identity exists on this device.",
                )));
            }
            IdentityStatus::Locked { reason } => {
                return Ok(MethodOutcome::Domain(locked_error(&reason)));
            }
        };
        let create = match PublicationCreate::parse(params) {
            Ok(create) => create,
            Err(error) => return Ok(MethodOutcome::Domain(DomainError::from(error))),
        };
        let content = match self.store.content(&create.content_id) {
            Ok(content) => content,
            Err(error) => return Ok(MethodOutcome::Domain(store_domain_error(error))),
        };
        if content.state != nexus_store::ContentState::Verified {
            return Ok(MethodOutcome::Domain(DomainError::new(
                DomainCode::NotFound,
                "The selected content is not verified in the managed store.",
            )));
        }
        if self.store.content_path(&create.content_id).is_err() {
            return Ok(MethodOutcome::Domain(DomainError::new(
                DomainCode::NotFound,
                "The selected content bytes are missing from the managed store.",
            )));
        }
        let manifest = match self.store.ensure_manifest_for_content(&create.content_id) {
            Ok(manifest) => manifest,
            Err(error) => return Ok(MethodOutcome::Domain(store_domain_error(error))),
        };
        let manifest = match nexus_protocol::ManifestV1::parse_body(&manifest) {
            Ok(manifest) => manifest,
            Err(error) => return Ok(MethodOutcome::Domain(protocol_domain_error(&error))),
        };
        let content_bytes = match decode_content_id(&create.content_id) {
            Some(bytes) => bytes,
            None => return Err(CoreError::InvalidArgument("content_id must be 32-byte hex")),
        };
        let identity = match IdentityId::parse(&identity_id) {
            Ok(identity) => identity,
            Err(_) => return Err(CoreError::InvalidArgument("identity ID is malformed")),
        };
        let manifest_root = match manifest.root() {
            Ok(root) => root,
            Err(error) => return Ok(MethodOutcome::Domain(protocol_domain_error(&error))),
        };
        let published_at_ms = now_ms();
        let mut publication = PublicationV1::new(
            identity,
            content_bytes,
            content.size_bytes,
            create.display_filename.clone(),
            create.description.clone(),
            create.category,
            manifest_root,
            1,
            published_at_ms,
            self.config().app_version.clone(),
        )
        .with_tags(create.tags.clone())
        .with_parents(create.parent_content_id, create.parent_publication_id);
        publication.supersedes = None;
        let body = match publication.encode_body() {
            Ok(body) => body,
            Err(error) => return Ok(MethodOutcome::Domain(protocol_domain_error(&error))),
        };
        let Some(signature) = self.sign_identity_domain(DomainTag::Publication, &body) else {
            return Ok(MethodOutcome::Domain(locked_error("identity_locked")));
        };
        let record = match SignedRecord::from_parts(body, signature) {
            Ok(record) => record,
            Err(error) => return Ok(MethodOutcome::Domain(protocol_domain_error(&error))),
        };
        let signed = match SignedPublication::from_record(record) {
            Ok(signed) => signed,
            Err(error) => return Ok(MethodOutcome::Domain(protocol_domain_error(&error))),
        };
        let publication_id = to_hex(&signed.publication_id());
        let stored = PublicationRecord {
            publication_id: publication_id.clone(),
            identity_id: identity_id.clone(),
            content_id: create.content_id.clone(),
            revision: signed.publication().revision,
            record_bytes: signed.raw().to_vec(),
            title: create.display_filename.clone(),
            description: if create.description.is_empty() {
                None
            } else {
                Some(create.description.clone())
            },
            filename: nexus_store::safe_display_name(&create.display_filename),
            published_at: published_at_ms / 1_000,
            updated_at: now_secs(),
            tombstoned: false,
            category: create.category,
            size_bytes: content.size_bytes,
            tags: create.tags.clone(),
            local: true,
            serving: true,
            announced: false,
            availability: PublicationAvailability::Local,
            last_checked_at: Some(published_at_ms),
        };
        if let Err(error) = self.store.upsert_publication(&stored) {
            return Ok(MethodOutcome::Domain(store_domain_error(error)));
        }
        if let Err(error) = self.store.adopt_local_import(&create.content_id, &publication_id) {
            return Ok(MethodOutcome::Domain(store_domain_error(error)));
        }
        let (op_id, token) = self.ops.begin();
        let context = self.publication_context();
        let ops = Arc::clone(&self.ops);
        let worker_content_id = create.content_id.clone();
        self.pool.submit(move || {
            run_publication_publish(&context, &ops, op_id, token, publication_id, worker_content_id);
        })?;
        Ok(MethodOutcome::Result(Value::Map(vec![
            (Value::Text("op_id".to_owned()), Value::Unsigned(op_id)),
            (Value::Text("accepted".to_owned()), Value::Bool(true)),
            (
                Value::Text("publication_id".to_owned()),
                Value::Text(stored.publication_id),
            ),
            (Value::Text("content_id".to_owned()), Value::Text(create.content_id)),
            (Value::Text("revision".to_owned()), Value::Unsigned(1)),
        ])))
    }

    /// Signs a tombstone, stops local serving, and withdraws the provider.
    pub(crate) fn publication_unpublish(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let identity_id = match self.identity_status() {
            IdentityStatus::Ready { identity_id } => identity_id,
            IdentityStatus::Absent => {
                return Ok(MethodOutcome::Domain(DomainError::new(
                    DomainCode::NotFound,
                    "No identity exists on this device.",
                )));
            }
            IdentityStatus::Locked { reason } => {
                return Ok(MethodOutcome::Domain(locked_error(&reason)));
            }
        };
        let publication_id =
            optional_text(params, "publication_id")?.ok_or(CoreError::InvalidArgument("publication_id is required"))?;
        if decode_content_id(&publication_id).is_none() {
            return Err(CoreError::InvalidArgument("publication_id must be 32-byte hex"));
        }
        let confirmation = matches!(map_get(params, "confirmation"), Some(Value::Bool(true)));
        if !confirmation {
            return Ok(MethodOutcome::Domain(DomainError::new(
                DomainCode::Conflict,
                "Unpublishing requires explicit confirmation.",
            )));
        }
        let publication = match self.store.publication(&publication_id) {
            Ok(publication) => publication,
            Err(error) => return Ok(MethodOutcome::Domain(store_domain_error(error))),
        };
        if !publication.local {
            return Ok(MethodOutcome::Domain(DomainError::new(
                DomainCode::Conflict,
                "Only a publication owned by this device can be unpublished.",
            )));
        }
        if publication.identity_id != identity_id {
            return Ok(MethodOutcome::Domain(DomainError::new(
                DomainCode::Conflict,
                "The publication belongs to a different identity.",
            )));
        }
        if publication.tombstoned {
            return Ok(MethodOutcome::Domain(DomainError::new(
                DomainCode::Conflict,
                "The publication is already unpublished.",
            )));
        }
        let signed_publication = match SignedPublication::parse(&publication.record_bytes) {
            Ok(signed) => signed,
            Err(error) => return Ok(MethodOutcome::Domain(protocol_domain_error(&error))),
        };
        let publication_bytes = match decode_content_id(&publication_id) {
            Some(bytes) => bytes,
            None => return Err(CoreError::InvalidArgument("publication_id must be 32-byte hex")),
        };
        let content_bytes = match decode_content_id(&publication.content_id) {
            Some(bytes) => bytes,
            None => return Err(CoreError::InvalidArgument("content_id must be 32-byte hex")),
        };
        let unpublished_at_ms = now_ms();
        let tombstone = TombstoneV1::new(
            signed_publication.publication().identity_id.clone(),
            publication_bytes,
            content_bytes,
            publication.revision,
            unpublished_at_ms,
        );
        let body = match tombstone.encode_body() {
            Ok(body) => body,
            Err(error) => return Ok(MethodOutcome::Domain(protocol_domain_error(&error))),
        };
        let Some(signature) = self.sign_identity_domain(DomainTag::Tombstone, &body) else {
            return Ok(MethodOutcome::Domain(locked_error("identity_locked")));
        };
        let record = match SignedRecord::from_parts(body, signature) {
            Ok(record) => record,
            Err(error) => return Ok(MethodOutcome::Domain(protocol_domain_error(&error))),
        };
        let signed_tombstone = match SignedTombstone::from_record(record) {
            Ok(tombstone) => tombstone,
            Err(error) => return Ok(MethodOutcome::Domain(protocol_domain_error(&error))),
        };
        if let Err(error) = signed_tombstone.validate_for(&signed_publication) {
            return Ok(MethodOutcome::Domain(protocol_domain_error(&error)));
        }
        let tombstone_id = format!("{publication_id}:{}", publication.revision);
        let stored = TombstoneRecord {
            publication_id: publication_id.clone(),
            identity_id: identity_id.clone(),
            content_id: publication.content_id.clone(),
            revision: publication.revision,
            record_bytes: signed_tombstone.raw().to_vec(),
            created_at: unpublished_at_ms / 1_000,
        };
        if let Err(error) = self.store.upsert_tombstone(&stored) {
            return Ok(MethodOutcome::Domain(store_domain_error(error)));
        }
        if let Err(error) = self.store.mark_publication_unpublished(&publication_id, now_secs()) {
            return Ok(MethodOutcome::Domain(store_domain_error(error)));
        }
        let withdraw_provider = match self
            .store
            .content_serveable_except_publication(&publication.content_id, &publication_id)
        {
            Ok(value) => !value,
            Err(error) => return Ok(MethodOutcome::Domain(store_domain_error(error))),
        };
        let (op_id, token) = self.ops.begin();
        let context = self.publication_context();
        let ops = Arc::clone(&self.ops);
        self.pool.submit(move || {
            run_publication_unpublish(
                &context,
                &ops,
                op_id,
                token,
                publication_id,
                publication.content_id,
                tombstone_id,
                stored.revision,
                signed_tombstone,
                withdraw_provider,
            );
        })?;
        Ok(MethodOutcome::Result(Value::Map(vec![
            (Value::Text("op_id".to_owned()), Value::Unsigned(op_id)),
            (Value::Text("accepted".to_owned()), Value::Bool(true)),
            (
                Value::Text("tombstone_id".to_owned()),
                Value::Text(stored_tombstone_id(&stored)),
            ),
        ])))
    }

    /// Returns a local or cached publication with tombstone state.
    pub(crate) fn publication_get(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let publication_id =
            optional_text(params, "publication_id")?.ok_or(CoreError::InvalidArgument("publication_id is required"))?;
        if decode_content_id(&publication_id).is_none() {
            return Err(CoreError::InvalidArgument("publication_id must be 32-byte hex"));
        }
        let publication = match self.store.publication(&publication_id) {
            Ok(publication) => publication,
            Err(error) => return Ok(MethodOutcome::Domain(store_domain_error(error))),
        };
        let tombstone = self.store.tombstone(&publication_id).ok().flatten();
        let signature_verified = signature_verified(&publication);
        Ok(MethodOutcome::Result(Value::Map(vec![
            (Value::Text("found".to_owned()), Value::Bool(true)),
            (
                Value::Text("publication".to_owned()),
                publication_value(&publication, tombstone.as_ref()),
            ),
            (
                Value::Text("tombstone".to_owned()),
                tombstone
                    .as_ref()
                    .map_or(Value::Null, |tombstone| tombstone_value(&publication, tombstone)),
            ),
            (
                Value::Text("serving".to_owned()),
                Value::Bool(publication.serving && !publication.tombstoned),
            ),
            (
                Value::Text("availability".to_owned()),
                Value::Text(effective_availability(&publication).as_str().to_owned()),
            ),
            (
                Value::Text("signature_verified".to_owned()),
                Value::Bool(signature_verified),
            ),
            (Value::Text("preview".to_owned()), preview_value(&publication.filename)),
        ])))
    }

    /// Lists publications owned by the current identity.
    pub(crate) fn publication_list_local(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let limit = optional_unsigned(params, "limit")?.unwrap_or(50);
        if !(1..=100).contains(&limit) {
            return Err(CoreError::InvalidArgument("limit must be between 1 and 100"));
        }
        let cursor = optional_text(params, "cursor")?;
        let identity_id = match self.identity_status() {
            IdentityStatus::Ready { identity_id } => Some(identity_id),
            IdentityStatus::Absent => None,
            IdentityStatus::Locked { reason } => {
                return Ok(MethodOutcome::Domain(locked_error(&reason)));
            }
        };
        let mut publications = match self.store.local_publications(identity_id.as_deref()) {
            Ok(publications) => publications,
            Err(error) => return Ok(MethodOutcome::Domain(store_domain_error(error))),
        };
        publications.sort_by(|left, right| left.publication_id.cmp(&right.publication_id));
        let total = publications.len();
        let start = cursor.map_or(0, |cursor| {
            publications
                .iter()
                .position(|publication| publication.publication_id > cursor)
                .unwrap_or(total)
        });
        let selected: Vec<PublicationRecord> = publications
            .into_iter()
            .skip(start)
            .take(usize::try_from(limit).unwrap_or(usize::MAX))
            .collect();
        let next_cursor = if start + selected.len() < total {
            selected.last().map_or(Value::Null, |publication| {
                Value::Text(publication.publication_id.clone())
            })
        } else {
            Value::Null
        };
        Ok(MethodOutcome::Result(Value::Map(vec![
            (
                Value::Text("identity_id".to_owned()),
                identity_id.map_or(Value::Null, Value::Text),
            ),
            (
                Value::Text("publications".to_owned()),
                Value::Array(selected.iter().map(publication_summary_value).collect()),
            ),
            (Value::Text("next_cursor".to_owned()), next_cursor),
            (
                Value::Text("total".to_owned()),
                Value::Unsigned(u64::try_from(total).unwrap_or(u64::MAX)),
            ),
        ])))
    }

    /// Looks up, verifies, caches, and probes availability for a publication.
    pub(crate) fn publication_lookup(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let publication_id =
            optional_text(params, "publication_id")?.ok_or(CoreError::InvalidArgument("publication_id is required"))?;
        if decode_content_id(&publication_id).is_none() {
            return Err(CoreError::InvalidArgument("publication_id must be 32-byte hex"));
        }
        let (op_id, token) = self.ops.begin();
        let context = self.publication_context();
        let ops = Arc::clone(&self.ops);
        self.pool.submit(move || {
            run_publication_lookup(&context, &ops, op_id, token, publication_id);
        })?;
        Ok(MethodOutcome::Result(Value::Map(vec![
            (Value::Text("op_id".to_owned()), Value::Unsigned(op_id)),
            (Value::Text("accepted".to_owned()), Value::Bool(true)),
        ])))
    }

    fn publication_context(&self) -> PublicationContext {
        PublicationContext {
            store: Arc::clone(&self.store),
            events: Arc::clone(&self.events),
            transport: self
                .network_snapshot()
                .map(|network| Arc::new(NetworkPublicationTransport::new(network)) as Arc<dyn PublicationTransport>),
        }
    }
}

/// Publishes one signed record and announces this node as a provider.
pub(crate) fn run_publication_publish(
    context: &PublicationContext,
    ops: &OperationRegistry,
    op_id: u64,
    token: Arc<CancelToken>,
    publication_id: String,
    content_id: String,
) {
    let publication = match context.store.publication(&publication_id) {
        Ok(publication) => publication,
        Err(error) => {
            emit_publication_event(
                &context.events,
                op_id,
                "publication.publish_failed",
                Value::Map(vec![
                    (Value::Text("publication_id".to_owned()), Value::Text(publication_id)),
                    (Value::Text("error".to_owned()), error_value(&store_domain_error(error))),
                ]),
            );
            ops.finish(op_id);
            return;
        }
    };
    let publication_bytes = match decode_content_id(&publication_id) {
        Some(bytes) => bytes,
        None => {
            emit_publication_event(
                &context.events,
                op_id,
                "publication.publish_failed",
                Value::Map(vec![
                    (Value::Text("publication_id".to_owned()), Value::Text(publication_id)),
                    (
                        Value::Text("error".to_owned()),
                        error_value(&DomainError::invalid("The publication ID is malformed.")),
                    ),
                ]),
            );
            ops.finish(op_id);
            return;
        }
    };
    let content_bytes = decode_content_id(&content_id).unwrap_or([0_u8; 32]);
    if token.is_cancelled() {
        emit_publish_cancelled(context, op_id, &publication_id, true);
        ops.finish(op_id);
        return;
    }
    let key = publication_dht_key(publication_bytes);
    let mut announced = false;
    let mut provider_announced = false;
    let mut error = None;
    match &context.transport {
        Some(transport) => {
            match transport.publish(&key, &publication.record_bytes) {
                Ok(()) => announced = true,
                Err(network) => error = Some(network_domain_error(&network)),
            }
            if !token.is_cancelled() {
                match transport.start_providing(&content_bytes) {
                    Ok(()) => provider_announced = true,
                    Err(network) => {
                        if error.is_none() {
                            error = Some(network_domain_error(&network));
                        }
                    }
                }
            }
        }
        None => {
            error = Some(network_domain_error(&NetError::Disabled));
        }
    }
    if token.is_cancelled() {
        emit_publish_cancelled(context, op_id, &publication_id, true);
        ops.finish(op_id);
        return;
    }
    let now = now_ms();
    let _ = context
        .store
        .mark_publication_announced(&publication_id, announced, now_secs());
    let _ = context
        .store
        .set_publication_availability(&publication_id, PublicationAvailability::Local, Some(now));
    let mut entries = vec![
        (
            Value::Text("publication_id".to_owned()),
            Value::Text(publication_id.clone()),
        ),
        (Value::Text("content_id".to_owned()), Value::Text(content_id)),
        (
            Value::Text("revision".to_owned()),
            Value::Unsigned(publication.revision),
        ),
        (Value::Text("record_verified".to_owned()), Value::Bool(true)),
        (Value::Text("announced".to_owned()), Value::Bool(announced)),
        (
            Value::Text("provider_announced".to_owned()),
            Value::Bool(provider_announced),
        ),
        (Value::Text("serving".to_owned()), Value::Bool(true)),
        (
            Value::Text("availability".to_owned()),
            Value::Text(PublicationAvailability::Local.as_str().to_owned()),
        ),
        (Value::Text("preview".to_owned()), preview_value(&publication.filename)),
    ];
    if let Some(error) = error {
        entries.push((Value::Text("error".to_owned()), error_value(&error)));
    }
    emit_publication_event(&context.events, op_id, "publication.published", Value::Map(entries));
    ops.finish(op_id);
}

/// Publishes a tombstone, withdraws the provider, and reports the limits.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_publication_unpublish(
    context: &PublicationContext,
    ops: &OperationRegistry,
    op_id: u64,
    token: Arc<CancelToken>,
    publication_id: String,
    content_id: String,
    tombstone_id: String,
    revision: u64,
    tombstone: SignedTombstone,
    withdraw_provider: bool,
) {
    if token.is_cancelled() {
        emit_unpublish_cancelled(context, op_id, &publication_id, &tombstone_id);
        ops.finish(op_id);
        return;
    }
    let publication_bytes = decode_content_id(&publication_id).unwrap_or([0_u8; 32]);
    let content_bytes = decode_content_id(&content_id).unwrap_or([0_u8; 32]);
    let key = tombstone_dht_key(publication_bytes);
    let mut announced = false;
    let mut provider_withdrawn = false;
    let mut error = None;
    if let Some(transport) = &context.transport {
        match transport.publish(&key, tombstone.raw()) {
            Ok(()) => announced = true,
            Err(network) => error = Some(network_domain_error(&network)),
        }
        if withdraw_provider {
            if !token.is_cancelled() {
                match transport.stop_providing(&content_bytes) {
                    Ok(()) => provider_withdrawn = true,
                    Err(network) => {
                        if error.is_none() {
                            error = Some(network_domain_error(&network));
                        }
                    }
                }
            }
        } else {
            provider_withdrawn = true;
        }
    } else {
        error = Some(network_domain_error(&NetError::Disabled));
    }
    if token.is_cancelled() {
        emit_unpublish_cancelled(context, op_id, &publication_id, &tombstone_id);
        ops.finish(op_id);
        return;
    }
    let now = now_ms();
    let _ = context
        .store
        .set_publication_availability(&publication_id, PublicationAvailability::RecordOnly, Some(now));
    let mut entries = vec![
        (Value::Text("publication_id".to_owned()), Value::Text(publication_id)),
        (Value::Text("tombstone_id".to_owned()), Value::Text(tombstone_id)),
        (Value::Text("revision".to_owned()), Value::Unsigned(revision)),
        (Value::Text("record_verified".to_owned()), Value::Bool(true)),
        (Value::Text("tombstone_announced".to_owned()), Value::Bool(announced)),
        (
            Value::Text("provider_withdrawn".to_owned()),
            Value::Bool(provider_withdrawn),
        ),
        (Value::Text("serving".to_owned()), Value::Bool(false)),
        (Value::Text("local_bytes_retained".to_owned()), Value::Bool(true)),
        (Value::Text("record_removed".to_owned()), Value::Bool(false)),
        (
            Value::Text("availability".to_owned()),
            Value::Text(PublicationAvailability::RecordOnly.as_str().to_owned()),
        ),
        (
            Value::Text("limits".to_owned()),
            Value::Text(UNPUBLISH_LIMITS.to_owned()),
        ),
    ];
    if let Some(error) = error {
        entries.push((Value::Text("error".to_owned()), error_value(&error)));
    }
    emit_publication_event(&context.events, op_id, "publication.unpublished", Value::Map(entries));
    ops.finish(op_id);
}

/// Looks up one signed publication, caches it, and probes providers.
pub(crate) fn run_publication_lookup(
    context: &PublicationContext,
    ops: &OperationRegistry,
    op_id: u64,
    token: Arc<CancelToken>,
    publication_id: String,
) {
    let publication_bytes = match decode_content_id(&publication_id) {
        Some(bytes) => bytes,
        None => {
            emit_lookup_failed(
                context,
                op_id,
                &publication_id,
                DomainError::invalid("The publication ID is malformed."),
            );
            ops.finish(op_id);
            return;
        }
    };
    if token.is_cancelled() {
        emit_lookup_cancelled(context, op_id, &publication_id);
        ops.finish(op_id);
        return;
    }
    let key = publication_dht_key(publication_bytes);
    let result = match &context.transport {
        Some(transport) => transport.lookup(&key),
        None => Err(NetError::Disabled),
    };
    if token.is_cancelled() {
        emit_lookup_cancelled(context, op_id, &publication_id);
        ops.finish(op_id);
        return;
    }
    match result {
        Ok(Some(raw)) => handle_lookup_record(context, op_id, &publication_id, &key, raw),
        Ok(None) => handle_lookup_missing(context, op_id, &publication_id, None),
        Err(error) => {
            let domain = network_domain_error(&error);
            handle_lookup_missing(context, op_id, &publication_id, Some(&domain));
        }
    }
    ops.finish(op_id);
}

fn handle_lookup_record(context: &PublicationContext, op_id: u64, publication_id: &str, key: &[u8; 32], raw: Vec<u8>) {
    let verified = SignedPublication::parse(&raw).and_then(|signed| {
        if to_hex(&signed.publication_id()) != publication_id {
            return Err(ProtocolError::DhtKeyMismatch);
        }
        if publication_dht_key(signed.publication_id()) != *key {
            return Err(ProtocolError::DhtKeyMismatch);
        }
        Ok(signed)
    });
    let signed = match verified {
        Ok(signed) => signed,
        Err(error) => {
            emit_lookup_failed(context, op_id, publication_id, protocol_domain_error(&error));
            return;
        }
    };
    let tombstone = match lookup_tombstone(context, &signed) {
        Ok(tombstone) => tombstone,
        Err(error) => {
            emit_lookup_failed(context, op_id, publication_id, error);
            return;
        }
    };
    if let Some(tombstone) = &tombstone {
        let body = tombstone.tombstone();
        let stored = TombstoneRecord {
            publication_id: publication_id.to_owned(),
            identity_id: body.identity_id.as_str().to_owned(),
            content_id: to_hex(&body.content_id),
            revision: body.revision,
            record_bytes: tombstone.raw().to_vec(),
            created_at: body.unpublished_at_ms / 1_000,
        };
        if let Err(error) = context.store.upsert_tombstone(&stored) {
            emit_lookup_failed(context, op_id, publication_id, store_domain_error(error));
            return;
        }
    }
    let now = now_ms();
    let cached_local = context
        .store
        .publication(publication_id)
        .ok()
        .filter(|record| record.local);
    if let Some(record) = cached_local {
        let providers = lookup_providers(context, &record.content_id);
        let availability = effective_availability(&record);
        emit_lookup_completed(
            context,
            op_id,
            publication_id,
            Some(&record),
            "local",
            availability,
            false,
            false,
            &providers,
            None,
        );
        return;
    }
    let content_id = to_hex(&signed.publication().content_id);
    let local_content = context
        .store
        .content(&content_id)
        .ok()
        .is_some_and(|content| content.state == nexus_store::ContentState::Verified);
    let providers = if local_content || tombstone.is_some() {
        Vec::new()
    } else {
        lookup_providers(context, &content_id)
    };
    let previous = context.store.publication(publication_id).ok();
    let availability = if tombstone.is_some() {
        PublicationAvailability::RecordOnly
    } else if local_content {
        PublicationAvailability::Local
    } else if !providers.is_empty() {
        PublicationAvailability::AvailableNow
    } else if previous.as_ref().is_some_and(|record| {
        record.availability == PublicationAvailability::AvailableNow
            && record
                .last_checked_at
                .is_some_and(|checked| now.saturating_sub(checked) <= PUBLICATION_AVAILABILITY_STALE_MS)
    }) {
        PublicationAvailability::SeenRecently
    } else {
        PublicationAvailability::NoKnownSource
    };
    let record = remote_record_from_signed(&signed, availability, now);
    let outcome = match context.store.cache_remote_publication(&record) {
        Ok(outcome) => outcome,
        Err(error) => {
            emit_lookup_failed(context, op_id, publication_id, store_domain_error(error));
            return;
        }
    };
    if outcome == PublicationCacheOutcome::Conflict {
        let cached = context.store.publication(publication_id).ok();
        emit_lookup_completed(
            context,
            op_id,
            publication_id,
            cached.as_ref(),
            "cache",
            cached
                .as_ref()
                .map_or(PublicationAvailability::RecordOnly, effective_availability),
            false,
            true,
            &providers,
            Some(&DomainError::new(
                DomainCode::Conflict,
                "A different record is already cached for this publication ID.",
            )),
        );
        return;
    }
    if let Err(error) = context
        .store
        .set_publication_availability(publication_id, availability, Some(now))
    {
        emit_lookup_failed(context, op_id, publication_id, store_domain_error(error));
        return;
    }
    let cached = context.store.publication(publication_id).ok();
    emit_lookup_completed(
        context,
        op_id,
        publication_id,
        cached.as_ref(),
        "network",
        availability,
        false,
        false,
        &providers,
        None,
    );
}

/// Looks up and verifies a signed tombstone for a publication.
///
/// A missing tombstone is not an error. Malformed or mismatched tombstone bytes
/// are rejected instead of being cached.
fn lookup_tombstone(
    context: &PublicationContext,
    signed: &SignedPublication,
) -> Result<Option<SignedTombstone>, DomainError> {
    let Some(transport) = &context.transport else {
        return Ok(None);
    };
    let key = tombstone_dht_key(signed.publication_id());
    let raw = match transport.lookup(&key) {
        Ok(Some(raw)) => raw,
        Ok(None) | Err(_) => return Ok(None),
    };
    let tombstone = SignedTombstone::parse(&raw).map_err(|error| protocol_domain_error(&error))?;
    tombstone
        .validate_for(signed)
        .map_err(|error| protocol_domain_error(&error))?;
    Ok(Some(tombstone))
}

fn handle_lookup_missing(context: &PublicationContext, op_id: u64, publication_id: &str, error: Option<&DomainError>) {
    let now = now_ms();
    match context.store.publication(publication_id) {
        Ok(mut record) => {
            let availability = if record.local {
                PublicationAvailability::Local
            } else if record.availability == PublicationAvailability::AvailableNow {
                PublicationAvailability::SeenRecently
            } else if context
                .store
                .content(&record.content_id)
                .ok()
                .is_some_and(|content| content.state == nexus_store::ContentState::Verified)
            {
                PublicationAvailability::Local
            } else {
                PublicationAvailability::NoKnownSource
            };
            let _ = context
                .store
                .set_publication_availability(publication_id, availability, Some(now));
            record.availability = availability;
            record.last_checked_at = Some(now);
            emit_lookup_completed(
                context,
                op_id,
                publication_id,
                Some(&record),
                if record.local { "local" } else { "cache" },
                availability,
                !record.local,
                false,
                &[],
                error,
            );
        }
        Err(_) => {
            let mut entries = vec![
                (
                    Value::Text("publication_id".to_owned()),
                    Value::Text(publication_id.to_owned()),
                ),
                (Value::Text("found".to_owned()), Value::Bool(false)),
                (Value::Text("source".to_owned()), Value::Text("network".to_owned())),
                (
                    Value::Text("availability".to_owned()),
                    Value::Text(PublicationAvailability::RecordOnly.as_str().to_owned()),
                ),
                (Value::Text("stale".to_owned()), Value::Bool(false)),
                (Value::Text("conflict".to_owned()), Value::Bool(false)),
                (Value::Text("record".to_owned()), Value::Null),
                (Value::Text("providers".to_owned()), Value::Array(Vec::new())),
                (Value::Text("checked_at_ms".to_owned()), Value::Unsigned(now)),
            ];
            if let Some(error) = error {
                entries.push((Value::Text("error".to_owned()), error_value(error)));
            }
            emit_publication_event(
                &context.events,
                op_id,
                "publication.lookup_completed",
                Value::Map(entries),
            );
        }
    }
}

fn lookup_providers(context: &PublicationContext, content_id: &str) -> Vec<DiscoveredPeer> {
    let Some(bytes) = decode_content_id(content_id) else {
        return Vec::new();
    };
    let Some(transport) = &context.transport else {
        return Vec::new();
    };
    transport.find_providers(&bytes).unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
fn emit_lookup_completed(
    context: &PublicationContext,
    op_id: u64,
    publication_id: &str,
    record: Option<&PublicationRecord>,
    source: &str,
    availability: PublicationAvailability,
    stale: bool,
    conflict: bool,
    providers: &[DiscoveredPeer],
    error: Option<&DomainError>,
) {
    let tombstone = record.and_then(|record| context.store.tombstone(&record.publication_id).ok().flatten());
    let mut entries = vec![
        (
            Value::Text("publication_id".to_owned()),
            Value::Text(publication_id.to_owned()),
        ),
        (Value::Text("found".to_owned()), Value::Bool(record.is_some())),
        (Value::Text("source".to_owned()), Value::Text(source.to_owned())),
        (
            Value::Text("availability".to_owned()),
            Value::Text(availability.as_str().to_owned()),
        ),
        (Value::Text("stale".to_owned()), Value::Bool(stale)),
        (Value::Text("conflict".to_owned()), Value::Bool(conflict)),
        (
            Value::Text("record".to_owned()),
            record.map_or(Value::Null, |record| publication_value(record, tombstone.as_ref())),
        ),
        (
            Value::Text("providers".to_owned()),
            Value::Array(providers.iter().map(provider_value).collect()),
        ),
        (Value::Text("checked_at_ms".to_owned()), Value::Unsigned(now_ms())),
    ];
    if let Some(error) = error {
        entries.push((Value::Text("error".to_owned()), error_value(error)));
    }
    emit_publication_event(
        &context.events,
        op_id,
        "publication.lookup_completed",
        Value::Map(entries),
    );
}

fn emit_publish_cancelled(context: &PublicationContext, op_id: u64, publication_id: &str, serving: bool) {
    emit_publication_event(
        &context.events,
        op_id,
        "publication.publish_cancelled",
        Value::Map(vec![
            (
                Value::Text("publication_id".to_owned()),
                Value::Text(publication_id.to_owned()),
            ),
            (Value::Text("serving".to_owned()), Value::Bool(serving)),
        ]),
    );
}

fn emit_unpublish_cancelled(context: &PublicationContext, op_id: u64, publication_id: &str, tombstone_id: &str) {
    emit_publication_event(
        &context.events,
        op_id,
        "publication.unpublish_cancelled",
        Value::Map(vec![
            (
                Value::Text("publication_id".to_owned()),
                Value::Text(publication_id.to_owned()),
            ),
            (
                Value::Text("tombstone_id".to_owned()),
                Value::Text(tombstone_id.to_owned()),
            ),
            (Value::Text("local_bytes_retained".to_owned()), Value::Bool(true)),
        ]),
    );
}

fn emit_lookup_cancelled(context: &PublicationContext, op_id: u64, publication_id: &str) {
    emit_publication_event(
        &context.events,
        op_id,
        "publication.lookup_cancelled",
        Value::Map(vec![(
            Value::Text("publication_id".to_owned()),
            Value::Text(publication_id.to_owned()),
        )]),
    );
}

fn emit_lookup_failed(context: &PublicationContext, op_id: u64, publication_id: &str, error: DomainError) {
    emit_publication_event(
        &context.events,
        op_id,
        "publication.lookup_failed",
        Value::Map(vec![
            (
                Value::Text("publication_id".to_owned()),
                Value::Text(publication_id.to_owned()),
            ),
            (Value::Text("error".to_owned()), error_value(&error)),
        ]),
    );
}

fn emit_publication_event(events: &EventQueue, op_id: u64, event_type: &str, data: Value) {
    let _ = events.push(PendingEvent::durable(event_type, data).with_op(op_id));
}

fn remote_record_from_signed(
    signed: &SignedPublication,
    availability: PublicationAvailability,
    checked_at: u64,
) -> PublicationRecord {
    let body = signed.publication();
    PublicationRecord {
        publication_id: to_hex(&signed.publication_id()),
        identity_id: body.identity_id.as_str().to_owned(),
        content_id: to_hex(&body.content_id),
        revision: body.revision,
        record_bytes: signed.raw().to_vec(),
        title: body.display_filename.clone(),
        description: if body.description.is_empty() {
            None
        } else {
            Some(body.description.clone())
        },
        filename: nexus_store::safe_display_name(&body.display_filename),
        published_at: body.published_at_ms / 1_000,
        updated_at: now_secs(),
        tombstoned: false,
        category: body.category,
        size_bytes: body.size,
        tags: body.tags.clone(),
        local: false,
        serving: false,
        announced: false,
        availability,
        last_checked_at: Some(checked_at),
    }
}

fn publication_value(record: &PublicationRecord, tombstone: Option<&TombstoneRecord>) -> Value {
    Value::Map(vec![
        (
            Value::Text("publication_id".to_owned()),
            Value::Text(record.publication_id.clone()),
        ),
        (
            Value::Text("identity_id".to_owned()),
            Value::Text(record.identity_id.clone()),
        ),
        (
            Value::Text("content_id".to_owned()),
            Value::Text(record.content_id.clone()),
        ),
        (Value::Text("revision".to_owned()), Value::Unsigned(record.revision)),
        (Value::Text("title".to_owned()), Value::Text(record.title.clone())),
        (
            Value::Text("description".to_owned()),
            record
                .description
                .as_ref()
                .map_or(Value::Null, |description| Value::Text(description.clone())),
        ),
        (Value::Text("filename".to_owned()), Value::Text(record.filename.clone())),
        (
            Value::Text("category".to_owned()),
            Value::Text(record.category.as_str().to_owned()),
        ),
        (Value::Text("size_bytes".to_owned()), Value::Unsigned(record.size_bytes)),
        (
            Value::Text("tags".to_owned()),
            Value::Array(record.tags.iter().cloned().map(Value::Text).collect()),
        ),
        (
            Value::Text("published_at_ms".to_owned()),
            Value::Unsigned(record.published_at.saturating_mul(1_000)),
        ),
        (
            Value::Text("updated_at_ms".to_owned()),
            Value::Unsigned(record.updated_at.saturating_mul(1_000)),
        ),
        (Value::Text("tombstoned".to_owned()), Value::Bool(record.tombstoned)),
        (Value::Text("local".to_owned()), Value::Bool(record.local)),
        (
            Value::Text("serving".to_owned()),
            Value::Bool(record.serving && !record.tombstoned),
        ),
        (Value::Text("announced".to_owned()), Value::Bool(record.announced)),
        (
            Value::Text("availability".to_owned()),
            Value::Text(effective_availability(record).as_str().to_owned()),
        ),
        (
            Value::Text("signature_verified".to_owned()),
            Value::Bool(signature_verified(record)),
        ),
        (Value::Text("preview".to_owned()), preview_value(&record.filename)),
        (
            Value::Text("tombstone".to_owned()),
            tombstone.map_or(Value::Null, |tombstone| tombstone_value(record, tombstone)),
        ),
    ])
}

fn publication_summary_value(record: &PublicationRecord) -> Value {
    Value::Map(vec![
        (
            Value::Text("publication_id".to_owned()),
            Value::Text(record.publication_id.clone()),
        ),
        (
            Value::Text("content_id".to_owned()),
            Value::Text(record.content_id.clone()),
        ),
        (Value::Text("title".to_owned()), Value::Text(record.title.clone())),
        (Value::Text("filename".to_owned()), Value::Text(record.filename.clone())),
        (
            Value::Text("category".to_owned()),
            Value::Text(record.category.as_str().to_owned()),
        ),
        (Value::Text("size_bytes".to_owned()), Value::Unsigned(record.size_bytes)),
        (
            Value::Text("tags".to_owned()),
            Value::Array(record.tags.iter().cloned().map(Value::Text).collect()),
        ),
        (Value::Text("revision".to_owned()), Value::Unsigned(record.revision)),
        (
            Value::Text("published_at_ms".to_owned()),
            Value::Unsigned(record.published_at.saturating_mul(1_000)),
        ),
        (
            Value::Text("serving".to_owned()),
            Value::Bool(record.serving && !record.tombstoned),
        ),
        (Value::Text("announced".to_owned()), Value::Bool(record.announced)),
        (Value::Text("tombstoned".to_owned()), Value::Bool(record.tombstoned)),
        (
            Value::Text("availability".to_owned()),
            Value::Text(effective_availability(record).as_str().to_owned()),
        ),
        (
            Value::Text("signature_verified".to_owned()),
            Value::Bool(signature_verified(record)),
        ),
        (Value::Text("preview".to_owned()), preview_value(&record.filename)),
    ])
}

fn tombstone_value(record: &PublicationRecord, tombstone: &TombstoneRecord) -> Value {
    let signature_verified = SignedTombstone::parse(&tombstone.record_bytes).is_ok_and(|signed_tombstone| {
        SignedPublication::parse(&record.record_bytes)
            .is_ok_and(|signed_publication| signed_tombstone.validate_for(&signed_publication).is_ok())
    });
    Value::Map(vec![
        (
            Value::Text("tombstone_id".to_owned()),
            Value::Text(stored_tombstone_id(tombstone)),
        ),
        (
            Value::Text("publication_id".to_owned()),
            Value::Text(tombstone.publication_id.clone()),
        ),
        (Value::Text("revision".to_owned()), Value::Unsigned(tombstone.revision)),
        (
            Value::Text("unpublished_at_ms".to_owned()),
            Value::Unsigned(tombstone.created_at.saturating_mul(1_000)),
        ),
        (
            Value::Text("signature_verified".to_owned()),
            Value::Bool(signature_verified),
        ),
        (
            Value::Text("limits".to_owned()),
            Value::Text(UNPUBLISH_LIMITS.to_owned()),
        ),
    ])
}

fn stored_tombstone_id(tombstone: &TombstoneRecord) -> String {
    format!("{}:{}", tombstone.publication_id, tombstone.revision)
}

fn signature_verified(record: &PublicationRecord) -> bool {
    SignedPublication::parse(&record.record_bytes)
        .is_ok_and(|signed| to_hex(&signed.publication_id()) == record.publication_id)
}

fn effective_availability(record: &PublicationRecord) -> PublicationAvailability {
    if record.tombstoned {
        return PublicationAvailability::RecordOnly;
    }
    if record.local && record.serving {
        return PublicationAvailability::Local;
    }
    record.availability
}

fn preview_value(filename: &str) -> Value {
    let extension = filename
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default();
    let (kind, reason) = if TEXT_EXTENSIONS.contains(&extension.as_str()) {
        ("text", "plain_text")
    } else if IMAGE_EXTENSIONS.contains(&extension.as_str()) {
        ("image", "raster_image")
    } else {
        ("none", "unsupported_format")
    };
    Value::Map(vec![
        (Value::Text("kind".to_owned()), Value::Text(kind.to_owned())),
        (Value::Text("supported".to_owned()), Value::Bool(kind != "none")),
        (Value::Text("auto_execute".to_owned()), Value::Bool(false)),
        (Value::Text("reason".to_owned()), Value::Text(reason.to_owned())),
    ])
}

fn provider_value(provider: &DiscoveredPeer) -> Value {
    Value::Map(vec![
        (Value::Text("peer_id".to_owned()), Value::Text(provider.peer_id.clone())),
        (
            Value::Text("addresses".to_owned()),
            Value::Array(provider.addresses.iter().cloned().map(Value::Text).collect()),
        ),
    ])
}

fn locked_error(reason: &str) -> DomainError {
    DomainError::new(DomainCode::Locked, format!("The identity is locked ({reason})."))
}

fn protocol_domain_error(error: &ProtocolError) -> DomainError {
    let code = match error {
        ProtocolError::UnsupportedVersion(_) => DomainCode::UnsupportedVersion,
        ProtocolError::IdentityKeyMismatch | ProtocolError::DhtKeyMismatch | ProtocolError::TombstoneMismatch => {
            DomainCode::VerificationFailed
        }
        _ => DomainCode::InvalidArgument,
    };
    DomainError::new(code, error.to_string())
}

fn store_domain_error(error: nexus_store::StoreError) -> DomainError {
    DomainError::from(CoreError::Store(error))
}

fn map_get<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value
        .as_map()
        .and_then(|entries| entries.iter().find(|(name, _)| name.as_text() == Some(key)))
        .map(|(_, value)| value)
}

fn optional_text(value: &Value, key: &str) -> Result<Option<String>, CoreError> {
    match map_get(value, key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Text(text)) => Ok(Some(text.clone())),
        Some(_) => Err(CoreError::InvalidArgument("a text field has the wrong type")),
    }
}

fn optional_unsigned(value: &Value, key: &str) -> Result<Option<u64>, CoreError> {
    match map_get(value, key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_unsigned()
            .map(Some)
            .ok_or(CoreError::InvalidArgument("an unsigned field has the wrong type")),
    }
}

fn optional_content_id(value: &Value, key: &str) -> Result<Option<[u8; 32]>, CoreError> {
    match optional_text(value, key)? {
        None => Ok(None),
        Some(text) => decode_content_id(&text)
            .map(Some)
            .ok_or(CoreError::InvalidArgument("a content ID must be 32-byte hex")),
    }
}

fn decode_content_id(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 {
        return None;
    }
    let mut bytes = [0_u8; 32];
    for (index, pair) in text.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let pair = std::str::from_utf8(pair).ok()?;
        bytes[index] = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(bytes)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
}

fn now_secs() -> u64 {
    now_ms() / 1_000
}

#[cfg(test)]
mod tests {
    use super::{PublicationCreate, preview_value};
    use nexus_crypto::Value;
    use nexus_protocol::Category;

    fn map(entries: Vec<(&str, Value)>) -> Value {
        Value::Map(
            entries
                .into_iter()
                .map(|(key, value)| (Value::Text(key.to_owned()), value))
                .collect(),
        )
    }

    fn get<'a>(value: &'a Value, key: &str) -> &'a Value {
        value
            .as_map()
            .and_then(|entries| entries.iter().find(|(name, _)| name.as_text() == Some(key)))
            .map(|(_, value)| value)
            .unwrap_or_else(|| panic!("missing field {key}"))
    }

    fn text(value: &Value) -> &str {
        value.as_text().expect("text")
    }

    fn boolean(value: &Value) -> bool {
        value.as_bool().expect("bool")
    }

    #[test]
    fn previews_only_inert_text_and_raster_images() {
        let preview = preview_value("notes.txt");
        assert_eq!(text(get(&preview, "kind")), "text");
        assert!(boolean(get(&preview, "supported")));
        assert!(!boolean(get(&preview, "auto_execute")));
        assert_eq!(text(get(&preview, "reason")), "plain_text");

        let image = preview_value("photo.PNG");
        assert_eq!(text(get(&image, "kind")), "image");
        assert!(boolean(get(&image, "supported")));
        assert!(!boolean(get(&image, "auto_execute")));
        assert_eq!(text(get(&image, "reason")), "raster_image");
    }

    #[test]
    fn preview_rejects_markup_executables_scripts_and_unknown_formats() {
        for filename in [
            "diagram.svg",
            "page.html",
            "page.htm",
            "setup.exe",
            "installer.msi",
            "script.ps1",
            "script.bat",
            "script.sh",
            "archive.zip",
            "no-extension",
        ] {
            let preview = preview_value(filename);
            assert_eq!(text(get(&preview, "kind")), "none", "{filename}");
            assert!(!boolean(get(&preview, "supported")), "{filename}");
            assert!(!boolean(get(&preview, "auto_execute")), "{filename}");
            assert_eq!(text(get(&preview, "reason")), "unsupported_format", "{filename}");
        }
    }

    #[test]
    fn create_defaults_category_and_deduplicates_tags_in_order() {
        let create = PublicationCreate::parse(&map(vec![
            ("content_id", Value::Text("a".repeat(64))),
            ("display_filename", Value::Text("notes.txt".to_owned())),
            (
                "tags",
                Value::Array(vec![
                    Value::Text("notes".to_owned()),
                    Value::Text("local-first".to_owned()),
                    Value::Text("notes".to_owned()),
                ]),
            ),
        ]))
        .expect("parse create");
        assert_eq!(create.category, Category::Other);
        assert_eq!(create.tags, vec!["notes".to_owned(), "local-first".to_owned()]);
        assert_eq!(create.description, "");
    }

    #[test]
    fn create_rejects_unknown_categories_bad_ids_and_bad_tags() {
        assert!(
            PublicationCreate::parse(&map(vec![
                ("content_id", Value::Text("a".repeat(64))),
                ("display_filename", Value::Text("notes.txt".to_owned())),
                ("category", Value::Text("unknown".to_owned())),
            ]))
            .is_err()
        );
        assert!(
            PublicationCreate::parse(&map(vec![
                ("content_id", Value::Text("short".to_owned())),
                ("display_filename", Value::Text("notes.txt".to_owned())),
            ]))
            .is_err()
        );
        assert!(
            PublicationCreate::parse(&map(vec![
                ("content_id", Value::Text("a".repeat(64))),
                ("display_filename", Value::Text("notes.txt".to_owned())),
                (
                    "tags",
                    Value::Array((0..11).map(|index| Value::Text(format!("tag{index}"))).collect()),
                ),
            ]))
            .is_err()
        );
        assert!(
            PublicationCreate::parse(&map(vec![
                ("content_id", Value::Text("a".repeat(64))),
                ("display_filename", Value::Text("notes.txt".to_owned())),
                ("tags", Value::Array(vec![Value::Text(String::new())])),
            ]))
            .is_err()
        );
    }
}

#[cfg(test)]
mod worker_tests {
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use nexus_crypto::{IdentityId, KeyPair, to_hex};
    use nexus_net::{DiscoveredPeer, NetError};
    use nexus_protocol::{
        Category, PublicationV1, SignedPublication, SignedTombstone, TombstoneV1, publication_dht_key,
        tombstone_dht_key,
    };
    use nexus_store::{PublicationAvailability, PublicationRecord, QuotaPolicy, Store, StoreConfig, safe_display_name};

    use super::*;
    use crate::event::EventQueue;
    use crate::ops::OperationRegistry;

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new(label: &str) -> Self {
            let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "nexus-core-publication-{label}-{}-{sequence}",
                std::process::id()
            ));
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

    fn store(root: &TempRoot) -> Arc<Store> {
        Arc::new(
            Store::open_with_config(
                &root.path,
                StoreConfig {
                    quota: QuotaPolicy::custom(16 * 1024 * 1024, 0).expect("test quota"),
                    max_file_size: 8 * 1024 * 1024,
                    ..StoreConfig::default()
                },
            )
            .expect("open store"),
        )
    }

    fn import(store: &Store, name: &str, bytes: &[u8]) -> (String, u64) {
        let staged = store.temporary_directory().join(name);
        fs::write(&staged, bytes).expect("stage bytes");
        let imported = store
            .import_local_staged(
                &staged,
                &to_hex(&nexus_crypto::sha256(bytes)),
                u64::try_from(bytes.len()).expect("size"),
                name,
            )
            .expect("import staged");
        (imported.content_id, imported.size_bytes)
    }

    fn signed_publication(content_id: &str, size: u64, filename: &str, keypair: &KeyPair) -> SignedPublication {
        let publication = PublicationV1::new(
            IdentityId::from_public_key(&keypair.public_key()),
            decode_content_id(content_id).expect("content id"),
            size,
            filename,
            "worker test",
            Category::Documents,
            [0x44_u8; 32],
            1,
            1_700_000_000_000,
            "0.1.0-test",
        )
        .with_tags(vec!["worker".to_owned()]);
        SignedPublication::sign(&publication, keypair).expect("sign publication")
    }

    fn publication_record(signed: &SignedPublication, content_id: &str, local: bool) -> PublicationRecord {
        let body = signed.publication();
        PublicationRecord {
            publication_id: to_hex(&signed.publication_id()),
            identity_id: body.identity_id.as_str().to_owned(),
            content_id: content_id.to_owned(),
            revision: body.revision,
            record_bytes: signed.raw().to_vec(),
            title: body.display_filename.clone(),
            description: Some(body.description.clone()),
            filename: safe_display_name(&body.display_filename),
            published_at: 1_700_000_000,
            updated_at: 1_700_000_000,
            tombstoned: false,
            category: body.category,
            size_bytes: body.size,
            tags: body.tags.clone(),
            local,
            serving: local,
            announced: false,
            availability: if local {
                PublicationAvailability::Local
            } else {
                PublicationAvailability::NoKnownSource
            },
            last_checked_at: None,
        }
    }

    struct FakeTransport {
        records: Mutex<HashMap<Vec<u8>, Vec<u8>>>,
        published: Mutex<Vec<(Vec<u8>, Vec<u8>)>>,
        started: Mutex<Vec<Vec<u8>>>,
        stopped: Mutex<Vec<Vec<u8>>>,
        providers: Vec<DiscoveredPeer>,
        publish_ok: bool,
        provider_ok: bool,
    }

    impl FakeTransport {
        fn new() -> Self {
            Self {
                records: Mutex::new(HashMap::new()),
                published: Mutex::new(Vec::new()),
                started: Mutex::new(Vec::new()),
                stopped: Mutex::new(Vec::new()),
                providers: Vec::new(),
                publish_ok: true,
                provider_ok: true,
            }
        }

        fn with_record(mut self, key: [u8; 32], value: &[u8]) -> Self {
            self.records
                .get_mut()
                .expect("records")
                .insert(key.to_vec(), value.to_vec());
            self
        }

        fn with_providers(mut self, providers: Vec<DiscoveredPeer>) -> Self {
            self.providers = providers;
            self
        }
    }

    impl PublicationTransport for FakeTransport {
        fn publish(&self, key: &[u8], value: &[u8]) -> Result<(), NetError> {
            if !self.publish_ok {
                return Err(NetError::Unavailable);
            }
            self.published
                .lock()
                .expect("published")
                .push((key.to_vec(), value.to_vec()));
            self.records
                .lock()
                .expect("records")
                .insert(key.to_vec(), value.to_vec());
            Ok(())
        }

        fn lookup(&self, key: &[u8]) -> Result<Option<Vec<u8>>, NetError> {
            Ok(self.records.lock().expect("records").get(key).cloned())
        }

        fn start_providing(&self, content_id: &[u8]) -> Result<(), NetError> {
            if !self.provider_ok {
                return Err(NetError::Unavailable);
            }
            self.started.lock().expect("started").push(content_id.to_vec());
            Ok(())
        }

        fn stop_providing(&self, content_id: &[u8]) -> Result<(), NetError> {
            if !self.provider_ok {
                return Err(NetError::Unavailable);
            }
            self.stopped.lock().expect("stopped").push(content_id.to_vec());
            Ok(())
        }

        fn find_providers(&self, _content_id: &[u8]) -> Result<Vec<DiscoveredPeer>, NetError> {
            Ok(self.providers.clone())
        }
    }

    fn event_field<'a>(event: &'a crate::event::Event, key: &str) -> Option<&'a Value> {
        map_get(&event.data, key)
    }

    fn next_event(events: &EventQueue) -> crate::event::Event {
        events
            .next_event(Duration::from_secs(1))
            .expect("event")
            .expect("queued")
    }

    #[test]
    fn publish_worker_announces_record_and_provider() {
        let root = TempRoot::new("publish");
        let store = store(&root);
        let keypair = KeyPair::from_seed([81_u8; 32]);
        let (content_id, size) = import(&store, "worker.txt", b"publication worker bytes");
        let signed = signed_publication(&content_id, size, "worker.txt", &keypair);
        let record = publication_record(&signed, &content_id, true);
        store.upsert_publication(&record).expect("publication");

        let events = Arc::new(EventQueue::new());
        let ops = Arc::new(OperationRegistry::new());
        let transport = Arc::new(FakeTransport::new());
        let context = PublicationContext {
            store: Arc::clone(&store),
            events: Arc::clone(&events),
            transport: Some(Arc::clone(&transport) as Arc<dyn PublicationTransport>),
        };
        let (op_id, token) = ops.begin();
        run_publication_publish(
            &context,
            &ops,
            op_id,
            token,
            record.publication_id.clone(),
            content_id.clone(),
        );

        let event = next_event(&events);
        assert_eq!(event.event_type, "publication.published");
        assert_eq!(event.op_id, Some(op_id));
        assert_eq!(
            event_field(&event, "record_verified").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(event_field(&event, "announced").and_then(Value::as_bool), Some(true));
        assert_eq!(
            event_field(&event, "provider_announced").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(event_field(&event, "serving").and_then(Value::as_bool), Some(true));
        assert_eq!(
            event_field(&event, "availability").and_then(Value::as_text),
            Some("local")
        );
        assert!(event_field(&event, "error").is_none());
        let published = transport.published.lock().expect("published");
        assert_eq!(published.len(), 1);
        assert_eq!(published[0].0, publication_dht_key(signed.publication_id()));
        assert_eq!(published[0].1, signed.raw());
        drop(published);
        assert_eq!(
            transport.started.lock().expect("started").as_slice(),
            &[decode_content_id(&content_id).expect("content id").to_vec()]
        );

        let stored = store.publication(&record.publication_id).expect("stored");
        assert!(stored.announced);
        assert_eq!(stored.availability, PublicationAvailability::Local);
        assert!(stored.last_checked_at.is_some());
    }

    #[test]
    fn publish_worker_defers_without_a_transport_but_keeps_serving() {
        let root = TempRoot::new("publish-deferred");
        let store = store(&root);
        let keypair = KeyPair::from_seed([82_u8; 32]);
        let (content_id, size) = import(&store, "deferred.txt", b"deferred publication bytes");
        let signed = signed_publication(&content_id, size, "deferred.txt", &keypair);
        let record = publication_record(&signed, &content_id, true);
        store.upsert_publication(&record).expect("publication");

        let events = Arc::new(EventQueue::new());
        let ops = Arc::new(OperationRegistry::new());
        let context = PublicationContext {
            store: Arc::clone(&store),
            events: Arc::clone(&events),
            transport: None,
        };
        let (op_id, token) = ops.begin();
        run_publication_publish(&context, &ops, op_id, token, record.publication_id.clone(), content_id);

        let event = next_event(&events);
        assert_eq!(event.event_type, "publication.published");
        assert_eq!(event.op_id, Some(op_id));
        assert_eq!(event_field(&event, "announced").and_then(Value::as_bool), Some(false));
        assert_eq!(
            event_field(&event, "provider_announced").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(event_field(&event, "serving").and_then(Value::as_bool), Some(true));
        assert!(event_field(&event, "error").is_some());
        let stored = store.publication(&record.publication_id).expect("stored");
        assert!(!stored.announced);
        assert_eq!(stored.availability, PublicationAvailability::Local);
    }

    #[test]
    fn unpublish_worker_announces_tombstone_and_withdraws_provider() {
        let root = TempRoot::new("unpublish");
        let store = store(&root);
        let keypair = KeyPair::from_seed([83_u8; 32]);
        let (content_id, size) = import(&store, "withdraw.txt", b"withdraw publication bytes");
        let signed = signed_publication(&content_id, size, "withdraw.txt", &keypair);
        let record = publication_record(&signed, &content_id, true);
        store.upsert_publication(&record).expect("publication");
        let tombstone = SignedTombstone::sign(
            &TombstoneV1::new(
                signed.publication().identity_id.clone(),
                signed.publication_id(),
                decode_content_id(&content_id).expect("content id"),
                1,
                1_700_000_100_000,
            ),
            &keypair,
        )
        .expect("sign tombstone");
        let tombstone_record = TombstoneRecord {
            publication_id: record.publication_id.clone(),
            identity_id: signed.publication().identity_id.as_str().to_owned(),
            content_id: content_id.clone(),
            revision: 1,
            record_bytes: tombstone.raw().to_vec(),
            created_at: 1_700_000_100,
        };
        store.upsert_tombstone(&tombstone_record).expect("tombstone");
        store
            .mark_publication_unpublished(&record.publication_id, 1_700_000_100)
            .expect("mark unpublished");

        let events = Arc::new(EventQueue::new());
        let ops = Arc::new(OperationRegistry::new());
        let transport = Arc::new(FakeTransport::new());
        let context = PublicationContext {
            store: Arc::clone(&store),
            events: Arc::clone(&events),
            transport: Some(Arc::clone(&transport) as Arc<dyn PublicationTransport>),
        };
        let (op_id, token) = ops.begin();
        let tombstone_id = format!("{}:1", record.publication_id);
        run_publication_unpublish(
            &context,
            &ops,
            op_id,
            token,
            record.publication_id.clone(),
            content_id.clone(),
            tombstone_id.clone(),
            1,
            tombstone.clone(),
            true,
        );

        let event = next_event(&events);
        assert_eq!(event.event_type, "publication.unpublished");
        assert_eq!(event.op_id, Some(op_id));
        assert_eq!(
            event_field(&event, "tombstone_id").and_then(Value::as_text),
            Some(tombstone_id.as_str())
        );
        assert_eq!(
            event_field(&event, "tombstone_announced").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            event_field(&event, "provider_withdrawn").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(event_field(&event, "serving").and_then(Value::as_bool), Some(false));
        assert_eq!(
            event_field(&event, "local_bytes_retained").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            event_field(&event, "record_removed").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            event_field(&event, "availability").and_then(Value::as_text),
            Some("record_only")
        );
        assert!(event_field(&event, "limits").and_then(Value::as_text).is_some());

        let published = transport.published.lock().expect("published");
        assert_eq!(published.len(), 1);
        assert_eq!(published[0].0, tombstone_dht_key(signed.publication_id()));
        assert_eq!(published[0].1, tombstone.raw());
        drop(published);
        assert_eq!(
            transport.stopped.lock().expect("stopped").as_slice(),
            &[decode_content_id(&content_id).expect("content id").to_vec()]
        );
        let stored = store.publication(&record.publication_id).expect("stored");
        assert_eq!(stored.availability, PublicationAvailability::RecordOnly);
        assert!(!stored.serving);
    }

    #[test]
    fn lookup_caches_a_verified_record_and_signed_tombstone() {
        let root = TempRoot::new("lookup-tombstone");
        let store = store(&root);
        let keypair = KeyPair::from_seed([84_u8; 32]);
        let content_id = "c".repeat(64);
        let signed = signed_publication(&content_id, 128, "lookup.txt", &keypair);
        let tombstone = SignedTombstone::sign(
            &TombstoneV1::new(
                signed.publication().identity_id.clone(),
                signed.publication_id(),
                decode_content_id(&content_id).expect("content id"),
                1,
                1_700_000_100_000,
            ),
            &keypair,
        )
        .expect("sign tombstone");
        let transport = FakeTransport::new()
            .with_record(publication_dht_key(signed.publication_id()), signed.raw())
            .with_record(tombstone_dht_key(signed.publication_id()), tombstone.raw())
            .with_providers(vec![DiscoveredPeer {
                peer_id: "peer-provider".to_owned(),
                addresses: vec!["/ip4/127.0.0.1/tcp/4001".to_owned()],
            }]);

        let events = Arc::new(EventQueue::new());
        let ops = Arc::new(OperationRegistry::new());
        let context = PublicationContext {
            store: Arc::clone(&store),
            events: Arc::clone(&events),
            transport: Some(Arc::new(transport) as Arc<dyn PublicationTransport>),
        };
        let publication_id = to_hex(&signed.publication_id());
        let (op_id, token) = ops.begin();
        run_publication_lookup(&context, &ops, op_id, token, publication_id.clone());

        let event = next_event(&events);
        assert_eq!(event.event_type, "publication.lookup_completed");
        assert_eq!(event.op_id, Some(op_id));
        assert_eq!(event_field(&event, "found").and_then(Value::as_bool), Some(true));
        assert_eq!(event_field(&event, "source").and_then(Value::as_text), Some("network"));
        assert_eq!(
            event_field(&event, "availability").and_then(Value::as_text),
            Some("record_only")
        );
        assert_eq!(event_field(&event, "conflict").and_then(Value::as_bool), Some(false));
        let record = event_field(&event, "record").expect("record");
        assert_eq!(map_get(record, "tombstoned").and_then(Value::as_bool), Some(true));
        assert_eq!(
            map_get(record, "signature_verified").and_then(Value::as_bool),
            Some(true)
        );
        let tombstone_value = map_get(record, "tombstone").expect("tombstone");
        assert_eq!(
            map_get(tombstone_value, "signature_verified").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            event_field(&event, "providers")
                .and_then(Value::as_array)
                .map(<[Value]>::len),
            Some(0)
        );

        let stored = store.publication(&publication_id).expect("stored publication");
        assert!(stored.tombstoned);
        assert_eq!(stored.availability, PublicationAvailability::RecordOnly);
        assert_eq!(
            store
                .tombstone(&publication_id)
                .expect("tombstone")
                .expect("row")
                .record_bytes,
            tombstone.raw()
        );
    }

    #[test]
    fn lookup_rejects_a_record_for_a_different_publication_id() {
        let root = TempRoot::new("lookup-mismatch");
        let store = store(&root);
        let keypair = KeyPair::from_seed([85_u8; 32]);
        let expected = signed_publication(&"d".repeat(64), 64, "expected.txt", &keypair);
        let other = signed_publication(&"e".repeat(64), 64, "other.txt", &keypair);
        let transport = FakeTransport::new().with_record(publication_dht_key(expected.publication_id()), other.raw());

        let events = Arc::new(EventQueue::new());
        let ops = Arc::new(OperationRegistry::new());
        let context = PublicationContext {
            store: Arc::clone(&store),
            events: Arc::clone(&events),
            transport: Some(Arc::new(transport) as Arc<dyn PublicationTransport>),
        };
        let expected_id = to_hex(&expected.publication_id());
        let (op_id, token) = ops.begin();
        run_publication_lookup(&context, &ops, op_id, token, expected_id.clone());

        let event = next_event(&events);
        assert_eq!(event.event_type, "publication.lookup_failed");
        assert_eq!(event.op_id, Some(op_id));
        assert_eq!(
            event_field(&event, "error")
                .and_then(|error| map_get(error, "code"))
                .and_then(Value::as_text),
            Some("verification_failed")
        );
        assert!(!store.publication_exists(&expected_id).expect("exists"));
        assert!(
            !store
                .publication_exists(&to_hex(&other.publication_id()))
                .expect("exists")
        );
    }
}
