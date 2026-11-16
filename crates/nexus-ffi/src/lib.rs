//! Stable C ABI surface for the Ember core.
//!
//! Every exported function validates arguments, catches Rust panics, and
//! returns a defined [`NexusStatus`]. Domain failures for method invocations
//! travel inside the versioned CBOR response envelope instead of the status.
//!
//! # Memory ownership
//!
//! Output buffers are allocated by this crate and must be released exactly
//! once with [`nexus_core_free_buffer`]. Input buffers are borrowed for the
//! duration of the call only.

#![allow(unsafe_code)]

mod jni_bridge;
mod registry;

use std::ffi::c_char;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

use nexus_core::{
    CoreConfig, CoreError, Engine, EnvelopeError, ImportError, MAX_CONFIG_BYTES, MAX_REQUEST_BYTES, MAX_STREAM_CHUNK,
    StoreError,
};

use registry as handles;

/// C ABI status.
#[repr(i32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NexusStatus {
    /// Success.
    Ok = 0,
    /// No event arrived before the timeout.
    Timeout = 1,
    /// The handle is unknown or was already destroyed.
    InvalidHandle = 2,
    /// A pointer, length, method ID, or config value is invalid.
    InvalidArgument = 3,
    /// The host and library ABI versions differ.
    AbiMismatch = 4,
    /// The event queue is closed because the core is stopping or stopped.
    QueueClosed = 5,
    /// A guarded internal fault.
    Internal = 6,
    /// An input or output exceeded the ABI size limit.
    BufferTooLarge = 7,
    /// The build does not support the requested method.
    Unsupported = 8,
}

/// Length-delimited output buffer owned by the core.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct NexusBuffer {
    /// Buffer start, or null when `len` is zero.
    pub ptr: *mut u8,
    /// Exact byte length.
    pub len: usize,
}

impl NexusBuffer {
    const EMPTY: Self = Self {
        ptr: ptr::null_mut(),
        len: 0,
    };
}

fn status_for(error: &CoreError) -> NexusStatus {
    match error {
        CoreError::Config(error) => match error {
            nexus_core::ConfigError::AbiMismatch { .. } => NexusStatus::AbiMismatch,
            nexus_core::ConfigError::TooLarge { .. } => NexusStatus::BufferTooLarge,
            _ => NexusStatus::InvalidArgument,
        },
        CoreError::Envelope(error) => match error {
            EnvelopeError::TooLarge { .. } => NexusStatus::BufferTooLarge,
            _ => NexusStatus::InvalidArgument,
        },
        CoreError::InvalidArgument(_) => NexusStatus::InvalidArgument,
        CoreError::Unsupported(_) => NexusStatus::Unsupported,
        CoreError::Event(error) => match error {
            nexus_core::EventError::Closed => NexusStatus::QueueClosed,
            nexus_core::EventError::TooLarge { .. } => NexusStatus::BufferTooLarge,
            _ => NexusStatus::Internal,
        },
        CoreError::Import(error) => match error {
            ImportError::ChunkTooLarge => NexusStatus::BufferTooLarge,
            ImportError::SizeMismatch { .. }
            | ImportError::SizeExceeded { .. }
            | ImportError::Empty
            | ImportError::Manifest(_) => NexusStatus::InvalidArgument,
            ImportError::Io(_) => NexusStatus::Internal,
        },
        CoreError::Store(error) => match error {
            StoreError::Unsupported(_) => NexusStatus::Unsupported,
            StoreError::FileTooLarge { .. }
            | StoreError::QuotaExceeded { .. }
            | StoreError::FreeSpaceReserve { .. }
            | StoreError::ContentMissing(_)
            | StoreError::InvalidContentId(_)
            | StoreError::InvalidId(_)
            | StoreError::InvalidFilename(_)
            | StoreError::InvalidManifest(_)
            | StoreError::InvalidRecord(_)
            | StoreError::IntegrityMismatch(_)
            | StoreError::ManifestMissing(_)
            | StoreError::RecordConflict(_)
            | StoreError::RecordNotFound(_)
            | StoreError::ConfirmationRequired(_)
            | StoreError::DestinationExists(_)
            | StoreError::UnsafePath(_)
            | StoreError::InvalidConfig(_) => NexusStatus::InvalidArgument,
            _ => NexusStatus::Internal,
        },
        CoreError::Network(error) => match error.code() {
            "invalid_config" | "invalid_address" | "missing_peer_id" | "invalid_peer_id" | "message_too_large" => {
                NexusStatus::InvalidArgument
            }
            _ => NexusStatus::Internal,
        },
        CoreError::NotRunning => NexusStatus::InvalidArgument,
        CoreError::Io(_) | CoreError::Identity(_) | CoreError::Internal(_) => NexusStatus::Internal,
    }
}

