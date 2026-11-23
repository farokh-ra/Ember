//! Voluntary replication: explicit hosting, re-verification, and availability.
//!
//! A replica is never created implicitly. `replica.add` verifies the complete
//! local file and its manifest before the row becomes active, and provider
//! advertisement happens only after that verification. Removing a replica
//! removes only this device's hosting reference; the original signed
//! publication record and any other references are untouched.

use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nexus_crypto::{Value, to_hex};
use nexus_net::{NetError, Network};
use nexus_protocol::{ManifestV1, ProtocolError, SignedPublication};
use nexus_store::{HostingLimits, ReplicaRecord, ReplicaState, Store, StoreError};

use crate::engine::{Engine, MethodOutcome};
use crate::envelope::error_value;
use crate::error::{CoreError, DomainCode, DomainError};
use crate::event::{EventQueue, PendingEvent};
use crate::hosting::HostingState;
use crate::network::domain_error as network_domain_error;
use crate::ops::{CancelToken, OperationRegistry};

/// How old a successful replica verification may be before the periodic
/// maintenance pass re-checks it.
pub const REPLICA_REVERIFY_INTERVAL_MS: u64 = 7 * 24 * 60 * 60 * 1_000;
/// How often the maintenance thread wakes to look for stale replicas.
const MAINTENANCE_WAKE: Duration = Duration::from_secs(60 * 60);
/// User-facing limits repeated by replica results.
pub const REPLICA_LIMITS: &str = "Hosting is voluntary. This device serves the file only while the replica is active and verified. Removing it stops this device from serving it but never deletes the original signed publication or copies held by other peers.";

pub(crate) struct ReplicaContext {
    store: Arc<Store>,
    events: Arc<EventQueue>,
    network: Option<Arc<Network>>,
    hosting: Arc<HostingState>,
}

pub(crate) struct MaintenanceHandle {
    stop: Arc<CancelToken>,
    thread: Option<JoinHandle<()>>,
}

