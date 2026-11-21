//! Signed profile publishing, lookup, caching, and conflict reporting.
//!
//! Profiles are exact signed records published under a DHT key derived from
//! the owner's identity ID. The local identity signs a canonical profile body
//! without exposing its keypair, the signed bytes are stored before any
//! network work starts, and a lookup never replaces an accepted record with a
//! same-revision record that has different bytes.
//!
//! Usernames are labels, not reservations. Lookup results therefore include
//! every other known profile that uses the same label so the UI can show each
//! distinct identity ID instead of implying uniqueness.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use nexus_crypto::{DomainTag, IdentityId, Value, to_hex};
use nexus_net::{NetError, Network};
use nexus_protocol::{
    MAX_PROFILE_PUBLICATION_REFS, ProfileV1, ProtocolError, SignedProfile, SignedRecord, profile_dht_key,
};
use nexus_store::{ProfileCacheOutcome, ProfileConflict, ProfileFreshness, ProfileRecord, ProfileState, Store};

use crate::engine::{Engine, IdentityStatus, MethodOutcome};
use crate::envelope::error_value;
use crate::error::{CoreError, DomainCode, DomainError};
use crate::event::{EventQueue, PendingEvent};
use crate::network::domain_error as network_domain_error;
use crate::ops::{CancelToken, OperationRegistry};

/// Cached remote profiles older than this are labelled stale.
pub const PROFILE_CACHE_STALE_MS: u64 = 7 * 24 * 60 * 60 * 1_000;

/// A bounded profile transport used by the workers and unit tests.
pub(crate) trait ProfileTransport: Send + Sync {
    /// Publishes exact signed bytes under a derived key.
    fn publish(&self, key: &[u8], value: &[u8]) -> Result<(), NetError>;
    /// Looks up exact signed bytes under a derived key.
    fn lookup(&self, key: &[u8]) -> Result<Option<Vec<u8>>, NetError>;
}

/// The live networking implementation of [`ProfileTransport`].
pub(crate) struct NetworkProfileTransport {
    network: Arc<Network>,
}

impl NetworkProfileTransport {
    pub(crate) fn new(network: Arc<Network>) -> Self {
        Self { network }
    }
}

impl ProfileTransport for NetworkProfileTransport {
    fn publish(&self, key: &[u8], value: &[u8]) -> Result<(), NetError> {
        self.network.publish_record(key, value)
    }

    fn lookup(&self, key: &[u8]) -> Result<Option<Vec<u8>>, NetError> {
        self.network
            .lookup_record(key)
            .map(|value| value.map(|value| value.value))
    }
}

/// Shared state for a profile worker.
#[derive(Clone)]
pub(crate) struct ProfileContext {
    pub(crate) store: Arc<Store>,
    pub(crate) events: Arc<EventQueue>,
    pub(crate) transport: Option<Arc<dyn ProfileTransport>>,
}

#[derive(Default)]
struct ProfileUpdate {
    display_name: Option<String>,
    username: Option<Option<String>>,
    bio: Option<Option<String>>,
    avatar_content_id: Option<Option<[u8; 32]>>,
    publication_refs: Option<Vec<[u8; 32]>>,
    publish: bool,
    expected_revision: Option<u64>,
    has_fields: bool,
}

impl ProfileUpdate {
    fn parse(params: &Value) -> Result<Self, CoreError> {
        let display_name = optional_text(params, "display_name")?;
        let username = nullable_text(params, "username")?;
        let bio = nullable_text(params, "bio")?;
        let avatar_content_id = match map_get(params, "avatar_content_id") {
            None => None,
            Some(Value::Null) => Some(None),
            Some(Value::Text(text)) => Some(Some(
                decode_content_id(text).ok_or(CoreError::InvalidArgument("avatar_content_id must be 32-byte hex"))?,
            )),
            Some(_) => return Err(CoreError::InvalidArgument("avatar_content_id must be text or null")),
        };
        let publication_refs = match map_get(params, "publication_refs") {
            None => None,
            Some(Value::Null) => Some(Vec::new()),
            Some(Value::Array(items)) => {
                if items.len() > MAX_PROFILE_PUBLICATION_REFS {
                    return Err(CoreError::InvalidArgument("too many publication_refs"));
                }
                let mut refs = Vec::with_capacity(items.len());
                for item in items {
                    let text = item
                        .as_text()
                        .ok_or(CoreError::InvalidArgument("publication_refs must contain text IDs"))?;
                    refs.push(
                        decode_content_id(text)
                            .ok_or(CoreError::InvalidArgument("publication_refs must be 32-byte hex"))?,
                    );
                }
                Some(refs)
            }
            Some(_) => return Err(CoreError::InvalidArgument("publication_refs must be an array or null")),
        };
        let publish = match map_get(params, "publish") {
            None => true,
            Some(Value::Bool(value)) => *value,
            Some(_) => return Err(CoreError::InvalidArgument("publish must be a boolean")),
        };
        let expected_revision = match map_get(params, "expected_revision") {
            None | Some(Value::Null) => None,
            Some(value) => Some(value.as_unsigned().ok_or(CoreError::InvalidArgument(
                "expected_revision must be an unsigned integer",
            ))?),
        };
        let has_fields = display_name.is_some()
            || username.is_some()
            || bio.is_some()
            || avatar_content_id.is_some()
            || publication_refs.is_some();
        Ok(Self {
            display_name,
            username,
            bio,
            avatar_content_id,
            publication_refs,
            publish,
            expected_revision,
            has_fields,
        })
    }
}