fn guarded(action: impl FnOnce() -> NexusStatus) -> NexusStatus {
    // Panic payloads are deliberately not surfaced across the ABI.
    catch_unwind(AssertUnwindSafe(action)).unwrap_or(NexusStatus::Internal)
}

fn into_buffer(bytes: Vec<u8>) -> NexusBuffer {
    if bytes.is_empty() {
        return NexusBuffer::EMPTY;
    }
    let boxed = bytes.into_boxed_slice();
    let len = boxed.len();
    NexusBuffer {
        ptr: Box::into_raw(boxed).cast::<u8>(),
        len,
    }
}

fn create_core(config_cbor: &[u8], local_secret: &[u8]) -> Result<u64, CoreError> {
    let config = CoreConfig::parse(config_cbor)?;
    let engine = Engine::create(config, local_secret)?;
    Ok(handles::insert(engine))
}

fn engine_for(handle: u64) -> Result<std::sync::Arc<Engine>, NexusStatus> {
    handles::get(handle).ok_or(NexusStatus::InvalidHandle)
}

/// Returns the native ABI version.
#[unsafe(no_mangle)]
pub extern "C" fn nexus_core_abi_version() -> u32 {
    nexus_core::ABI_VERSION
}

/// Returns a static semantic-version string valid for the process lifetime.
#[unsafe(no_mangle)]
pub extern "C" fn nexus_core_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast()
}

/// Creates a core instance.
///
/// # Safety
///
/// `config_cbor` and `local_secret` must be valid readable buffers of their
/// declared lengths, and `out_handle` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nexus_core_create(
    config_cbor: *const u8,
    config_len: usize,
    local_secret: *const u8,
    local_secret_len: usize,
    out_handle: *mut u64,
) -> NexusStatus {
    guarded(|| {
        if out_handle.is_null() {
            return NexusStatus::InvalidArgument;
        }
        // SAFETY: required by the caller contract.
        unsafe {
            out_handle.write(0);
        }
        if config_len > MAX_CONFIG_BYTES {
            return NexusStatus::BufferTooLarge;
        }
        if local_secret_len != nexus_identity::LOCAL_ENCRYPTION_KEY_LENGTH {
            return NexusStatus::InvalidArgument;
        }
        let Some(config) = read_input(config_cbor, config_len) else {
            return NexusStatus::InvalidArgument;
        };
        let Some(secret) = read_input(local_secret, local_secret_len) else {
            return NexusStatus::InvalidArgument;
        };
        match create_handle(config, secret) {
            (NexusStatus::Ok, handle) => {
                // SAFETY: checked non-null above.
                unsafe {
                    out_handle.write(handle);
                }
                NexusStatus::Ok
            }
            (status, _) => status,
        }
    })
}

/// Starts the core. Idempotent.
#[unsafe(no_mangle)]
pub extern "C" fn nexus_core_start(handle: u64) -> NexusStatus {
    guarded(|| start_call(handle))
}

/// Stops the core and flushes durable state. Idempotent.
#[unsafe(no_mangle)]
pub extern "C" fn nexus_core_stop(handle: u64) -> NexusStatus {
    guarded(|| stop_call(handle))
}

/// Stops if needed and releases the handle.
#[unsafe(no_mangle)]
pub extern "C" fn nexus_core_destroy(handle: u64) -> NexusStatus {
    guarded(|| destroy_call(handle))
}

/// Invokes a method ID with a CBOR request envelope.
///
/// # Safety
///
/// `request_cbor` must be a readable buffer of `request_len` bytes and
/// `out_response` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nexus_core_invoke(
    handle: u64,
    method_id: u32,
    request_cbor: *const u8,
    request_len: usize,
    out_response: *mut NexusBuffer,
) -> NexusStatus {
    guarded(|| {
        if out_response.is_null() {
            return NexusStatus::InvalidArgument;
        }
        // SAFETY: required by the caller contract.
        unsafe {
            out_response.write(NexusBuffer::EMPTY);
        }
        if request_len > MAX_REQUEST_BYTES {
            return NexusStatus::BufferTooLarge;
        }
        let Some(request) = read_input(request_cbor, request_len) else {
            return NexusStatus::InvalidArgument;
        };
        let (status, response) = invoke_call(handle, method_id, request);
        if status == NexusStatus::Ok {
            // SAFETY: checked non-null above.
            unsafe {
                out_response.write(into_buffer(response));
            }
        }
        status
    })
}

