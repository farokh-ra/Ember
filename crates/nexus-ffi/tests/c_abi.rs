//! Contract tests for the C ABI, exercised through the exported symbols.
//!
//! These tests cover the same lifecycle, invoke, event, streaming, panic, and
//! error paths as the C harness, so Windows and Android adapters share one
//! behavioural contract.

#![allow(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use nexus_core::{MAX_REQUEST_BYTES, MAX_STREAM_CHUNK, Response, decode_response};
use nexus_crypto::{CborLimits, Value, decode, to_hex};
use nexus_ffi::{
    NexusBuffer, NexusStatus, nexus_core_abi_version, nexus_core_cancel, nexus_core_create, nexus_core_destroy,
    nexus_core_export_close, nexus_core_export_open, nexus_core_export_read, nexus_core_free_buffer,
    nexus_core_import_abort, nexus_core_import_begin, nexus_core_import_finish, nexus_core_import_write,
    nexus_core_invoke, nexus_core_next_event, nexus_core_start, nexus_core_stop, nexus_core_version,
};

static COUNTER: AtomicU64 = AtomicU64::new(1);

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(label: &str) -> Self {
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("nexus-ffi-contract-{}-{sequence}-{label}", std::process::id()));
        fs::create_dir_all(&path).expect("create test root");
        Self { path }
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn config_for(root: &Path, abi_version: u64) -> Vec<u8> {
    Value::Map(vec![
        (Value::Text("abi_version".to_owned()), Value::Unsigned(abi_version)),
        (
            Value::Text("data_root".to_owned()),
            Value::Text(root.to_string_lossy().into_owned()),
        ),
        (Value::Text("enable_test_methods".to_owned()), Value::Bool(true)),
    ])
    .to_canonical_bytes()
    .expect("config CBOR")
}

fn request(params: Value) -> Vec<u8> {
    Value::Map(vec![
        (Value::Text("v".to_owned()), Value::Unsigned(1)),
        (Value::Text("params".to_owned()), params),
    ])
    .to_canonical_bytes()
    .expect("request CBOR")
}

fn empty_params() -> Value {
    Value::Map(Vec::new())
}

fn create(root: &Path) -> (NexusStatus, u64) {
    let config = config_for(root, 1);
    let secret = [9_u8; 32];
    let mut handle = u64::MAX;
    // SAFETY: all buffers are valid for the duration of the call.
    let status = unsafe {
        nexus_core_create(
            config.as_ptr(),
            config.len(),
            secret.as_ptr(),
            secret.len(),
            &mut handle,
        )
    };
    (status, handle)
}

fn take_buffer(buffer: NexusBuffer) -> Vec<u8> {
    if buffer.ptr.is_null() || buffer.len == 0 {
        return Vec::new();
    }
    // SAFETY: the core returned a valid buffer of this length.
    let bytes = unsafe { std::slice::from_raw_parts(buffer.ptr, buffer.len) }.to_vec();
    // SAFETY: the buffer is freed exactly once here.
    unsafe {
        nexus_core_free_buffer(buffer);
    }
    bytes
}

fn invoke(handle: u64, method_id: u32, params: Value) -> (NexusStatus, Vec<u8>) {
    let request = request(params);
    let mut output = NexusBuffer {
        ptr: std::ptr::null_mut(),
        len: 0,
    };
    // SAFETY: request bytes and the output slot are valid.
    let status = unsafe { nexus_core_invoke(handle, method_id, request.as_ptr(), request.len(), &mut output) };
    (status, take_buffer(output))
}

fn next_event(handle: u64, timeout_ms: u32) -> (NexusStatus, Vec<u8>) {
    let mut output = NexusBuffer {
        ptr: std::ptr::null_mut(),
        len: 0,
    };
    // SAFETY: the output slot is valid.
    let status = unsafe { nexus_core_next_event(handle, timeout_ms, &mut output) };
    (status, take_buffer(output))
}

fn success(bytes: &[u8]) -> Value {
    match decode_response(bytes).expect("response envelope") {
        Response::Success(value) => value,
        Response::Failure(error) => panic!("unexpected domain failure: {error:?}"),
    }
}