impl Engine {
    /// Returns the local signed profile, if one exists.
    pub(crate) fn profile_get_local(&self) -> Result<MethodOutcome, CoreError> {
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
        let record = self.store.local_profile(&identity_id)?;
        let conflicts = self.store.profile_conflicts(&identity_id)?;
        let profile = record
            .as_ref()
            .map_or(Value::Null, |record| profile_value(record, &conflicts));
        Ok(MethodOutcome::Result(Value::Map(vec![
            (Value::Text("identity_id".to_owned()), Value::Text(identity_id)),
            (Value::Text("has_profile".to_owned()), Value::Bool(record.is_some())),
            (Value::Text("profile".to_owned()), profile),
            (
                Value::Text("conflicts".to_owned()),
                Value::Array(conflicts.iter().map(conflict_value).collect()),
            ),
        ])))
    }

    /// Signs and stores a profile, then optionally publishes it.
    pub(crate) fn profile_publish(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
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
        let update = match ProfileUpdate::parse(params) {
            Ok(update) => update,
            Err(error) => return Ok(MethodOutcome::Domain(CoreError::domain(error))),
        };
        let existing = self.store.local_profile(&identity_id)?;
        if let Some(expected) = update.expected_revision {
            let current = existing.as_ref().map_or(0, |record| record.revision);
            if expected != current {
                return Ok(MethodOutcome::Domain(conflict_error(
                    "The profile changed since it was loaded.",
                    current,
                )));
            }
        }

        if !update.has_fields {
            return self.republish_existing(identity_id, existing, update.publish);
        }

        let existing_signed = existing
            .as_ref()
            .and_then(|record| SignedProfile::parse(&record.record_bytes).ok());
        let existing_profile = existing_signed.as_ref().map(SignedProfile::profile);
        let display_name = update
            .display_name
            .or_else(|| existing_profile.map(|profile| profile.display_name.clone()));
        let Some(display_name) = display_name else {
            return Ok(MethodOutcome::Domain(DomainError::invalid(
                "display_name is required for the first profile",
            )));
        };
        let username = match update.username {
            Some(username) => username,
            None => existing_profile.and_then(|profile| profile.username.clone()),
        };
        let bio = match update.bio {
            Some(bio) => bio.unwrap_or_default(),
            None => existing_profile.map_or_else(String::new, |profile| profile.bio.clone()),
        };
        let avatar_content_id = match update.avatar_content_id {
            Some(content_id) => content_id,
            None => existing_profile.and_then(|profile| profile.avatar_content_id),
        };
        let publication_refs = update
            .publication_refs
            .unwrap_or_else(|| existing_profile.map_or_else(Vec::new, |profile| profile.publication_refs.clone()));

        let revision = match existing.as_ref() {
            Some(record) => {
                let next = record.revision.saturating_add(1);
                if next == record.revision {
                    return Ok(MethodOutcome::Domain(DomainError::new(
                        DomainCode::Conflict,
                        "The profile revision limit has been reached.",
                    )));
                }
                next
            }
            None => 1,
        };
        let identity = match IdentityId::parse(&identity_id) {
            Ok(identity) => identity,
            Err(_) => return Err(CoreError::InvalidArgument("identity ID is malformed")),
        };
        let mut profile = ProfileV1::new(identity, display_name, bio, revision, now_ms());
        profile.username = username;
        profile.avatar_content_id = avatar_content_id;
        profile.publication_refs = publication_refs;
        let body = match profile.encode_body() {
            Ok(body) => body,
            Err(error) => return Ok(MethodOutcome::Domain(profile_protocol_error(&error))),
        };
        let Some(signature) = self.sign_identity_domain(DomainTag::Profile, &body) else {
            return Ok(MethodOutcome::Domain(locked_error("identity_locked")));
        };
        let record = match SignedRecord::from_parts(body, signature) {
            Ok(record) => record,
            Err(error) => return Ok(MethodOutcome::Domain(profile_protocol_error(&error))),
        };
        let signed = match SignedProfile::from_record(record) {
            Ok(signed) => signed,
            Err(error) => return Ok(MethodOutcome::Domain(profile_protocol_error(&error))),
        };
        let state = if update.publish {
            ProfileState::Pending
        } else {
            ProfileState::Draft
        };
        let stored = profile_record_from_signed(&signed, state, now_ms(), now_secs());
        if let Err(error) = self.store.store_local_profile(&stored, update.expected_revision) {
            return Ok(MethodOutcome::Domain(store_domain_error(error)));
        }

        if !update.publish {
            let (op_id, _token) = self.ops.begin();
            emit_profile_event(
                &self.events,
                op_id,
                "profile.draft_saved",
                Value::Map(vec![
                    (Value::Text("identity_id".to_owned()), Value::Text(identity_id)),
                    (Value::Text("revision".to_owned()), Value::Unsigned(revision)),
                    (Value::Text("state".to_owned()), Value::Text("draft".to_owned())),
                    (Value::Text("discoverable".to_owned()), Value::Bool(false)),
                    (Value::Text("record_verified".to_owned()), Value::Bool(true)),
                    (
                        Value::Text("updated_at_ms".to_owned()),
                        Value::Unsigned(stored.updated_at_ms),
                    ),
                ]),
            );
            self.ops.finish(op_id);
            return Ok(MethodOutcome::Result(profile_accepted(op_id, revision, "draft")));
        }

        let (op_id, token) = self.ops.begin();
        let context = self.profile_context();
        let ops = Arc::clone(&self.ops);
        self.pool.submit(move || {
            run_profile_publish(&context, &ops, op_id, token, signed);
        })?;
        Ok(MethodOutcome::Result(profile_accepted(op_id, revision, "pending")))
    }

    /// Looks up and verifies another identity's signed profile.
    pub(crate) fn profile_lookup(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let identity_id = map_get(params, "identity_id")
            .and_then(Value::as_text)
            .ok_or(CoreError::InvalidArgument("identity_id is required"))?;
        let identity_id = IdentityId::parse(identity_id)
            .map_err(|_| CoreError::InvalidArgument("identity_id is malformed"))?
            .as_str()
            .to_owned();
        let (op_id, token) = self.ops.begin();
        let context = self.profile_context();
        let ops = Arc::clone(&self.ops);
        self.pool.submit(move || {
            run_profile_lookup(&context, &ops, op_id, token, identity_id);
        })?;
        Ok(MethodOutcome::Result(Value::Map(vec![
            (Value::Text("op_id".to_owned()), Value::Unsigned(op_id)),
            (Value::Text("accepted".to_owned()), Value::Bool(true)),
        ])))
    }