/// Waits up to `timeout_ms` for the next CBOR event envelope.
///
/// # Safety
///
/// `out_event` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nexus_core_next_event(
    handle: u64,
    timeout_ms: u32,
    out_event: *mut NexusBuffer,
) -> NexusStatus {
    guarded(|| {
        if out_event.is_null() {
            return NexusStatus::InvalidArgument;
        }
        // SAFETY: required by the caller contract.
        unsafe {
            out_event.write(NexusBuffer::EMPTY);
        }
        let (status, event) = next_event_call(handle, timeout_ms);
        if status == NexusStatus::Ok {
            // SAFETY: checked non-null above.
            unsafe {
                out_event.write(into_buffer(event));
            }
        }
        status
    })
}

/// Requests cooperative cancellation of an asynchronous operation.
#[unsafe(no_mangle)]
pub extern "C" fn nexus_core_cancel(handle: u64, op_id: u64) -> NexusStatus {
    guarded(|| cancel_call(handle, op_id))
}

/// Frees a buffer returned by the core.
///
/// # Safety
///
/// `buffer` must come from this library and must not have been freed before.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nexus_core_free_buffer(buffer: NexusBuffer) {
    if buffer.ptr.is_null() || buffer.len == 0 {
        return;
    }
    // SAFETY: the pointer was produced by `Box::into_raw` over a `Box<[u8]>`
    // with exactly this length and has not been freed.
    unsafe {
        let slice = ptr::slice_from_raw_parts_mut(buffer.ptr, buffer.len);
        drop(Box::from_raw(slice));
    }
}

/// Starts a streaming import.
///
/// # Safety
///
/// `display_name_utf8` must be readable for `display_name_len` bytes and
/// `out_import_id` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nexus_core_import_begin(
    handle: u64,
    display_name_utf8: *const u8,
    display_name_len: usize,
    expected_size: u64,
    out_import_id: *mut u64,
) -> NexusStatus {
    guarded(|| {
        if out_import_id.is_null() {
            return NexusStatus::InvalidArgument;
        }
        // SAFETY: required by the caller contract.
        unsafe {
            out_import_id.write(0);
        }
        let Some(name) = read_input(display_name_utf8, display_name_len) else {
            return NexusStatus::InvalidArgument;
        };
        let Ok(name) = std::str::from_utf8(name) else {
            return NexusStatus::InvalidArgument;
        };
        if name.is_empty() || expected_size == 0 {
            return NexusStatus::InvalidArgument;
        }
        let (status, import_id) = import_begin_call(handle, name, expected_size);
        if status == NexusStatus::Ok {
            // SAFETY: checked non-null above.
            unsafe {
                out_import_id.write(import_id);
            }
        }
        status
    })
}

/// Writes at most 1 MiB to an import.
///
/// # Safety
///
/// `data` must be readable for `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nexus_core_import_write(
    handle: u64,
    import_id: u64,
    data: *const u8,
    len: usize,
) -> NexusStatus {
    guarded(|| {
        if len > MAX_STREAM_CHUNK {
            return NexusStatus::BufferTooLarge;
        }
        let Some(bytes) = read_input(data, len) else {
            return NexusStatus::InvalidArgument;
        };
        import_write_call(handle, import_id, bytes)
    })
}

/// Finalizes an import and returns the CBOR summary.
///
/// # Safety
///
/// `out_summary` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nexus_core_import_finish(
    handle: u64,
    import_id: u64,
    out_summary: *mut NexusBuffer,
) -> NexusStatus {
    guarded(|| {
        if out_summary.is_null() {
            return NexusStatus::InvalidArgument;
        }
        // SAFETY: required by the caller contract.
        unsafe {
            out_summary.write(NexusBuffer::EMPTY);
        }
        let (status, summary) = import_finish_call(handle, import_id);
        if status == NexusStatus::Ok {
            // SAFETY: checked non-null above.
            unsafe {
                out_summary.write(into_buffer(summary));
            }
        }
        status
    })
}

/// Aborts an import and removes its temporary file.
#[unsafe(no_mangle)]
pub extern "C" fn nexus_core_import_abort(handle: u64, import_id: u64) -> NexusStatus {
    guarded(|| import_abort_call(handle, import_id))
}

