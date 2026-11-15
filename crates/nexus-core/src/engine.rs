//! Core lifecycle, method dispatch, event emission, and streaming orchestration.

use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nexus_crypto::Value;
use nexus_identity::{Identity, IdentityVault, LocalEncryptionKey};
use nexus_store::{MAX_FILE_SIZE, QuotaPolicy, QuotaUsage, Store, StoreConfig, StoreError, safe_display_name};

use crate::config::CoreConfig;
use crate::envelope::{MAX_RESPONSE_BYTES, Request, encode_failure, encode_success, error_value};
use crate::error::{CoreError, DomainCode, DomainError};
use crate::event::{EventQueue, PendingEvent};
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

enum MethodOutcome {
    Result(Value),
    Domain(DomainError),
}

/// One core instance. The FFI registry owns it through an `Arc`.
pub struct Engine {
    config: CoreConfig,
    store: Store,
    state: Mutex<LifecycleState>,
    identity: Mutex<IdentitySlot>,
    secret: zeroize::Zeroizing<Vec<u8>>,
    events: Arc<EventQueue>,
    ops: Arc<OperationRegistry>,
    pool: WorkerPool,
    imports: Mutex<HashMap<u64, ImportSession>>,
    exports: Mutex<HashMap<u64, ExportCursor>>,
    next_import: AtomicU64,
    next_export: AtomicU64,
    dropped_events: AtomicU64,
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
        let pool = WorkerPool::new("nexus-core-op", OPERATION_WORKERS)?;
        Ok(Self {
            config,
            store,
            state: Mutex::new(LifecycleState::Created),
            identity: Mutex::new(IdentitySlot::absent()),
            secret: zeroize::Zeroizing::new(secret.to_vec()),
            events: Arc::new(EventQueue::new()),
            ops: Arc::new(OperationRegistry::new()),
            pool,
            imports: Mutex::new(HashMap::new()),
            exports: Mutex::new(HashMap::new()),
            next_import: AtomicU64::new(1),
            next_export: AtomicU64::new(1),
            dropped_events: AtomicU64::new(0),
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
        Ok(())
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
            0x0401 => self.store_usage().map(MethodOutcome::Result),
            0x0B01 => Ok(MethodOutcome::Result(self.test_echo(&params)?)),
            0x0B02 => self.test_fault(&params),
            _ => Err(CoreError::Unsupported(method_id)),
        }
    }

    fn system_info(&self) -> Result<Value, CoreError> {
        let capabilities: Vec<Value> = ["system.info", "system.health", "system.snapshot", "store.usage"]
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
                Value::Text(self.network_state().to_owned()),
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
            entries.push((
                Value::Text("network".to_owned()),
                Value::Map(vec![
                    (
                        Value::Text("state".to_owned()),
                        Value::Text(self.network_state().to_owned()),
                    ),
                    (Value::Text("peer_count".to_owned()), Value::Unsigned(0)),
                ]),
            ));
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

    fn identity_status(&self) -> IdentityStatus {
        self.identity
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .status
            .clone()
    }

    fn network_state(&self) -> &'static str {
        if self.config.network_enabled {
            "unavailable"
        } else {
            "disabled"
        }
    }

    fn require_running(&self) -> Result<(), CoreError> {
        if self.state() == LifecycleState::Running {
            Ok(())
        } else {
            Err(CoreError::NotRunning)
        }
    }

    /// Emits a lifecycle or identity event without ever blocking the caller.
    fn try_emit(&self, event: PendingEvent) {
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