    fn republish_existing(
        &self,
        identity_id: String,
        existing: Option<ProfileRecord>,
        publish: bool,
    ) -> Result<MethodOutcome, CoreError> {
        let Some(existing) = existing else {
            return Ok(MethodOutcome::Domain(DomainError::invalid(
                "display_name is required for the first profile",
            )));
        };
        if !publish {
            return Ok(MethodOutcome::Domain(DomainError::invalid(
                "profile fields are required to save a draft",
            )));
        }
        let signed = match SignedProfile::parse(&existing.record_bytes) {
            Ok(signed) => signed,
            Err(error) => return Ok(MethodOutcome::Domain(profile_protocol_error(&error))),
        };
        if signed.profile().identity_id.as_str() != identity_id {
            return Ok(MethodOutcome::Domain(profile_protocol_error(
                &ProtocolError::IdentityKeyMismatch,
            )));
        }
        let revision = existing.revision;
        let mut pending = existing.clone();
        pending.state = ProfileState::Pending;
        pending.published_at = None;
        pending.freshness = ProfileFreshness::Current;
        self.store.store_local_profile(&pending, Some(revision))?;
        let (op_id, token) = self.ops.begin();
        let context = self.profile_context();
        let ops = Arc::clone(&self.ops);
        self.pool.submit(move || {
            run_profile_publish(&context, &ops, op_id, token, signed);
        })?;
        Ok(MethodOutcome::Result(profile_accepted(op_id, revision, "pending")))
    }

    fn profile_context(&self) -> ProfileContext {
        ProfileContext {
            store: Arc::clone(&self.store),
            events: Arc::clone(&self.events),
            transport: self
                .network_snapshot()
                .map(|network| Arc::new(NetworkProfileTransport::new(network)) as Arc<dyn ProfileTransport>),
        }
    }
}

/// Publishes one exact signed profile record and reports the outcome.
pub(crate) fn run_profile_publish(
    context: &ProfileContext,
    ops: &OperationRegistry,
    op_id: u64,
    token: Arc<CancelToken>,
    signed: SignedProfile,
) {
    let identity_id = signed.profile().identity_id.as_str().to_owned();
    let revision = signed.profile().revision;
    let updated_at_ms = signed.profile().updated_at_ms;
    let record_bytes = signed.raw();
    let key = profile_dht_key(&identity_id);
    if token.is_cancelled() {
        emit_publish_cancelled(context, op_id, &identity_id, revision);
        ops.finish(op_id);
        return;
    }
    let result = match &context.transport {
        Some(transport) => transport.publish(&key, record_bytes),
        None => Err(NetError::Disabled),
    };
    if token.is_cancelled() {
        emit_publish_cancelled(context, op_id, &identity_id, revision);
        ops.finish(op_id);
        return;
    }
    let at = now_ms();
    match result {
        Ok(()) => {
            let _ = context.store.mark_profile_published(&identity_id, true, at);
            emit_profile_event(
                &context.events,
                op_id,
                "profile.published",
                published_data(&identity_id, revision, updated_at_ms, true, "published", None),
            );
        }
        Err(error) => {
            let _ = context.store.mark_profile_published(&identity_id, false, at);
            let domain = network_domain_error(&error);
            emit_profile_event(
                &context.events,
                op_id,
                "profile.published",
                published_data(&identity_id, revision, updated_at_ms, false, "deferred", Some(&domain)),
            );
        }
    }
    ops.finish(op_id);
}

/// Looks up, verifies, and caches one profile, then reports freshness.
pub(crate) fn run_profile_lookup(
    context: &ProfileContext,
    ops: &OperationRegistry,
    op_id: u64,
    token: Arc<CancelToken>,
    identity_id: String,
) {
    let key = profile_dht_key(&identity_id);
    if token.is_cancelled() {
        emit_lookup_cancelled(context, op_id, &identity_id);
        ops.finish(op_id);
        return;
    }
    let result = match &context.transport {
        Some(transport) => transport.lookup(&key),
        None => Err(NetError::Disabled),
    };
    if token.is_cancelled() {
        emit_lookup_cancelled(context, op_id, &identity_id);
        ops.finish(op_id);
        return;
    }
    let now = now_ms();
    match result {
        Ok(Some(raw)) => handle_network_record(context, op_id, &identity_id, &key, raw, now),
        Ok(None) => handle_missing_record(context, op_id, &identity_id, now, None),
        Err(error) => {
            let domain = network_domain_error(&error);
            handle_missing_record(context, op_id, &identity_id, now, Some(&domain));
        }
    }
    ops.finish(op_id);
}