/// Opens a read cursor for a verified 32-byte content ID.
///
/// # Safety
///
/// `content_id` must be readable for `content_id_len` bytes and
/// `out_export_id` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nexus_core_export_open(
    handle: u64,
    content_id: *const u8,
    content_id_len: usize,
    out_export_id: *mut u64,
) -> NexusStatus {
    guarded(|| {
        if out_export_id.is_null() {
            return NexusStatus::InvalidArgument;
        }
        // SAFETY: required by the caller contract.
        unsafe {
            out_export_id.write(0);
        }
        if content_id_len != 32 {
            return NexusStatus::InvalidArgument;
        }
        let Some(content_id) = read_input(content_id, content_id_len) else {
            return NexusStatus::InvalidArgument;
        };
        let (status, export_id) = export_open_call(handle, content_id);
        if status == NexusStatus::Ok {
            // SAFETY: checked non-null above.
            unsafe {
                out_export_id.write(export_id);
            }
        }
        status
    })
}

/// Reads at most `max_len` bytes from an export cursor.
///
/// # Safety
///
/// `out_chunk` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nexus_core_export_read(
    handle: u64,
    export_id: u64,
    max_len: u32,
    out_chunk: *mut NexusBuffer,
) -> NexusStatus {
    guarded(|| {
        if out_chunk.is_null() {
            return NexusStatus::InvalidArgument;
        }
        // SAFETY: required by the caller contract.
        unsafe {
            out_chunk.write(NexusBuffer::EMPTY);
        }
        let max_len = max_len as usize;
        if max_len > MAX_STREAM_CHUNK {
            return NexusStatus::BufferTooLarge;
        }
        let (status, chunk) = export_read_call(handle, export_id, u32::try_from(max_len).unwrap_or(u32::MAX));
        if status == NexusStatus::Ok {
            // SAFETY: checked non-null above.
            unsafe {
                out_chunk.write(into_buffer(chunk));
            }
        }
        status
    })
}

/// Closes an export cursor.
#[unsafe(no_mangle)]
pub extern "C" fn nexus_core_export_close(handle: u64, export_id: u64) -> NexusStatus {
    guarded(|| export_close_call(handle, export_id))
}

fn read_input<'a>(data: *const u8, len: usize) -> Option<&'a [u8]> {
    if len == 0 {
        return Some(&[]);
    }
    if data.is_null() {
        return None;
    }
    // SAFETY: the caller contract guarantees a readable buffer of `len` bytes.
    Some(unsafe { std::slice::from_raw_parts(data, len) })
}

// Safe helpers shared by the JNI shim. They accept owned slices and therefore
// avoid exposing raw pointers to the JNI layer.

pub(crate) fn create_handle(config: &[u8], secret: &[u8]) -> (NexusStatus, u64) {
    if config.len() > MAX_CONFIG_BYTES || secret.len() != nexus_identity::LOCAL_ENCRYPTION_KEY_LENGTH {
        return (NexusStatus::InvalidArgument, 0);
    }
    match create_core(config, secret) {
        Ok(handle) => (NexusStatus::Ok, handle),
        Err(error) => (status_for(&error), 0),
    }
}

pub(crate) fn start_call(handle: u64) -> NexusStatus {
    match engine_for(handle) {
        Ok(engine) => engine
            .start()
            .map_or_else(|error| status_for(&error), |()| NexusStatus::Ok),
        Err(status) => status,
    }
}

pub(crate) fn stop_call(handle: u64) -> NexusStatus {
    match engine_for(handle) {
        Ok(engine) => engine
            .stop()
            .map_or_else(|error| status_for(&error), |()| NexusStatus::Ok),
        Err(status) => status,
    }
}

pub(crate) fn destroy_call(handle: u64) -> NexusStatus {
    match handles::remove(handle) {
        Some(engine) => {
            engine.shutdown();
            NexusStatus::Ok
        }
        None => NexusStatus::InvalidHandle,
    }
}

pub(crate) fn invoke_call(handle: u64, method_id: u32, request: &[u8]) -> (NexusStatus, Vec<u8>) {
    if request.len() > MAX_REQUEST_BYTES {
        return (NexusStatus::BufferTooLarge, Vec::new());
    }
    let engine = match engine_for(handle) {
        Ok(engine) => engine,
        Err(status) => return (status, Vec::new()),
    };
    match engine.invoke(method_id, request) {
        Ok(response) => (NexusStatus::Ok, response),
        Err(error) => (status_for(&error), Vec::new()),
    }
}