fn failure(bytes: &[u8]) -> nexus_core::DomainError {
    match decode_response(bytes).expect("response envelope") {
        Response::Failure(error) => error,
        Response::Success(value) => panic!("unexpected success: {value:?}"),
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

fn decode_event(bytes: &[u8]) -> Value {
    decode(bytes, CborLimits::new(16, 4_096, 1024 * 1024)).expect("event CBOR")
}

fn hex_decode(text: &str) -> Vec<u8> {
    assert_eq!(text.len() % 2, 0);
    let mut bytes = Vec::with_capacity(text.len() / 2);
    let raw = text.as_bytes();
    for pair in raw.as_chunks::<2>().0 {
        let high = char::from(pair[0]).to_digit(16).expect("hex");
        let low = char::from(pair[1]).to_digit(16).expect("hex");
        bytes.push(u8::try_from((high << 4) | low).expect("byte"));
    }
    bytes
}

#[test]
fn abi_and_version_are_exported() {
    assert_eq!(nexus_core_abi_version(), 1);
    // SAFETY: the version string is static and NUL-terminated.
    let version = unsafe { std::ffi::CStr::from_ptr(nexus_core_version()) };
    assert_eq!(version.to_str().expect("utf-8"), "0.1.0");
}

#[test]
fn lifecycle_dispatch_and_errors_share_one_contract() {
    let root = TempRoot::new("lifecycle");
    let (status, handle) = create(&root.path);
    assert_eq!(status, NexusStatus::Ok);
    assert_ne!(handle, 0);

    assert_eq!(nexus_core_start(handle), NexusStatus::Ok);
    assert_eq!(nexus_core_start(handle), NexusStatus::Ok);

    let (status, bytes) = invoke(handle, 0x0101, empty_params());
    assert_eq!(status, NexusStatus::Ok);
    let info = success(&bytes);
    assert_eq!(map_get(&info, "abi_version").and_then(Value::as_unsigned), Some(1));
    assert_eq!(
        map_get(&info, "protocol_version").and_then(Value::as_text),
        Some("0.1.0-stage-09")
    );

    let (status, bytes) = invoke(
        handle,
        0x0B01,
        Value::Map(vec![(
            Value::Text("payload".to_owned()),
            Value::Text("c-abi".to_owned()),
        )]),
    );
    assert_eq!(status, NexusStatus::Ok);
    assert_eq!(
        map_get(&success(&bytes), "payload").and_then(Value::as_text),
        Some("c-abi")
    );

    let (status, bytes) = invoke(
        handle,
        0x0B02,
        Value::Map(vec![(
            Value::Text("fault".to_owned()),
            Value::Text("quota_exceeded".to_owned()),
        )]),
    );
    assert_eq!(status, NexusStatus::Ok);
    assert_eq!(failure(&bytes).code, nexus_core::DomainCode::QuotaExceeded);

    // A panic is contained at the ABI boundary and the core stays usable.
    let (status, _) = invoke(
        handle,
        0x0B02,
        Value::Map(vec![(Value::Text("fault".to_owned()), Value::Text("panic".to_owned()))]),
    );
    assert_eq!(status, NexusStatus::Internal);
    let (status, bytes) = invoke(handle, 0x0101, empty_params());
    assert_eq!(status, NexusStatus::Ok);
    assert!(matches!(success(&bytes), Value::Map(_)));

    let (status, _) = invoke(handle, 0x9999, empty_params());
    assert_eq!(status, NexusStatus::Unsupported);

    let mut output = NexusBuffer {
        ptr: std::ptr::null_mut(),
        len: 0,
    };
    // SAFETY: an oversized length is rejected before the null pointer is read.
    let status = unsafe { nexus_core_invoke(handle, 0x0101, std::ptr::null(), MAX_REQUEST_BYTES + 1, &mut output) };
    assert_eq!(status, NexusStatus::BufferTooLarge);

    // SAFETY: malformed CBOR is rejected by the core.
    let status = unsafe { nexus_core_invoke(handle, 0x0101, b"\xff".as_ptr(), 1, &mut output) };
    assert_eq!(status, NexusStatus::InvalidArgument);

    assert_eq!(nexus_core_stop(handle), NexusStatus::Ok);
    assert_eq!(nexus_core_stop(handle), NexusStatus::Ok);
    assert_eq!(nexus_core_destroy(handle), NexusStatus::Ok);
    assert_eq!(nexus_core_destroy(handle), NexusStatus::InvalidHandle);
    assert_eq!(nexus_core_start(handle), NexusStatus::InvalidHandle);
    assert_eq!(nexus_core_stop(handle), NexusStatus::InvalidHandle);
}

#[test]
fn abi_mismatch_is_rejected_before_database_work() {
    let root = TempRoot::new("abi-mismatch");
    let target = root.path.join("untouched");
    let config = config_for(&target, 2);
    let secret = [4_u8; 32];
    let mut handle = u64::MAX;
    // SAFETY: all buffers are valid for the duration of the call.
    let status = unsafe {
        nexus_core_create(
            config.as_ptr(),
            config.len(),
            secret.as_ptr(),
            secret.len(),
            &mut handle,
        )
    };
    assert_eq!(status, NexusStatus::AbiMismatch);
    assert_eq!(handle, 0);
    assert!(!target.exists(), "no database or store may be created");

    let short_secret = [4_u8; 31];
    let valid = config_for(&target, 1);
    // SAFETY: all buffers are valid for the duration of the call.
    let status = unsafe {
        nexus_core_create(
            valid.as_ptr(),
            valid.len(),
            short_secret.as_ptr(),
            short_secret.len(),
            &mut handle,
        )
    };
    assert_eq!(status, NexusStatus::InvalidArgument);
    assert!(!target.exists());
}

#[test]
fn events_async_progress_and_cancellation_are_ordered() {
    let root = TempRoot::new("events");
    let (_, handle) = create(&root.path);
    assert_eq!(nexus_core_start(handle), NexusStatus::Ok);

    // Drain lifecycle events so the async operation is unambiguous.
    while nextevent_ok(handle, 5) {}

    let (status, bytes) = invoke(
        handle,
        0x0B02,
        Value::Map(vec![
            (Value::Text("fault".to_owned()), Value::Text("async".to_owned())),
            (Value::Text("delay_ms".to_owned()), Value::Unsigned(5)),
            (Value::Text("steps".to_owned()), Value::Unsigned(3)),
        ]),
    );
    assert_eq!(status, NexusStatus::Ok);
    let op_id = map_get(&success(&bytes), "op_id")
        .and_then(Value::as_unsigned)
        .expect("op_id");

    let mut progress = 0;
    let mut terminal = 0;
    let mut last_seq = 0;
    while terminal == 0 {
        let (status, bytes) = next_event(handle, 2_000);
        assert_eq!(status, NexusStatus::Ok, "expected a terminal event");
        let event = decode_event(&bytes);
        let seq = map_get(&event, "seq").and_then(Value::as_unsigned).expect("seq");
        assert!(seq > last_seq);
        last_seq = seq;
        let event_type = map_get(&event, "type").and_then(Value::as_text).expect("type");
        assert_eq!(
            map_get(&event, "op_id").and_then(Value::as_unsigned),
            Some(op_id),
            "events must correlate with the originating operation"
        );
        if event_type == "test.progress" {
            progress += 1;
        } else if event_type == "test.completed" {
            terminal += 1;
        }
    }
    assert!(progress > 0);
    assert_eq!(terminal, 1);

    let (status, bytes) = invoke(
        handle,
        0x0B02,
        Value::Map(vec![
            (Value::Text("fault".to_owned()), Value::Text("async".to_owned())),
            (Value::Text("delay_ms".to_owned()), Value::Unsigned(200)),
            (Value::Text("steps".to_owned()), Value::Unsigned(50)),
        ]),
    );
    assert_eq!(status, NexusStatus::Ok);
    let op_id = map_get(&success(&bytes), "op_id")
        .and_then(Value::as_unsigned)
        .expect("op_id");
    assert_eq!(nexus_core_cancel(handle, op_id), NexusStatus::Ok);
    assert_eq!(nexus_core_cancel(handle, op_id), NexusStatus::Ok);

    let mut cancelled = false;
    for _ in 0..64 {
        let (status, bytes) = next_event(handle, 2_000);
        assert_eq!(status, NexusStatus::Ok);
        let event = decode_event(&bytes);
        if map_get(&event, "type").and_then(Value::as_text) == Some("test.cancelled") {
            cancelled = true;
            break;
        }
    }
    assert!(cancelled, "cancellation must produce a terminal event");
    assert_eq!(nexus_core_cancel(handle, op_id), NexusStatus::Ok);

    assert_eq!(nexus_core_stop(handle), NexusStatus::Ok);
    assert_eq!(nexus_core_destroy(handle), NexusStatus::Ok);
}

fn nextevent_ok(handle: u64, timeout_ms: u32) -> bool {
    let (status, bytes) = next_event(handle, timeout_ms);
    if status == NexusStatus::Ok {
        assert!(!bytes.is_empty());
        true
    } else {
        assert_eq!(status, NexusStatus::Timeout);
        false
    }
}

#[test]
fn streaming_import_and_export_round_trip() {
    let root = TempRoot::new("streaming");
    let (_, handle) = create(&root.path);
    assert_eq!(nexus_core_start(handle), NexusStatus::Ok);

    let payload = b"hello world";
    let name = b"harness.bin";
    let mut import_id = 0_u64;
    // SAFETY: buffers are valid for the duration of the call.
    let status =
        unsafe { nexus_core_import_begin(handle, name.as_ptr(), name.len(), payload.len() as u64, &mut import_id) };
    assert_eq!(status, NexusStatus::Ok);
    assert_ne!(import_id, 0);

    // SAFETY: buffers are valid for the duration of the call.
    let status = unsafe { nexus_core_import_write(handle, import_id, payload[..5].as_ptr(), 5) };
    assert_eq!(status, NexusStatus::Ok);
    // SAFETY: buffers are valid for the duration of the call.
    let status = unsafe { nexus_core_import_write(handle, import_id, payload[5..].as_ptr(), payload.len() - 5) };
    assert_eq!(status, NexusStatus::Ok);

    let mut summary = NexusBuffer {
        ptr: std::ptr::null_mut(),
        len: 0,
    };
    // SAFETY: the output slot is valid.
    let status = unsafe { nexus_core_import_finish(handle, import_id, &mut summary) };
    assert_eq!(status, NexusStatus::Ok);
    let summary = take_buffer(summary);
    let summary = decode(&summary, CborLimits::new(16, 64, 64 * 1024)).expect("summary CBOR");
    let content_id = map_get(&summary, "content_id")
        .and_then(Value::as_text)
        .expect("content_id");
    assert_eq!(content_id, to_hex(&nexus_crypto::sha256(payload)));
    assert_eq!(
        map_get(&summary, "size").and_then(Value::as_unsigned),
        Some(payload.len() as u64)
    );
    assert_eq!(
        map_get(&summary, "chunk_size").and_then(Value::as_unsigned),
        Some(1024 * 1024)
    );
    assert_eq!(
        map_get(&summary, "display_name").and_then(Value::as_text),
        Some("harness.bin")
    );

    let content_bytes = hex_decode(content_id);
    assert_eq!(content_bytes.len(), 32);
    let mut export_id = 0_u64;
    // SAFETY: the content ID buffer and output slot are valid.
    let status = unsafe { nexus_core_export_open(handle, content_bytes.as_ptr(), content_bytes.len(), &mut export_id) };
    assert_eq!(status, NexusStatus::Ok);

    let mut exported = Vec::new();
    loop {
        let mut chunk = NexusBuffer {
            ptr: std::ptr::null_mut(),
            len: 0,
        };
        // SAFETY: the output slot is valid.
        let status = unsafe { nexus_core_export_read(handle, export_id, 5, &mut chunk) };
        assert_eq!(status, NexusStatus::Ok);
        let bytes = take_buffer(chunk);
        if bytes.is_empty() {
            break;
        }
        exported.extend_from_slice(&bytes);
    }
    assert_eq!(exported, payload);
    assert_eq!(nexus_core_export_close(handle, export_id), NexusStatus::Ok);
    assert_eq!(nexus_core_export_close(handle, export_id), NexusStatus::Ok);

    // Aborting discards the temporary file and invalidates the import ID.
    let mut aborted_id = 0_u64;
    // SAFETY: buffers are valid for the duration of the call.
    let status = unsafe { nexus_core_import_begin(handle, name.as_ptr(), name.len(), 4, &mut aborted_id) };
    assert_eq!(status, NexusStatus::Ok);
    // SAFETY: buffers are valid for the duration of the call.
    let status = unsafe { nexus_core_import_write(handle, aborted_id, b"data".as_ptr(), 4) };
    assert_eq!(status, NexusStatus::Ok);
    assert_eq!(nexus_core_import_abort(handle, aborted_id), NexusStatus::Ok);
    let mut ignored = NexusBuffer {
        ptr: std::ptr::null_mut(),
        len: 0,
    };
    // SAFETY: the output slot is valid.
    let status = unsafe { nexus_core_import_finish(handle, aborted_id, &mut ignored) };
    assert_eq!(status, NexusStatus::InvalidArgument);

    // Oversized writes and declared-size mismatches are rejected.
    let mut bounded_id = 0_u64;
    // SAFETY: buffers are valid for the duration of the call.
    let status = unsafe { nexus_core_import_begin(handle, name.as_ptr(), name.len(), 3, &mut bounded_id) };
    assert_eq!(status, NexusStatus::Ok);
    let oversized = vec![0_u8; MAX_STREAM_CHUNK + 1];
    // SAFETY: buffers are valid for the duration of the call.
    let status = unsafe { nexus_core_import_write(handle, bounded_id, oversized.as_ptr(), oversized.len()) };
    assert_eq!(status, NexusStatus::BufferTooLarge);
    // SAFETY: buffers are valid for the duration of the call.
    let status = unsafe { nexus_core_import_write(handle, bounded_id, b"four".as_ptr(), 4) };
    assert_eq!(status, NexusStatus::InvalidArgument);
    assert_eq!(nexus_core_import_abort(handle, bounded_id), NexusStatus::Ok);

    assert_eq!(nexus_core_stop(handle), NexusStatus::Ok);
    assert_eq!(nexus_core_destroy(handle), NexusStatus::Ok);
}

#[test]
fn one_hundred_lifecycle_cycles_do_not_leak_handles_or_locks() {
    let root = TempRoot::new("cycles");
    for cycle in 0..100 {
        let (status, handle) = create(&root.path);
        assert_eq!(status, NexusStatus::Ok, "create cycle {cycle}");
        assert_eq!(nexus_core_start(handle), NexusStatus::Ok, "start cycle {cycle}");
        let (status, bytes) = invoke(handle, 0x0102, empty_params());
        assert_eq!(status, NexusStatus::Ok, "health cycle {cycle}");
        assert!(matches!(success(&bytes), Value::Map(_)));
        let (status, _) = next_event(handle, 5);
        assert!(matches!(status, NexusStatus::Ok | NexusStatus::Timeout));
        assert_eq!(nexus_core_stop(handle), NexusStatus::Ok, "stop cycle {cycle}");
        assert_eq!(nexus_core_destroy(handle), NexusStatus::Ok, "destroy cycle {cycle}");
    }

    // The same data root is reused, proving every lock and handle was released.
    let (status, handle) = create(&root.path);
    assert_eq!(status, NexusStatus::Ok);
    assert_eq!(nexus_core_start(handle), NexusStatus::Ok);
    assert_eq!(nexus_core_stop(handle), NexusStatus::Ok);
    assert_eq!(nexus_core_destroy(handle), NexusStatus::Ok);
}