fn handle_network_record(
    context: &ProfileContext,
    op_id: u64,
    identity_id: &str,
    key: &[u8; 32],
    raw: Vec<u8>,
    now: u64,
) {
    let verified = SignedProfile::parse(&raw).and_then(|signed| {
        if signed.profile().identity_id.as_str() != identity_id {
            return Err(ProtocolError::IdentityKeyMismatch);
        }
        if profile_dht_key(signed.profile().identity_id.as_str()) != *key {
            return Err(ProtocolError::DhtKeyMismatch);
        }
        Ok(signed)
    });
    let signed = match verified {
        Ok(signed) => signed,
        Err(error) => {
            emit_profile_event(
                &context.events,
                op_id,
                "profile.lookup_failed",
                Value::Map(vec![
                    (
                        Value::Text("identity_id".to_owned()),
                        Value::Text(identity_id.to_owned()),
                    ),
                    (
                        Value::Text("error".to_owned()),
                        error_value(&profile_protocol_error(&error)),
                    ),
                ]),
            );
            return;
        }
    };
    let incoming = profile_record_from_signed(&signed, ProfileState::Remote, now, now);
    let outcome = match context.store.cache_remote_profile(&incoming) {
        Ok(outcome) => outcome,
        Err(error) => {
            emit_profile_event(
                &context.events,
                op_id,
                "profile.lookup_failed",
                Value::Map(vec![
                    (
                        Value::Text("identity_id".to_owned()),
                        Value::Text(identity_id.to_owned()),
                    ),
                    (Value::Text("error".to_owned()), error_value(&store_domain_error(error))),
                ]),
            );
            return;
        }
    };
    let cached = context.store.profile(identity_id).ok().flatten();
    let conflicts = context.store.profile_conflicts(identity_id).unwrap_or_default();
    let collisions = cached
        .as_ref()
        .map_or_else(Vec::new, |record| username_collisions(&context.store, record));
    let (record, stale, conflict, source) = match outcome {
        ProfileCacheOutcome::Stored | ProfileCacheOutcome::Unchanged => (cached, false, false, "network"),
        ProfileCacheOutcome::Conflict => (cached, false, true, "cache"),
        ProfileCacheOutcome::Stale => (cached, true, false, "cache"),
    };
    let freshness = record
        .as_ref()
        .map_or(ProfileFreshness::Unknown, |record| record.freshness);
    emit_lookup_completed(
        context,
        op_id,
        identity_id,
        record.as_ref(),
        source,
        freshness,
        stale,
        conflict,
        &conflicts,
        &collisions,
        now,
        None,
    );
}