impl MaintenanceHandle {
    pub(crate) fn stop(&mut self) {
        self.stop.cancel();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for MaintenanceHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Engine {
    /// Explicitly hosts a verified local file.
    pub(crate) fn replica_add(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let confirmation = matches!(map_get(params, "confirmation"), Some(Value::Bool(true)));
        if !confirmation {
            return Ok(MethodOutcome::Domain(DomainError::new(
                DomainCode::Conflict,
                "Hosting a replica requires explicit confirmation.",
            )));
        }
        self.update_hosting_signals(params)?;
        if let Some(reason) = self.hosting.blocked_reason() {
            return Ok(MethodOutcome::Domain(blocked_error(reason)));
        }
        let requested = match RequestedReplica::parse(params) {
            Ok(requested) => requested,
            Err(error) => return Ok(MethodOutcome::Domain(DomainError::from(error))),
        };
        let (content_id, publication_id) = match resolve_replica(&self.store, &requested) {
            Ok(resolved) => resolved,
            Err(error) => return Ok(MethodOutcome::Domain(error)),
        };
        if let Err(error) = self.store.ensure_quota(0) {
            return Ok(MethodOutcome::Domain(DomainError::from_store(&error)));
        }
        let replica_id = replica_id_for(&content_id);
        let previous = self.store.replica(&replica_id).ok();
        if previous
            .as_ref()
            .is_some_and(|record| record.state == ReplicaState::Active)
        {
            return Ok(MethodOutcome::Domain(DomainError::new(
                DomainCode::Conflict,
                "This file is already hosted on this device.",
            )));
        }
        let source_peer = requested.source_peer.clone();
        self.try_emit(PendingEvent::durable(
            "replica.verifying",
            Value::Map(vec![
                (Value::Text("replica_id".to_owned()), Value::Text(replica_id.clone())),
                (Value::Text("content_id".to_owned()), Value::Text(content_id.clone())),
            ]),
        ));
        let size = match verify_replica(&self.store, &content_id, publication_id.as_deref()) {
            Ok(size) => size,
            Err(error) => {
                if let Some(mut record) = previous {
                    record.state = ReplicaState::VerificationFailed;
                    record.announced = false;
                    record.last_checked_at = Some(now_secs());
                    record.failure_count = record.failure_count.saturating_add(1);
                    record.last_error = Some(bounded_error(&error));
                    record.updated_at = now_secs();
                    let _ = self.store.upsert_replica(&record);
                }
                self.try_emit(PendingEvent::durable(
                    "replica.failed",
                    Value::Map(vec![
                        (Value::Text("replica_id".to_owned()), Value::Text(replica_id)),
                        (Value::Text("error".to_owned()), error_value(&error)),
                    ]),
                ));
                return Ok(MethodOutcome::Domain(error));
            }
        };
        let now = now_secs();
        let created_at = previous.as_ref().map_or(now, |record| record.created_at);
        let record = ReplicaRecord {
            replica_id: replica_id.clone(),
            content_id: content_id.clone(),
            publication_id: publication_id.clone(),
            state: ReplicaState::Active,
            source_peer,
            verified_size_bytes: Some(size),
            last_verified_at: Some(now),
            last_checked_at: Some(now),
            last_seen_at: None,
            failure_count: 0,
            last_error: None,
            announced: false,
            created_at,
            updated_at: now,
        };
        if let Err(error) = self.store.upsert_replica(&record) {
            return Ok(MethodOutcome::Domain(DomainError::from_store(&error)));
        }
        if let Err(error) =
            self.store
                .add_reference(&content_id, nexus_store::ReferenceType::ActiveReplica, &replica_id)
        {
            return Ok(MethodOutcome::Domain(DomainError::from_store(&error)));
        }
        let (op_id, token) = self.ops.begin();
        let context = self.replica_context();
        let ops = Arc::clone(&self.ops);
        let worker_id = replica_id.clone();
        let worker_content = content_id.clone();
        self.pool.submit(move || {
            announce_replica(&context, &ops, op_id, token, worker_id, worker_content);
        })?;
        Ok(MethodOutcome::Result(Value::Map(vec![
            (Value::Text("op_id".to_owned()), Value::Unsigned(op_id)),
            (Value::Text("accepted".to_owned()), Value::Bool(true)),
            (
                Value::Text("replica".to_owned()),
                replica_value(&record, self.hosting.as_ref()),
            ),
            (Value::Text("usage".to_owned()), self.usage_value()?),
            (Value::Text("attribution_preserved".to_owned()), Value::Bool(true)),
            (Value::Text("limits".to_owned()), Value::Text(REPLICA_LIMITS.to_owned())),
        ])))
    }

    /// Lists replicas with a stable cursor and aggregate usage.
    pub(crate) fn replica_list(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let limit = optional_unsigned(params, "limit")?.unwrap_or(50);
        if !(1..=100).contains(&limit) {
            return Err(CoreError::InvalidArgument("limit must be between 1 and 100"));
        }
        let include_removed = matches!(map_get(params, "include_removed"), Some(Value::Bool(true)));
        let cursor = optional_text(params, "cursor")?;
        let page = match self.store.replicas_page(
            cursor.as_deref(),
            usize::try_from(limit).unwrap_or(usize::MAX),
            include_removed,
        ) {
            Ok(page) => page,
            Err(error) => return Ok(MethodOutcome::Domain(DomainError::from_store(&error))),
        };
        let next_cursor = if page.len() == usize::try_from(limit).unwrap_or(usize::MAX) {
            page.last()
                .map_or(Value::Null, |record| Value::Text(record.replica_id.clone()))
        } else {
            Value::Null
        };
        Ok(MethodOutcome::Result(Value::Map(vec![
            (
                Value::Text("replicas".to_owned()),
                Value::Array(
                    page.iter()
                        .map(|record| replica_value(record, self.hosting.as_ref()))
                        .collect(),
                ),
            ),
            (Value::Text("next_cursor".to_owned()), next_cursor),
            (
                Value::Text("total".to_owned()),
                Value::Unsigned(u64::try_from(self.store.replica_count()?).unwrap_or(u64::MAX)),
            ),
            (Value::Text("usage".to_owned()), self.usage_value()?),
            (Value::Text("hosting".to_owned()), self.hosting_value()),
        ])))
    }

    /// Suspends hosting without deleting bytes or the original publication.
    pub(crate) fn replica_pause(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let replica_id = match required_replica_id(params) {
            Ok(replica_id) => replica_id,
            Err(error) => return Ok(MethodOutcome::Domain(DomainError::from(error))),
        };
        let record = match self.store.replica(&replica_id) {
            Ok(record) => record,
            Err(error) => return Ok(MethodOutcome::Domain(DomainError::from_store(&error))),
        };
        if record.state == ReplicaState::Paused {
            return Ok(MethodOutcome::Result(self.replica_mutation_value(&record)?));
        }
        if record.state == ReplicaState::Removed {
            return Ok(MethodOutcome::Domain(DomainError::new(
                DomainCode::Conflict,
                "The replica has already been removed.",
            )));
        }
        let now = now_secs();
        if let Err(error) = self.store.set_replica_state(&replica_id, ReplicaState::Paused, now) {
            return Ok(MethodOutcome::Domain(DomainError::from_store(&error)));
        }
        let _ = self.store.mark_replica_announced(&replica_id, false, now);
        let record = self.store.replica(&replica_id).unwrap_or(record);
        self.try_emit(PendingEvent::durable(
            "replica.paused",
            Value::Map(vec![
                (Value::Text("replica_id".to_owned()), Value::Text(replica_id.clone())),
                (
                    Value::Text("content_id".to_owned()),
                    Value::Text(record.content_id.clone()),
                ),
            ]),
        ));
        let context = self.replica_context();
        let content_id = record.content_id.clone();
        let pool = &self.pool;
        pool.submit(move || withdraw_provider_if_unused(&context, &content_id))?;
        Ok(MethodOutcome::Result(self.replica_mutation_value(&record)?))
    }

    /// Re-verifies and resumes a paused or failed replica.
    pub(crate) fn replica_resume(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        self.update_hosting_signals(params)?;
        if let Some(reason) = self.hosting.blocked_reason() {
            return Ok(MethodOutcome::Domain(blocked_error(reason)));
        }
        let replica_id = match required_replica_id(params) {
            Ok(replica_id) => replica_id,
            Err(error) => return Ok(MethodOutcome::Domain(DomainError::from(error))),
        };
        let record = match self.store.replica(&replica_id) {
            Ok(record) => record,
            Err(error) => return Ok(MethodOutcome::Domain(DomainError::from_store(&error))),
        };
        if record.state == ReplicaState::Active {
            return Ok(MethodOutcome::Result(self.replica_mutation_value(&record)?));
        }
        if record.state == ReplicaState::Removed {
            return Ok(MethodOutcome::Domain(DomainError::new(
                DomainCode::Conflict,
                "A removed replica must be added again explicitly.",
            )));
        }
        if let Err(error) = self.store.ensure_quota(0) {
            return Ok(MethodOutcome::Domain(DomainError::from_store(&error)));
        }
        self.try_emit(PendingEvent::durable(
            "replica.verifying",
            Value::Map(vec![
                (Value::Text("replica_id".to_owned()), Value::Text(replica_id.clone())),
                (
                    Value::Text("content_id".to_owned()),
                    Value::Text(record.content_id.clone()),
                ),
            ]),
        ));
        let size = match verify_replica(&self.store, &record.content_id, record.publication_id.as_deref()) {
            Ok(size) => size,
            Err(error) => {
                let _ = self
                    .store
                    .record_replica_check(&replica_id, now_secs(), None, Some(&bounded_error(&error)));
                let _ = self
                    .store
                    .set_replica_state(&replica_id, ReplicaState::VerificationFailed, now_secs());
                let _ = self.store.mark_replica_announced(&replica_id, false, now_secs());
                self.try_emit(PendingEvent::durable(
                    "replica.failed",
                    Value::Map(vec![
                        (Value::Text("replica_id".to_owned()), Value::Text(replica_id)),
                        (Value::Text("error".to_owned()), error_value(&error)),
                    ]),
                ));
                return Ok(MethodOutcome::Domain(error));
            }
        };
        let now = now_secs();
        if let Err(error) = self.store.record_replica_check(&replica_id, now, Some(size), None) {
            return Ok(MethodOutcome::Domain(DomainError::from_store(&error)));
        }
        if let Err(error) = self.store.set_replica_state(&replica_id, ReplicaState::Active, now) {
            return Ok(MethodOutcome::Domain(DomainError::from_store(&error)));
        }
        if let Err(error) = self.store.add_reference(
            &record.content_id,
            nexus_store::ReferenceType::ActiveReplica,
            &replica_id,
        ) {
            return Ok(MethodOutcome::Domain(DomainError::from_store(&error)));
        }
        let record = self.store.replica(&replica_id).unwrap_or(record);
        self.try_emit(PendingEvent::durable(
            "replica.resumed",
            Value::Map(vec![
                (Value::Text("replica_id".to_owned()), Value::Text(replica_id.clone())),
                (
                    Value::Text("content_id".to_owned()),
                    Value::Text(record.content_id.clone()),
                ),
            ]),
        ));
        let (op_id, token) = self.ops.begin();
        let context = self.replica_context();
        let ops = Arc::clone(&self.ops);
        let worker_id = replica_id;
        let worker_content = record.content_id.clone();
        self.pool.submit(move || {
            announce_replica(&context, &ops, op_id, token, worker_id, worker_content);
        })?;
        Ok(MethodOutcome::Result(self.replica_mutation_value(&record)?))
    }

    /// Stops hosting and releases this device's replica reference.
    pub(crate) fn replica_remove(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let confirmation = matches!(map_get(params, "confirmation"), Some(Value::Bool(true)));
        if !confirmation {
            return Ok(MethodOutcome::Domain(DomainError::new(
                DomainCode::Conflict,
                "Removing a hosted replica requires explicit confirmation.",
            )));
        }
        let replica_id = match required_replica_id(params) {
            Ok(replica_id) => replica_id,
            Err(error) => return Ok(MethodOutcome::Domain(DomainError::from(error))),
        };
        let record = match self.store.replica(&replica_id) {
            Ok(record) => record,
            Err(error) => return Ok(MethodOutcome::Domain(DomainError::from_store(&error))),
        };
        if record.state == ReplicaState::Removed {
            return Ok(MethodOutcome::Result(self.replica_mutation_value(&record)?));
        }
        let now = now_secs();
        if let Err(error) = self.store.set_replica_state(&replica_id, ReplicaState::Removed, now) {
            return Ok(MethodOutcome::Domain(DomainError::from_store(&error)));
        }
        let _ = self.store.mark_replica_announced(&replica_id, false, now);
        if let Err(error) = self.store.release_reference(
            &record.content_id,
            nexus_store::ReferenceType::ActiveReplica,
            &replica_id,
            true,
        ) {
            return Ok(MethodOutcome::Domain(DomainError::from_store(&error)));
        }
        let record = self.store.replica(&replica_id).unwrap_or(record);
        self.try_emit(PendingEvent::durable(
            "replica.removed",
            Value::Map(vec![
                (Value::Text("replica_id".to_owned()), Value::Text(replica_id)),
                (
                    Value::Text("content_id".to_owned()),
                    Value::Text(record.content_id.clone()),
                ),
                (
                    Value::Text("publication_id".to_owned()),
                    record
                        .publication_id
                        .as_ref()
                        .map_or(Value::Null, |id| Value::Text(id.clone())),
                ),
                (Value::Text("publication_preserved".to_owned()), Value::Bool(true)),
                (Value::Text("attribution_preserved".to_owned()), Value::Bool(true)),
            ]),
        ));
        let context = self.replica_context();
        let content_id = record.content_id.clone();
        self.pool
            .submit(move || withdraw_provider_if_unused(&context, &content_id))?;
        Ok(MethodOutcome::Result(self.replica_mutation_value(&record)?))
    }

    /// Starts an asynchronous full re-verification of one replica.
    pub(crate) fn replica_verify(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let replica_id = match required_replica_id(params) {
            Ok(replica_id) => replica_id,
            Err(error) => return Ok(MethodOutcome::Domain(DomainError::from(error))),
        };
        let record = match self.store.replica(&replica_id) {
            Ok(record) => record,
            Err(error) => return Ok(MethodOutcome::Domain(DomainError::from_store(&error))),
        };
        if record.state == ReplicaState::Removed {
            return Ok(MethodOutcome::Domain(DomainError::new(
                DomainCode::Conflict,
                "A removed replica cannot be verified.",
            )));
        }
        let (op_id, token) = self.ops.begin();
        let context = self.replica_context();
        let ops = Arc::clone(&self.ops);
        self.pool.submit(move || {
            verify_one_replica(&context, &ops, op_id, token, record, "replica.verified");
        })?;
        Ok(MethodOutcome::Result(Value::Map(vec![
            (Value::Text("op_id".to_owned()), Value::Unsigned(op_id)),
            (Value::Text("accepted".to_owned()), Value::Bool(true)),
        ])))
    }

    /// Returns the effective hosting limits and current device signals.
    pub(crate) fn hosting_limits_get(&self) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        Ok(MethodOutcome::Result(self.hosting_value()))
    }

    /// Updates hosting limits and optional power/network signals.
    pub(crate) fn hosting_limits_set(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        self.update_hosting_signals(params)?;
        let current = self.hosting.limits();
        let upload = optional_unsigned(params, "upload_bytes_per_second")?.unwrap_or(current.upload_bytes_per_second);
        let allow_metered = optional_bool(params, "allow_metered")?.unwrap_or(current.allow_metered);
        let battery =
            optional_unsigned(params, "battery_min_percent")?.unwrap_or(u64::from(current.battery_min_percent));
        let battery_min_percent = u8::try_from(battery)
            .map_err(|_| CoreError::InvalidArgument("battery_min_percent must be between 0 and 100"))?;
        let limits = HostingLimits {
            upload_bytes_per_second: upload,
            allow_metered,
            battery_min_percent,
        };
        if let Err(error) = limits.validate() {
            return Ok(MethodOutcome::Domain(DomainError::from_store(&error)));
        }
        if let Err(error) = self.store.set_hosting_limits(&limits) {
            return Ok(MethodOutcome::Domain(DomainError::from_store(&error)));
        }
        self.hosting.replace_limits(limits);
        if let Some(network) = self.network_snapshot() {
            let _ = network.set_upload_rate(limits.upload_bytes_per_second);
        }
        self.try_emit(PendingEvent::durable(
            "hosting.limits_changed",
            Value::Map(vec![
                (Value::Text("limits".to_owned()), limits_value(limits)),
                (Value::Text("hosting".to_owned()), self.hosting_value()),
            ]),
        ));
        Ok(MethodOutcome::Result(self.hosting_value()))
    }

    /// Starts startup verification and the periodic 7-day maintenance loop.
    pub(crate) fn start_replica_maintenance(&self) {
        let mut slot = self.maintenance.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if slot.is_some() {
            return;
        }
        let stop = Arc::new(CancelToken::new());
        let token = Arc::clone(&stop);
        let context = self.replica_context();
        let thread = std::thread::Builder::new()
            .name("nexus-replica-maintenance".to_owned())
            .spawn(move || {
                run_maintenance_loop(&context, token);
            });
        if let Ok(thread) = thread {
            *slot = Some(MaintenanceHandle {
                stop,
                thread: Some(thread),
            });
        }
    }

    pub(crate) fn stop_replica_maintenance(&self) {
        let handle = self
            .maintenance
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some(mut handle) = handle {
            handle.stop();
        }
    }

    fn replica_context(&self) -> ReplicaContext {
        ReplicaContext {
            store: Arc::clone(&self.store),
            events: Arc::clone(&self.events),
            network: self.network_snapshot(),
            hosting: Arc::clone(&self.hosting),
        }
    }

    fn update_hosting_signals(&self, params: &Value) -> Result<(), CoreError> {
        if let Some(metered) = optional_bool(params, "metered")? {
            self.hosting.set_metered(metered);
        }
        if let Some(percent) = optional_unsigned(params, "battery_percent")? {
            let percent = u8::try_from(percent)
                .map_err(|_| CoreError::InvalidArgument("battery_percent must be between 0 and 100"))?;
            if percent > 100 {
                return Err(CoreError::InvalidArgument("battery_percent must be between 0 and 100"));
            }
            self.hosting.set_battery_percent(Some(percent));
        }
        Ok(())
    }

    fn hosting_value(&self) -> Value {
        let limits = self.hosting.limits();
        Value::Map(vec![
            (Value::Text("limits".to_owned()), limits_value(limits)),
            (Value::Text("metered".to_owned()), Value::Bool(self.hosting.metered())),
            (
                Value::Text("battery_percent".to_owned()),
                self.hosting
                    .battery_percent()
                    .map_or(Value::Null, |percent| Value::Unsigned(u64::from(percent))),
            ),
            (
                Value::Text("serving_allowed".to_owned()),
                Value::Bool(self.hosting.serving_allowed()),
            ),
            (
                Value::Text("blocked_reason".to_owned()),
                self.hosting
                    .blocked_reason()
                    .map_or(Value::Null, |reason| Value::Text(reason.to_owned())),
            ),
            (
                Value::Text("limits_notice".to_owned()),
                Value::Text(REPLICA_LIMITS.to_owned()),
            ),
        ])
    }

    fn usage_value(&self) -> Result<Value, CoreError> {
        let usage = self.store.replica_usage()?;
        Ok(Value::Map(vec![
            (
                Value::Text("total_count".to_owned()),
                Value::Unsigned(usage.total_count),
            ),
            (
                Value::Text("active_count".to_owned()),
                Value::Unsigned(usage.active_count),
            ),
            (
                Value::Text("paused_count".to_owned()),
                Value::Unsigned(usage.paused_count),
            ),
            (
                Value::Text("failed_count".to_owned()),
                Value::Unsigned(usage.failed_count),
            ),
            (
                Value::Text("storage_bytes".to_owned()),
                Value::Unsigned(usage.storage_bytes),
            ),
        ]))
    }

    fn replica_mutation_value(&self, record: &ReplicaRecord) -> Result<Value, CoreError> {
        Ok(Value::Map(vec![
            (
                Value::Text("replica".to_owned()),
                replica_value(record, self.hosting.as_ref()),
            ),
            (Value::Text("usage".to_owned()), self.usage_value()?),
            (Value::Text("hosting".to_owned()), self.hosting_value()),
        ]))
    }
}

struct RequestedReplica {
    content_id: Option<String>,
    publication_id: Option<String>,
    source_peer: Option<String>,
}

impl RequestedReplica {
    fn parse(params: &Value) -> Result<Self, CoreError> {
        let publication_id = optional_text(params, "publication_id")?;
        let content_id = optional_text(params, "content_id")?;
        if publication_id.is_none() && content_id.is_none() {
            return Err(CoreError::InvalidArgument(
                "replica.add requires content_id or publication_id",
            ));
        }
        if let Some(publication_id) = &publication_id
            && decode_hex_id(publication_id).is_none()
        {
            return Err(CoreError::InvalidArgument("publication_id must be 32-byte hex"));
        }
        if let Some(content_id) = &content_id
            && decode_hex_id(content_id).is_none()
        {
            return Err(CoreError::InvalidArgument("content_id must be 32-byte hex"));
        }
        Ok(Self {
            content_id,
            publication_id,
            source_peer: optional_text(params, "source_peer")?,
        })
    }
}

fn resolve_replica(store: &Store, requested: &RequestedReplica) -> Result<(String, Option<String>), DomainError> {
    match &requested.publication_id {
        Some(publication_id) => {
            let publication = store
                .publication(publication_id)
                .map_err(|error| DomainError::from_store(&error))?;
            if let Some(content_id) = &requested.content_id
                && publication.content_id != *content_id
            {
                return Err(DomainError::new(
                    DomainCode::Conflict,
                    "The publication does not describe the requested content.",
                ));
            }
            Ok((publication.content_id, Some(publication_id.clone())))
        }
        None => {
            let content_id = requested
                .content_id
                .clone()
                .ok_or_else(|| DomainError::invalid("A content identifier is required."))?;
            let publication = store
                .publication_for_content(&content_id)
                .map_err(|error| DomainError::from_store(&error))?;
            let Some(raw) = publication else {
                return Ok((content_id, None));
            };
            let signed = SignedPublication::parse(&raw).map_err(|error| protocol_domain_error(&error))?;
            if to_hex(&signed.publication().content_id) != content_id {
                return Err(DomainError::new(
                    DomainCode::VerificationFailed,
                    "The cached publication does not match the requested content.",
                ));
            }
            Ok((content_id, Some(to_hex(&signed.publication_id()))))
        }
    }
}

fn verify_replica(store: &Store, content_id: &str, publication_id: Option<&str>) -> Result<u64, DomainError> {
    let content = store
        .content(content_id)
        .map_err(|error| DomainError::from_store(&error))?;
    if let Err(error) = store.verify_content(content_id) {
        return Err(verification_error(&error));
    }
    let manifest_bytes = store
        .ensure_manifest_for_content(content_id)
        .map_err(|error| DomainError::from_store(&error))?;
    let manifest = ManifestV1::parse_body(&manifest_bytes).map_err(|error| protocol_domain_error(&error))?;
    if to_hex(&manifest.content_id) != content_id || manifest.total_size != content.size_bytes {
        return Err(DomainError::new(
            DomainCode::VerificationFailed,
            "The manifest does not match the hosted content.",
        ));
    }
    if let Some(publication_id) = publication_id {
        let record = store
            .publication(publication_id)
            .map_err(|error| DomainError::from_store(&error))?;
        let signed = SignedPublication::parse(&record.record_bytes).map_err(|error| protocol_domain_error(&error))?;
        if to_hex(&signed.publication_id()) != publication_id {
            return Err(DomainError::new(
                DomainCode::VerificationFailed,
                "The publication record does not match its identifier.",
            ));
        }
        if to_hex(&signed.publication().content_id) != content_id {
            return Err(DomainError::new(
                DomainCode::VerificationFailed,
                "The publication does not describe the hosted content.",
            ));
        }
        manifest
            .validate_for_publication(signed.publication())
            .map_err(|error| protocol_domain_error(&error))?;
    }
    Ok(content.size_bytes)
}

fn verification_error(error: &StoreError) -> DomainError {
    match error {
        StoreError::ContentMissing(_) | StoreError::IntegrityMismatch(_) | StoreError::InvalidManifest(_) => {
            DomainError::new(
                DomainCode::VerificationFailed,
                "The hosted file failed complete verification.",
            )
        }
        _ => DomainError::from_store(error),
    }
}

fn run_maintenance_loop(context: &ReplicaContext, token: Arc<CancelToken>) {
    let mut first = true;
    loop {
        if token.is_cancelled() {
            break;
        }
        run_maintenance_pass(context, &token, first);
        first = false;
        if token.wait(MAINTENANCE_WAKE) {
            break;
        }
    }
}

fn run_maintenance_pass(context: &ReplicaContext, token: &CancelToken, startup: bool) {
    let replicas = match context.store.replicas() {
        Ok(replicas) => replicas,
        Err(_) => {
            emit_maintenance_completed(context, 0, 0, 0, false);
            return;
        }
    };
    let now_ms = now_ms();
    let mut checked = 0_u64;
    let mut failed = 0_u64;
    let mut skipped = 0_u64;
    for record in replicas {
        if token.is_cancelled() {
            emit_maintenance_completed(context, checked, failed, skipped, true);
            return;
        }
        if record.state != ReplicaState::Active {
            continue;
        }
        if !startup
            && record
                .last_verified_at
                .is_some_and(|at| now_ms.saturating_sub(at.saturating_mul(1_000)) < REPLICA_REVERIFY_INTERVAL_MS)
        {
            skipped += 1;
            continue;
        }
        checked += 1;
        match verify_replica(&context.store, &record.content_id, record.publication_id.as_deref()) {
            Ok(size) => {
                let now = now_secs();
                let _ = context
                    .store
                    .record_replica_check(&record.replica_id, now, Some(size), None);
                announce_without_op(context, &record.replica_id, &record.content_id);
            }
            Err(error) => {
                failed += 1;
                let now = now_secs();
                let _ = context
                    .store
                    .record_replica_check(&record.replica_id, now, None, Some(&bounded_error(&error)));
                let _ = context
                    .store
                    .set_replica_state(&record.replica_id, ReplicaState::VerificationFailed, now);
                let _ = context.store.mark_replica_announced(&record.replica_id, false, now);
                withdraw_provider_if_unused(context, &record.content_id);
                emit_event(
                    &context.events,
                    None,
                    "replica.verification_failed",
                    Value::Map(vec![
                        (
                            Value::Text("replica_id".to_owned()),
                            Value::Text(record.replica_id.clone()),
                        ),
                        (
                            Value::Text("content_id".to_owned()),
                            Value::Text(record.content_id.clone()),
                        ),
                        (Value::Text("error".to_owned()), error_value(&error)),
                    ]),
                );
            }
        }
    }
    emit_maintenance_completed(context, checked, failed, skipped, false);
}

fn emit_maintenance_completed(context: &ReplicaContext, checked: u64, failed: u64, skipped: u64, cancelled: bool) {
    emit_event(
        &context.events,
        None,
        "replica.maintenance_completed",
        Value::Map(vec![
            (Value::Text("checked".to_owned()), Value::Unsigned(checked)),
            (Value::Text("failed".to_owned()), Value::Unsigned(failed)),
            (Value::Text("skipped".to_owned()), Value::Unsigned(skipped)),
            (Value::Text("cancelled".to_owned()), Value::Bool(cancelled)),
        ]),
    );
}

fn announce_replica(
    context: &ReplicaContext,
    ops: &OperationRegistry,
    op_id: u64,
    token: Arc<CancelToken>,
    replica_id: String,
    content_id: String,
) {
    if token.is_cancelled() {
        emit_event(
            &context.events,
            Some(op_id),
            "replica.cancelled",
            Value::Map(vec![
                (Value::Text("replica_id".to_owned()), Value::Text(replica_id)),
                (Value::Text("action".to_owned()), Value::Text("announce".to_owned())),
            ]),
        );
        ops.finish(op_id);
        return;
    }
    let announced = start_providing(context, &content_id);
    let now = now_secs();
    let _ = context
        .store
        .mark_replica_announced(&replica_id, announced.is_ok(), now);
    let mut entries = vec![
        (Value::Text("replica_id".to_owned()), Value::Text(replica_id)),
        (Value::Text("content_id".to_owned()), Value::Text(content_id)),
        (Value::Text("state".to_owned()), Value::Text("active".to_owned())),
        (Value::Text("announced".to_owned()), Value::Bool(announced.is_ok())),
        (Value::Text("verified".to_owned()), Value::Bool(true)),
        (Value::Text("attribution_preserved".to_owned()), Value::Bool(true)),
    ];
    if let Err(error) = announced {
        entries.push((Value::Text("error".to_owned()), error_value(&error)));
    }
    emit_event(&context.events, Some(op_id), "replica.active", Value::Map(entries));
    ops.finish(op_id);
}

fn verify_one_replica(
    context: &ReplicaContext,
    ops: &OperationRegistry,
    op_id: u64,
    token: Arc<CancelToken>,
    record: ReplicaRecord,
    success_event: &'static str,
) {
    emit_event(
        &context.events,
        Some(op_id),
        "replica.verifying",
        Value::Map(vec![
            (
                Value::Text("replica_id".to_owned()),
                Value::Text(record.replica_id.clone()),
            ),
            (
                Value::Text("content_id".to_owned()),
                Value::Text(record.content_id.clone()),
            ),
        ]),
    );
    if token.is_cancelled() {
        emit_event(
            &context.events,
            Some(op_id),
            "replica.cancelled",
            Value::Map(vec![
                (
                    Value::Text("replica_id".to_owned()),
                    Value::Text(record.replica_id.clone()),
                ),
                (Value::Text("action".to_owned()), Value::Text("verify".to_owned())),
            ]),
        );
        ops.finish(op_id);
        return;
    }
    match verify_replica(&context.store, &record.content_id, record.publication_id.as_deref()) {
        Ok(size) => {
            let now = now_secs();
            let _ = context
                .store
                .record_replica_check(&record.replica_id, now, Some(size), None);
            // Verification recovers a failed replica, but a paused replica
            // stays paused: pausing is an explicit user choice.
            let resumed = record.state == ReplicaState::VerificationFailed;
            if resumed {
                let _ = context
                    .store
                    .set_replica_state(&record.replica_id, ReplicaState::Active, now);
                let _ = context.store.add_reference(
                    &record.content_id,
                    nexus_store::ReferenceType::ActiveReplica,
                    &record.replica_id,
                );
            }
            let active = resumed || record.state == ReplicaState::Active;
            let announced = if active {
                start_providing(context, &record.content_id)
            } else {
                Ok(())
            };
            let _ = context
                .store
                .mark_replica_announced(&record.replica_id, active && announced.is_ok(), now);
            let mut entries = vec![
                (
                    Value::Text("replica_id".to_owned()),
                    Value::Text(record.replica_id.clone()),
                ),
                (
                    Value::Text("content_id".to_owned()),
                    Value::Text(record.content_id.clone()),
                ),
                (Value::Text("verified".to_owned()), Value::Bool(true)),
                (Value::Text("verified_size_bytes".to_owned()), Value::Unsigned(size)),
                (
                    Value::Text("state".to_owned()),
                    Value::Text(if active { "active" } else { "paused" }.to_owned()),
                ),
                (
                    Value::Text("announced".to_owned()),
                    Value::Bool(active && announced.is_ok()),
                ),
                (
                    Value::Text("checked_at_ms".to_owned()),
                    Value::Unsigned(now.saturating_mul(1_000)),
                ),
            ];
            if let Err(error) = announced {
                entries.push((Value::Text("error".to_owned()), error_value(&error)));
            }
            emit_event(&context.events, Some(op_id), success_event, Value::Map(entries));
        }
        Err(error) => {
            let now = now_secs();
            let _ = context
                .store
                .record_replica_check(&record.replica_id, now, None, Some(&bounded_error(&error)));
            let _ = context
                .store
                .set_replica_state(&record.replica_id, ReplicaState::VerificationFailed, now);
            let _ = context.store.mark_replica_announced(&record.replica_id, false, now);
            withdraw_provider_if_unused(context, &record.content_id);
            emit_event(
                &context.events,
                Some(op_id),
                "replica.verification_failed",
                Value::Map(vec![
                    (
                        Value::Text("replica_id".to_owned()),
                        Value::Text(record.replica_id.clone()),
                    ),
                    (
                        Value::Text("content_id".to_owned()),
                        Value::Text(record.content_id.clone()),
                    ),
                    (Value::Text("error".to_owned()), error_value(&error)),
                ]),
            );
        }
    }
    ops.finish(op_id);
}

fn announce_without_op(context: &ReplicaContext, replica_id: &str, content_id: &str) {
    let announced = start_providing(context, content_id);
    let _ = context
        .store
        .mark_replica_announced(replica_id, announced.is_ok(), now_secs());
    emit_event(
        &context.events,
        None,
        "replica.active",
        Value::Map(vec![
            (Value::Text("replica_id".to_owned()), Value::Text(replica_id.to_owned())),
            (Value::Text("content_id".to_owned()), Value::Text(content_id.to_owned())),
            (Value::Text("state".to_owned()), Value::Text("active".to_owned())),
            (Value::Text("announced".to_owned()), Value::Bool(announced.is_ok())),
            (Value::Text("verified".to_owned()), Value::Bool(true)),
        ]),
    );
}

fn start_providing(context: &ReplicaContext, content_id: &str) -> Result<(), DomainError> {
    if let Some(reason) = context.hosting.blocked_reason() {
        return Err(blocked_error(reason));
    }
    let Some(content_id) = decode_hex_id(content_id) else {
        return Err(DomainError::invalid("The content identifier is invalid."));
    };
    let Some(network) = &context.network else {
        return Err(network_domain_error(&NetError::Disabled));
    };
    network
        .start_providing(&content_id)
        .map_err(|error| network_domain_error(&error))
}

fn withdraw_provider_if_unused(context: &ReplicaContext, content_id: &str) {
    let Ok(serveable) = context.store.content_serveable(content_id) else {
        return;
    };
    if serveable {
        return;
    }
    let Some(content_id) = decode_hex_id(content_id) else {
        return;
    };
    if let Some(network) = &context.network {
        let _ = network.stop_providing(&content_id);
    }
}

fn replica_value(record: &ReplicaRecord, hosting: &HostingState) -> Value {
    Value::Map(vec![
        (
            Value::Text("replica_id".to_owned()),
            Value::Text(record.replica_id.clone()),
        ),
        (
            Value::Text("content_id".to_owned()),
            Value::Text(record.content_id.clone()),
        ),
        (
            Value::Text("publication_id".to_owned()),
            record
                .publication_id
                .as_ref()
                .map_or(Value::Null, |id| Value::Text(id.clone())),
        ),
        (
            Value::Text("state".to_owned()),
            Value::Text(record.state.as_str().to_owned()),
        ),
        (
            Value::Text("source_peer".to_owned()),
            record
                .source_peer
                .as_ref()
                .map_or(Value::Null, |peer| Value::Text(peer.clone())),
        ),
        (
            Value::Text("verified_size_bytes".to_owned()),
            record.verified_size_bytes.map_or(Value::Null, Value::Unsigned),
        ),
        (
            Value::Text("last_verified_at_ms".to_owned()),
            record
                .last_verified_at
                .map_or(Value::Null, |at| Value::Unsigned(at.saturating_mul(1_000))),
        ),
        (
            Value::Text("last_checked_at_ms".to_owned()),
            record
                .last_checked_at
                .map_or(Value::Null, |at| Value::Unsigned(at.saturating_mul(1_000))),
        ),
        (
            Value::Text("last_seen_at_ms".to_owned()),
            record
                .last_seen_at
                .map_or(Value::Null, |at| Value::Unsigned(at.saturating_mul(1_000))),
        ),
        (
            Value::Text("failure_count".to_owned()),
            Value::Unsigned(record.failure_count),
        ),
        (
            Value::Text("last_error".to_owned()),
            record
                .last_error
                .as_ref()
                .map_or(Value::Null, |error| Value::Text(error.clone())),
        ),
        (Value::Text("announced".to_owned()), Value::Bool(record.announced)),
        (
            Value::Text("serving".to_owned()),
            Value::Bool(record.state == ReplicaState::Active && hosting.serving_allowed()),
        ),
        (Value::Text("attribution_preserved".to_owned()), Value::Bool(true)),
        (
            Value::Text("created_at_ms".to_owned()),
            Value::Unsigned(record.created_at.saturating_mul(1_000)),
        ),
        (
            Value::Text("updated_at_ms".to_owned()),
            Value::Unsigned(record.updated_at.saturating_mul(1_000)),
        ),
    ])
}

fn limits_value(limits: HostingLimits) -> Value {
    Value::Map(vec![
        (
            Value::Text("upload_bytes_per_second".to_owned()),
            Value::Unsigned(limits.upload_bytes_per_second),
        ),
        (
            Value::Text("allow_metered".to_owned()),
            Value::Bool(limits.allow_metered),
        ),
        (
            Value::Text("battery_min_percent".to_owned()),
            Value::Unsigned(u64::from(limits.battery_min_percent)),
        ),
    ])
}

fn blocked_error(reason: &'static str) -> DomainError {
    let message = match reason {
        "metered_network" => "Hosting is paused because this network is metered.",
        "battery_low" => "Hosting is paused because the battery is below the configured minimum.",
        _ => "Hosting is paused by the current device policy.",
    };
    DomainError::new(DomainCode::Conflict, message).with_details(Value::Map(vec![(
        Value::Text("reason".to_owned()),
        Value::Text(reason.to_owned()),
    )]))
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

fn bounded_error(error: &DomainError) -> String {
    nexus_observability::text(&error.message).chars().take(512).collect()
}

fn replica_id_for(content_id: &str) -> String {
    format!("replica-{content_id}")
}

fn required_replica_id(params: &Value) -> Result<String, CoreError> {
    let replica_id =
        optional_text(params, "replica_id")?.ok_or(CoreError::InvalidArgument("replica_id is required"))?;
    if replica_id.len() < 9 || replica_id.len() > 128 || !replica_id.is_ascii() {
        return Err(CoreError::InvalidArgument("replica_id is out of bounds"));
    }
    Ok(replica_id)
}

fn emit_event(events: &EventQueue, op_id: Option<u64>, event_type: &str, data: Value) {
    let event = PendingEvent::durable(event_type, data);
    let event = match op_id {
        Some(op_id) => event.with_op(op_id),
        None => event,
    };
    let _ = events.push(event);
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

fn optional_bool(value: &Value, key: &str) -> Result<Option<bool>, CoreError> {
    match map_get(value, key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_bool()
            .map(Some)
            .ok_or(CoreError::InvalidArgument("a boolean field has the wrong type")),
    }
}

fn decode_hex_id(text: &str) -> Option<[u8; 32]> {
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