pub(crate) fn next_event_call(handle: u64, timeout_ms: u32) -> (NexusStatus, Vec<u8>) {
    let engine = match engine_for(handle) {
        Ok(engine) => engine,
        Err(status) => return (status, Vec::new()),
    };
    match engine.next_event(timeout_ms) {
        Ok(Some(event)) => (NexusStatus::Ok, event),
        Ok(None) => (NexusStatus::Timeout, Vec::new()),
        Err(error) => (status_for(&error), Vec::new()),
    }
}

pub(crate) fn cancel_call(handle: u64, op_id: u64) -> NexusStatus {
    match engine_for(handle) {
        Ok(engine) => engine
            .cancel(op_id)
            .map_or_else(|error| status_for(&error), |()| NexusStatus::Ok),
        Err(status) => status,
    }
}

pub(crate) fn import_begin_call(handle: u64, display_name: &str, expected_size: u64) -> (NexusStatus, u64) {
    if display_name.is_empty() || expected_size == 0 {
        return (NexusStatus::InvalidArgument, 0);
    }
    let engine = match engine_for(handle) {
        Ok(engine) => engine,
        Err(status) => return (status, 0),
    };
    match engine.import_begin(display_name, expected_size) {
        Ok(import_id) => (NexusStatus::Ok, import_id),
        Err(error) => (status_for(&error), 0),
    }
}

pub(crate) fn import_write_call(handle: u64, import_id: u64, data: &[u8]) -> NexusStatus {
    if data.len() > MAX_STREAM_CHUNK {
        return NexusStatus::BufferTooLarge;
    }
    match engine_for(handle) {
        Ok(engine) => engine
            .import_write(import_id, data)
            .map_or_else(|error| status_for(&error), |()| NexusStatus::Ok),
        Err(status) => status,
    }
}

pub(crate) fn import_finish_call(handle: u64, import_id: u64) -> (NexusStatus, Vec<u8>) {
    let engine = match engine_for(handle) {
        Ok(engine) => engine,
        Err(status) => return (status, Vec::new()),
    };
    match engine.import_finish(import_id) {
        Ok(summary) => (NexusStatus::Ok, summary),
        Err(error) => (status_for(&error), Vec::new()),
    }
}

pub(crate) fn import_abort_call(handle: u64, import_id: u64) -> NexusStatus {
    match engine_for(handle) {
        Ok(engine) => engine
            .import_abort(import_id)
            .map_or_else(|error| status_for(&error), |()| NexusStatus::Ok),
        Err(status) => status,
    }
}

pub(crate) fn export_open_call(handle: u64, content_id: &[u8]) -> (NexusStatus, u64) {
    if content_id.len() != 32 {
        return (NexusStatus::InvalidArgument, 0);
    }
    let engine = match engine_for(handle) {
        Ok(engine) => engine,
        Err(status) => return (status, 0),
    };
    match engine.export_open(content_id) {
        Ok(export_id) => (NexusStatus::Ok, export_id),
        Err(error) => (status_for(&error), 0),
    }
}

pub(crate) fn export_read_call(handle: u64, export_id: u64, max_len: u32) -> (NexusStatus, Vec<u8>) {
    let max_len = max_len as usize;
    if max_len > MAX_STREAM_CHUNK {
        return (NexusStatus::BufferTooLarge, Vec::new());
    }
    let engine = match engine_for(handle) {
        Ok(engine) => engine,
        Err(status) => return (status, Vec::new()),
    };
    match engine.export_read(export_id, max_len) {
        Ok(chunk) => (NexusStatus::Ok, chunk),
        Err(error) => (status_for(&error), Vec::new()),
    }
}

pub(crate) fn export_close_call(handle: u64, export_id: u64) -> NexusStatus {
    match engine_for(handle) {
        Ok(engine) => {
            engine.export_close(export_id);
            NexusStatus::Ok
        }
        Err(status) => status,
    }
}

#[cfg(test)]
mod tests {
    use super::{NexusBuffer, NexusStatus, guarded, nexus_core_abi_version, nexus_core_free_buffer};

    #[test]
    fn exports_abi_version() {
        assert_eq!(1, nexus_core_abi_version());
    }

    #[test]
    fn panic_is_contained() {
        assert_eq!(guarded(|| panic!("boom")), NexusStatus::Internal);
    }

    #[test]
    fn empty_buffer_is_safe_to_free() {
        // SAFETY: null with zero length is explicitly allowed.
        unsafe {
            nexus_core_free_buffer(NexusBuffer {
                ptr: std::ptr::null_mut(),
                len: 0,
            });
        }
    }
}