fn handle_missing_record(
    context: &ProfileContext,
    op_id: u64,
    identity_id: &str,
    now: u64,
    error: Option<&DomainError>,
) {
    let cached = context.store.profile(identity_id).ok().flatten();
    match cached {
        Some(mut record) => {
            // The network did not return a verified record, so a cached remote
            // row is unconfirmed no matter how recently it was last checked.
            let freshness = if record.state.is_local() {
                ProfileFreshness::Current
            } else {
                ProfileFreshness::Stale
            };
            let source = if record.state.is_local() { "local" } else { "cache" };
            let stale = !record.state.is_local();
            let _ = context.store.mark_profile_checked(identity_id, freshness, now);
            record.freshness = freshness;
            let conflicts = context.store.profile_conflicts(identity_id).unwrap_or_default();
            let collisions = username_collisions(&context.store, &record);
            emit_lookup_completed(
                context,
                op_id,
                identity_id,
                Some(&record),
                source,
                freshness,
                stale,
                false,
                &conflicts,
                &collisions,
                now,
                error,
            );
        }
        None => match error {
            Some(error) => emit_profile_event(
                &context.events,
                op_id,
                "profile.lookup_failed",
                Value::Map(vec![
                    (
                        Value::Text("identity_id".to_owned()),
                        Value::Text(identity_id.to_owned()),
                    ),
                    (Value::Text("error".to_owned()), error_value(error)),
                ]),
            ),
            None => emit_lookup_completed(
                context,
                op_id,
                identity_id,
                None,
                "network",
                ProfileFreshness::Unknown,
                false,
                false,
                &[],
                &[],
                now,
                None,
            ),
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_lookup_completed(
    context: &ProfileContext,
    op_id: u64,
    identity_id: &str,
    record: Option<&ProfileRecord>,
    source: &str,
    freshness: ProfileFreshness,
    stale: bool,
    conflict: bool,
    conflicts: &[ProfileConflict],
    collisions: &[ProfileRecord],
    checked_at: u64,
    error: Option<&DomainError>,
) {
    let mut entries = vec![
        (
            Value::Text("identity_id".to_owned()),
            Value::Text(identity_id.to_owned()),
        ),
        (Value::Text("found".to_owned()), Value::Bool(record.is_some())),
        (Value::Text("source".to_owned()), Value::Text(source.to_owned())),
        (
            Value::Text("freshness".to_owned()),
            Value::Text(freshness.as_str().to_owned()),
        ),
        (Value::Text("stale".to_owned()), Value::Bool(stale)),
        (Value::Text("conflict".to_owned()), Value::Bool(conflict)),
        (
            Value::Text("record".to_owned()),
            record.map_or(Value::Null, |record| profile_value(record, conflicts)),
        ),
        (
            Value::Text("conflicts".to_owned()),
            Value::Array(conflicts.iter().map(conflict_value).collect()),
        ),
        (
            Value::Text("username_collisions".to_owned()),
            Value::Array(collisions.iter().map(profile_summary_value).collect()),
        ),
        (Value::Text("username_is_label".to_owned()), Value::Bool(true)),
        (Value::Text("checked_at_ms".to_owned()), Value::Unsigned(checked_at)),
    ];
    if let Some(error) = error {
        entries.push((Value::Text("error".to_owned()), error_value(error)));
    }
    emit_profile_event(&context.events, op_id, "profile.lookup_completed", Value::Map(entries));
}

fn emit_publish_cancelled(context: &ProfileContext, op_id: u64, identity_id: &str, revision: u64) {
    emit_profile_event(
        &context.events,
        op_id,
        "profile.publish_cancelled",
        Value::Map(vec![
            (
                Value::Text("identity_id".to_owned()),
                Value::Text(identity_id.to_owned()),
            ),
            (Value::Text("revision".to_owned()), Value::Unsigned(revision)),
        ]),
    );
}

fn emit_lookup_cancelled(context: &ProfileContext, op_id: u64, identity_id: &str) {
    emit_profile_event(
        &context.events,
        op_id,
        "profile.lookup_cancelled",
        Value::Map(vec![(
            Value::Text("identity_id".to_owned()),
            Value::Text(identity_id.to_owned()),
        )]),
    );
}

fn emit_profile_event(events: &EventQueue, op_id: u64, event_type: &str, data: Value) {
    let _ = events.push(PendingEvent::durable(event_type, data).with_op(op_id));
}

fn published_data(
    identity_id: &str,
    revision: u64,
    updated_at_ms: u64,
    discoverable: bool,
    announcement: &str,
    error: Option<&DomainError>,
) -> Value {
    let mut entries = vec![
        (
            Value::Text("identity_id".to_owned()),
            Value::Text(identity_id.to_owned()),
        ),
        (Value::Text("revision".to_owned()), Value::Unsigned(revision)),
        (
            Value::Text("state".to_owned()),
            Value::Text(if discoverable { "published" } else { "pending" }.to_owned()),
        ),
        (Value::Text("discoverable".to_owned()), Value::Bool(discoverable)),
        (
            Value::Text("announcement".to_owned()),
            Value::Text(announcement.to_owned()),
        ),
        (Value::Text("record_verified".to_owned()), Value::Bool(true)),
        (Value::Text("updated_at_ms".to_owned()), Value::Unsigned(updated_at_ms)),
    ];
    if let Some(error) = error {
        entries.push((Value::Text("error".to_owned()), error_value(error)));
    }
    Value::Map(entries)
}

fn profile_accepted(op_id: u64, revision: u64, state: &str) -> Value {
    Value::Map(vec![
        (Value::Text("op_id".to_owned()), Value::Unsigned(op_id)),
        (Value::Text("accepted".to_owned()), Value::Bool(true)),
        (Value::Text("revision".to_owned()), Value::Unsigned(revision)),
        (Value::Text("state".to_owned()), Value::Text(state.to_owned())),
    ])
}

fn profile_record_from_signed(
    signed: &SignedProfile,
    state: ProfileState,
    checked_at: u64,
    local_updated_at: u64,
) -> ProfileRecord {
    let profile = signed.profile();
    ProfileRecord {
        identity_id: profile.identity_id.as_str().to_owned(),
        revision: profile.revision,
        record_bytes: signed.raw().to_vec(),
        display_name: profile.display_name.clone(),
        username: profile.username.clone(),
        bio: Some(profile.bio.clone()).filter(|bio| !bio.is_empty()),
        avatar_content_id: profile.avatar_content_id.map(|content_id| to_hex(&content_id)),
        updated_at: local_updated_at,
        updated_at_ms: profile.updated_at_ms,
        state,
        published_at: None,
        last_checked_at: (state == ProfileState::Remote).then_some(checked_at),
        freshness: ProfileFreshness::Current,
    }
}

fn profile_value(record: &ProfileRecord, conflicts: &[ProfileConflict]) -> Value {
    let freshness = cache_freshness(record, now_ms());
    let signed = SignedProfile::parse(&record.record_bytes).ok();
    let publication_refs = signed.as_ref().map_or_else(Vec::new, |signed| {
        signed
            .profile()
            .publication_refs
            .iter()
            .map(|reference| Value::Text(to_hex(reference)))
            .collect()
    });
    Value::Map(vec![
        (
            Value::Text("identity_id".to_owned()),
            Value::Text(record.identity_id.clone()),
        ),
        (Value::Text("revision".to_owned()), Value::Unsigned(record.revision)),
        (
            Value::Text("display_name".to_owned()),
            Value::Text(record.display_name.clone()),
        ),
        (
            Value::Text("username".to_owned()),
            record
                .username
                .as_ref()
                .map_or(Value::Null, |username| Value::Text(username.clone())),
        ),
        (Value::Text("username_is_label".to_owned()), Value::Bool(true)),
        (
            Value::Text("bio".to_owned()),
            record.bio.as_ref().map_or(Value::Null, |bio| Value::Text(bio.clone())),
        ),
        (
            Value::Text("avatar_content_id".to_owned()),
            record
                .avatar_content_id
                .as_ref()
                .map_or(Value::Null, |content_id| Value::Text(content_id.clone())),
        ),
        (
            Value::Text("publication_refs".to_owned()),
            Value::Array(publication_refs),
        ),
        (
            Value::Text("state".to_owned()),
            Value::Text(record.state.as_str().to_owned()),
        ),
        (
            Value::Text("freshness".to_owned()),
            Value::Text(freshness.as_str().to_owned()),
        ),
        (
            Value::Text("stale".to_owned()),
            Value::Bool(freshness == ProfileFreshness::Stale),
        ),
        (
            Value::Text("discoverable".to_owned()),
            Value::Bool(record.state == ProfileState::Published),
        ),
        (Value::Text("signed".to_owned()), Value::Bool(true)),
        (Value::Text("record_verified".to_owned()), Value::Bool(signed.is_some())),
        (
            Value::Text("updated_at_ms".to_owned()),
            Value::Unsigned(record.updated_at_ms),
        ),
        (
            Value::Text("published_at".to_owned()),
            record.published_at.map_or(Value::Null, Value::Unsigned),
        ),
        (
            Value::Text("last_checked_at".to_owned()),
            record.last_checked_at.map_or(Value::Null, Value::Unsigned),
        ),
        (
            Value::Text("conflict_count".to_owned()),
            Value::Unsigned(u64::try_from(conflicts.len()).unwrap_or(u64::MAX)),
        ),
    ])
}

fn profile_summary_value(record: &ProfileRecord) -> Value {
    Value::Map(vec![
        (
            Value::Text("identity_id".to_owned()),
            Value::Text(record.identity_id.clone()),
        ),
        (Value::Text("revision".to_owned()), Value::Unsigned(record.revision)),
        (
            Value::Text("display_name".to_owned()),
            Value::Text(record.display_name.clone()),
        ),
        (
            Value::Text("username".to_owned()),
            record
                .username
                .as_ref()
                .map_or(Value::Null, |username| Value::Text(username.clone())),
        ),
        (
            Value::Text("state".to_owned()),
            Value::Text(record.state.as_str().to_owned()),
        ),
    ])
}

fn conflict_value(conflict: &ProfileConflict) -> Value {
    Value::Map(vec![
        (Value::Text("revision".to_owned()), Value::Unsigned(conflict.revision)),
        (
            Value::Text("observed_at".to_owned()),
            Value::Unsigned(conflict.observed_at),
        ),
    ])
}

fn username_collisions(store: &Store, record: &ProfileRecord) -> Vec<ProfileRecord> {
    let Some(username) = record.username.as_deref() else {
        return Vec::new();
    };
    store
        .profiles_with_username(username)
        .unwrap_or_default()
        .into_iter()
        .filter(|other| other.identity_id != record.identity_id)
        .collect()
}

fn cache_freshness(record: &ProfileRecord, now: u64) -> ProfileFreshness {
    if record.state.is_local() {
        return ProfileFreshness::Current;
    }
    if record
        .last_checked_at
        .is_none_or(|checked| now.saturating_sub(checked) > PROFILE_CACHE_STALE_MS)
    {
        ProfileFreshness::Stale
    } else {
        record.freshness
    }
}

fn locked_error(reason: &str) -> DomainError {
    DomainError::new(DomainCode::Locked, "The identity is locked.").with_details(Value::Map(vec![(
        Value::Text("reason".to_owned()),
        Value::Text(reason.to_owned()),
    )]))
}

fn conflict_error(message: &str, current_revision: u64) -> DomainError {
    DomainError::new(DomainCode::Conflict, message).with_details(Value::Map(vec![(
        Value::Text("current_revision".to_owned()),
        Value::Unsigned(current_revision),
    )]))
}

fn profile_protocol_error(error: &ProtocolError) -> DomainError {
    let (code, message) = match error {
        ProtocolError::UnsupportedVersion(_) => (
            DomainCode::UnsupportedVersion,
            "The profile record version is not supported.",
        ),
        ProtocolError::RecordTooLarge => (DomainCode::InvalidArgument, "The profile record is too large."),
        ProtocolError::VerificationFailed
        | ProtocolError::IdentityKeyMismatch
        | ProtocolError::DhtKeyMismatch
        | ProtocolError::InvalidDhtRecord => (
            DomainCode::VerificationFailed,
            "The profile record failed verification.",
        ),
        ProtocolError::InvalidField(_)
        | ProtocolError::MissingField(_)
        | ProtocolError::InvalidFieldKey
        | ProtocolError::InvalidEnvelope => (DomainCode::InvalidArgument, "The profile fields are invalid."),
        _ => (DomainCode::InvalidArgument, "The profile record is invalid."),
    };
    DomainError::new(code, message)
}

fn store_domain_error(error: nexus_store::StoreError) -> DomainError {
    match error {
        nexus_store::StoreError::RecordConflict(_) => {
            DomainError::new(DomainCode::Conflict, "The profile changed since it was loaded.")
        }
        nexus_store::StoreError::RecordNotFound(_) => {
            DomainError::new(DomainCode::NotFound, "The profile is not stored locally.")
        }
        nexus_store::StoreError::InvalidId(message) | nexus_store::StoreError::InvalidRecord(message) => {
            DomainError::invalid(message)
        }
        _ => DomainError::internal("The profile could not be stored."),
    }
}

impl CoreError {
    fn domain(error: CoreError) -> DomainError {
        match error {
            CoreError::InvalidArgument(message) => DomainError::invalid(message),
            other => DomainError::internal(other.to_string()),
        }
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

fn optional_text(value: &Value, key: &str) -> Result<Option<String>, CoreError> {
    match map_get(value, key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Text(text)) => Ok(Some(text.clone())),
        Some(_) => Err(CoreError::InvalidArgument("profile text field must be text")),
    }
}

fn nullable_text(value: &Value, key: &str) -> Result<Option<Option<String>>, CoreError> {
    match map_get(value, key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::Text(text)) => Ok(Some(Some(text.clone()))),
        Some(_) => Err(CoreError::InvalidArgument("profile text field must be text or null")),
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

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    use nexus_crypto::{IdentityId, KeyPair};
    use nexus_protocol::{ProfileV1, SignedProfile};
    use nexus_store::{ProfileState, QuotaPolicy, Store, StoreConfig};

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
            let path =
                std::env::temp_dir().join(format!("nexus-core-profile-{label}-{}-{sequence}", std::process::id()));
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

    struct FakeTransport {
        publish_ok: bool,
        record: Option<Vec<u8>>,
        lookup_fails: bool,
        published: Mutex<Vec<Vec<u8>>>,
    }

    impl FakeTransport {
        fn publishing(record: Option<Vec<u8>>) -> Self {
            Self {
                publish_ok: true,
                record,
                lookup_fails: false,
                published: Mutex::new(Vec::new()),
            }
        }
    }

    impl ProfileTransport for FakeTransport {
        fn publish(&self, _key: &[u8], value: &[u8]) -> Result<(), NetError> {
            if self.publish_ok {
                self.published.lock().expect("published").push(value.to_vec());
                Ok(())
            } else {
                Err(NetError::Unavailable)
            }
        }

        fn lookup(&self, _key: &[u8]) -> Result<Option<Vec<u8>>, NetError> {
            if self.lookup_fails {
                Err(NetError::Timeout)
            } else {
                Ok(self.record.clone())
            }
        }
    }

    fn signed_profile(
        keypair: &KeyPair,
        display_name: &str,
        username: Option<&str>,
        revision: u64,
        updated_at_ms: u64,
    ) -> SignedProfile {
        let mut profile = ProfileV1::new(
            IdentityId::from_public_key(&keypair.public_key()),
            display_name,
            "local-first",
            revision,
            updated_at_ms,
        );
        profile.username = username.map(str::to_owned);
        profile.sign(keypair).expect("sign profile")
    }

    fn store_profile(store: &Store, signed: &SignedProfile, state: ProfileState) {
        let record = profile_record_from_signed(signed, state, now_ms(), now_secs());
        store.store_local_profile(&record, None).expect("store profile");
    }

    fn event_field<'a>(event: &'a crate::event::Event, key: &str) -> Option<&'a Value> {
        map_get(&event.data, key)
    }

    #[test]
    fn publish_worker_marks_published_and_emits_a_verified_event() {
        let root = TempRoot::new("publish-ok");
        let store = store(&root);
        let keypair = KeyPair::from_seed([41_u8; 32]);
        let signed = signed_profile(&keypair, "Ada", Some("ada"), 1, 1_000);
        store_profile(&store, &signed, ProfileState::Pending);

        let events = Arc::new(EventQueue::new());
        let ops = Arc::new(OperationRegistry::new());
        let transport = Arc::new(FakeTransport::publishing(None));
        let context = ProfileContext {
            store: Arc::clone(&store),
            events: Arc::clone(&events),
            transport: Some(Arc::clone(&transport) as Arc<dyn ProfileTransport>),
        };
        let (op_id, token) = ops.begin();
        run_profile_publish(&context, &ops, op_id, token, signed.clone());

        let event = events
            .next_event(Duration::from_secs(1))
            .expect("event")
            .expect("queued");
        assert_eq!(event.event_type, "profile.published");
        assert_eq!(event.op_id, Some(op_id));
        assert_eq!(event_field(&event, "discoverable").and_then(Value::as_bool), Some(true));
        assert_eq!(
            event_field(&event, "announcement").and_then(Value::as_text),
            Some("published")
        );
        let stored = store
            .profile(signed.profile().identity_id.as_str())
            .expect("profile")
            .expect("row");
        assert_eq!(stored.state, ProfileState::Published);
        assert_eq!(transport.published.lock().expect("published").len(), 1);
    }

    #[test]
    fn publish_worker_defers_without_a_transport() {
        let root = TempRoot::new("publish-deferred");
        let store = store(&root);
        let keypair = KeyPair::from_seed([42_u8; 32]);
        let signed = signed_profile(&keypair, "Ada", None, 1, 1_000);
        store_profile(&store, &signed, ProfileState::Pending);

        let events = Arc::new(EventQueue::new());
        let ops = Arc::new(OperationRegistry::new());
        let context = ProfileContext {
            store: Arc::clone(&store),
            events: Arc::clone(&events),
            transport: None,
        };
        let (op_id, token) = ops.begin();
        run_profile_publish(&context, &ops, op_id, token, signed.clone());

        let event = events
            .next_event(Duration::from_secs(1))
            .expect("event")
            .expect("queued");
        assert_eq!(event.event_type, "profile.published");
        assert_eq!(
            event_field(&event, "discoverable").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            event_field(&event, "announcement").and_then(Value::as_text),
            Some("deferred")
        );
        assert!(event_field(&event, "error").is_some());
        let stored = store
            .profile(signed.profile().identity_id.as_str())
            .expect("profile")
            .expect("row");
        assert_eq!(stored.state, ProfileState::Pending);
    }

    #[test]
    fn lookup_caches_a_verified_record_and_reports_username_collisions() {
        let root = TempRoot::new("lookup-collision");
        let store = store(&root);
        let local = KeyPair::from_seed([43_u8; 32]);
        let remote = KeyPair::from_seed([44_u8; 32]);
        let local_signed = signed_profile(&local, "Ada", Some("ada"), 1, 1_000);
        let remote_signed = signed_profile(&remote, "Another Ada", Some("ADA"), 1, 2_000);
        store_profile(&store, &local_signed, ProfileState::Published);

        let events = Arc::new(EventQueue::new());
        let ops = Arc::new(OperationRegistry::new());
        let context = ProfileContext {
            store: Arc::clone(&store),
            events: Arc::clone(&events),
            transport: Some(
                Arc::new(FakeTransport::publishing(Some(remote_signed.raw().to_vec()))) as Arc<dyn ProfileTransport>
            ),
        };
        let (op_id, token) = ops.begin();
        run_profile_lookup(
            &context,
            &ops,
            op_id,
            token,
            remote_signed.profile().identity_id.as_str().to_owned(),
        );

        let event = events
            .next_event(Duration::from_secs(1))
            .expect("event")
            .expect("queued");
        assert_eq!(event.event_type, "profile.lookup_completed");
        assert_eq!(event_field(&event, "found").and_then(Value::as_bool), Some(true));
        assert_eq!(event_field(&event, "source").and_then(Value::as_text), Some("network"));
        let record = event_field(&event, "record").expect("record");
        assert_eq!(
            map_get(record, "identity_id").and_then(Value::as_text),
            Some(remote_signed.profile().identity_id.as_str())
        );
        match event_field(&event, "username_collisions") {
            Some(Value::Array(collisions)) => {
                assert_eq!(collisions.len(), 1);
                assert_eq!(
                    map_get(&collisions[0], "identity_id").and_then(Value::as_text),
                    Some(local_signed.profile().identity_id.as_str())
                );
            }
            other => panic!("missing username collisions: {other:?}"),
        }
        let cached = store
            .profile(remote_signed.profile().identity_id.as_str())
            .expect("profile")
            .expect("row");
        assert_eq!(cached.state, ProfileState::Remote);
    }

    #[test]
    fn lookup_keeps_a_newer_cache_when_the_network_returns_an_older_revision() {
        let root = TempRoot::new("lookup-stale");
        let store = store(&root);
        let keypair = KeyPair::from_seed([45_u8; 32]);
        let newer = signed_profile(&keypair, "Ada", None, 2, 2_000);
        let older = signed_profile(&keypair, "Ada", None, 1, 1_000);
        let newer_record = profile_record_from_signed(&newer, ProfileState::Remote, now_ms(), now_secs());
        store.cache_remote_profile(&newer_record).expect("cache newer");

        let events = Arc::new(EventQueue::new());
        let ops = Arc::new(OperationRegistry::new());
        let context = ProfileContext {
            store: Arc::clone(&store),
            events: Arc::clone(&events),
            transport: Some(
                Arc::new(FakeTransport::publishing(Some(older.raw().to_vec()))) as Arc<dyn ProfileTransport>
            ),
        };
        let (op_id, token) = ops.begin();
        run_profile_lookup(
            &context,
            &ops,
            op_id,
            token,
            newer.profile().identity_id.as_str().to_owned(),
        );

        let event = events
            .next_event(Duration::from_secs(1))
            .expect("event")
            .expect("queued");
        assert_eq!(event.event_type, "profile.lookup_completed");
        assert_eq!(event_field(&event, "stale").and_then(Value::as_bool), Some(true));
        assert_eq!(event_field(&event, "source").and_then(Value::as_text), Some("cache"));
        let record = event_field(&event, "record").expect("record");
        assert_eq!(map_get(record, "revision").and_then(Value::as_unsigned), Some(2));
    }

    #[test]
    fn lookup_labels_a_cache_stale_when_the_network_fails() {
        let root = TempRoot::new("lookup-offline");
        let store = store(&root);
        let keypair = KeyPair::from_seed([46_u8; 32]);
        let signed = signed_profile(&keypair, "Ada", None, 1, 1_000);
        let record = profile_record_from_signed(&signed, ProfileState::Remote, now_ms(), now_secs());
        store.cache_remote_profile(&record).expect("cache");

        let events = Arc::new(EventQueue::new());
        let ops = Arc::new(OperationRegistry::new());
        let context = ProfileContext {
            store: Arc::clone(&store),
            events: Arc::clone(&events),
            transport: Some(Arc::new(FakeTransport {
                publish_ok: false,
                record: None,
                lookup_fails: true,
                published: Mutex::new(Vec::new()),
            }) as Arc<dyn ProfileTransport>),
        };
        let (op_id, token) = ops.begin();
        run_profile_lookup(
            &context,
            &ops,
            op_id,
            token,
            signed.profile().identity_id.as_str().to_owned(),
        );

        let event = events
            .next_event(Duration::from_secs(1))
            .expect("event")
            .expect("queued");
        assert_eq!(event.event_type, "profile.lookup_completed");
        assert_eq!(event_field(&event, "found").and_then(Value::as_bool), Some(true));
        assert_eq!(event_field(&event, "stale").and_then(Value::as_bool), Some(true));
        assert_eq!(event_field(&event, "freshness").and_then(Value::as_text), Some("stale"));
        let error = event_field(&event, "error").expect("error");
        assert_eq!(
            map_get(error, "code").and_then(Value::as_text),
            Some(DomainCode::Timeout.as_str())
        );
    }

    #[test]
    fn lookup_keeps_a_local_profile_current_when_the_network_fails() {
        let root = TempRoot::new("lookup-local-offline");
        let store = store(&root);
        let keypair = KeyPair::from_seed([49_u8; 32]);
        let signed = signed_profile(&keypair, "Ada", None, 1, 1_000);
        store_profile(&store, &signed, ProfileState::Published);

        let events = Arc::new(EventQueue::new());
        let ops = Arc::new(OperationRegistry::new());
        let context = ProfileContext {
            store: Arc::clone(&store),
            events: Arc::clone(&events),
            transport: Some(Arc::new(FakeTransport {
                publish_ok: false,
                record: None,
                lookup_fails: true,
                published: Mutex::new(Vec::new()),
            }) as Arc<dyn ProfileTransport>),
        };
        let (op_id, token) = ops.begin();
        run_profile_lookup(
            &context,
            &ops,
            op_id,
            token,
            signed.profile().identity_id.as_str().to_owned(),
        );

        let event = events
            .next_event(Duration::from_secs(1))
            .expect("event")
            .expect("queued");
        assert_eq!(event.event_type, "profile.lookup_completed");
        assert_eq!(event_field(&event, "found").and_then(Value::as_bool), Some(true));
        assert_eq!(event_field(&event, "source").and_then(Value::as_text), Some("local"));
        assert_eq!(
            event_field(&event, "freshness").and_then(Value::as_text),
            Some("current")
        );
        assert_eq!(event_field(&event, "stale").and_then(Value::as_bool), Some(false));
    }

    #[test]
    fn lookup_rejects_a_record_for_a_different_identity() {
        let root = TempRoot::new("lookup-wrong-identity");
        let store = store(&root);
        let requested = KeyPair::from_seed([47_u8; 32]);
        let other = KeyPair::from_seed([48_u8; 32]);
        let other_signed = signed_profile(&other, "Other", None, 1, 1_000);

        let events = Arc::new(EventQueue::new());
        let ops = Arc::new(OperationRegistry::new());
        let context = ProfileContext {
            store: Arc::clone(&store),
            events: Arc::clone(&events),
            transport: Some(
                Arc::new(FakeTransport::publishing(Some(other_signed.raw().to_vec()))) as Arc<dyn ProfileTransport>
            ),
        };
        let (op_id, token) = ops.begin();
        let requested_id = IdentityId::from_public_key(&requested.public_key());
        run_profile_lookup(&context, &ops, op_id, token, requested_id.as_str().to_owned());

        let event = events
            .next_event(Duration::from_secs(1))
            .expect("event")
            .expect("queued");
        assert_eq!(event.event_type, "profile.lookup_failed");
        let error = event_field(&event, "error").expect("error");
        assert_eq!(
            map_get(error, "code").and_then(Value::as_text),
            Some(DomainCode::VerificationFailed.as_str())
        );
        assert!(store.profile(requested_id.as_str()).expect("profile").is_none());
    }
}
