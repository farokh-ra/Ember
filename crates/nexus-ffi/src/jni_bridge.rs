//! Android JNI shim.
//!
//! The shim is deliberately thin: every entry point copies a `jbyteArray`
//! into a Rust slice, calls the same safe helpers used by the C ABI, and
//! copies the result back into a new Java array. No pointer is exposed to
//! Kotlin and no Rust panic crosses the JNI boundary.
//!
//! Result encoding for functions that need a status and payload:
//! - `LongArray` of exactly two entries: `[status, value]`.
//! - `ByteArray` whose first four bytes are the little-endian `NexusStatus`
//!   followed by the payload bytes (may be empty).
//! - Plain `Int` for status-only functions.

#![allow(unsafe_code)]

use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

use jni::JNIEnv;
use jni::objects::{JByteArray, JClass};
use jni::sys::{JNI_ERR, JNI_VERSION_1_6, jbyteArray, jint, jlong, jlongArray, jstring};

use crate::{
    NexusStatus, cancel_call, create_handle, destroy_call, export_close_call, export_open_call, export_read_call,
    import_abort_call, import_begin_call, import_finish_call, import_write_call, invoke_call, next_event_call,
    start_call, stop_call,
};

fn status_payload(status: NexusStatus, payload: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(4 + payload.len());
    output.extend_from_slice(&(status as i32).to_le_bytes());
    output.extend_from_slice(payload);
    output
}

fn status_int(status: NexusStatus) -> jint {
    status as i32
}

fn read_array(env: &mut JNIEnv<'_>, array: &JByteArray<'_>) -> Option<Vec<u8>> {
    if array.as_raw().is_null() {
        return None;
    }
    env.convert_byte_array(array).ok()
}

fn byte_array(env: &mut JNIEnv<'_>, bytes: &[u8]) -> jbyteArray {
    env.byte_array_from_slice(bytes)
        .map_or(ptr::null_mut(), jni::objects::JByteArray::into_raw)
}

fn status_handle(env: &mut JNIEnv<'_>, status: NexusStatus, value: u64) -> jlongArray {
    let Ok(array) = env.new_long_array(2) else {
        return ptr::null_mut();
    };
    let values = [i64::from(status as i32), value as i64];
    if env.set_long_array_region(&array, 0, &values).is_err() {
        return ptr::null_mut();
    }
    array.into_raw()
}

fn string_or_null(env: &mut JNIEnv<'_>, text: &str) -> jstring {
    env.new_string(text)
        .map_or(ptr::null_mut(), jni::objects::JString::into_raw)
}

fn guarded_int(action: impl FnOnce() -> jint) -> jint {
    // Panic payloads are deliberately not surfaced across JNI.
    catch_unwind(AssertUnwindSafe(action)).unwrap_or(NexusStatus::Internal as jint)
}

fn guarded_bytes(env: &mut JNIEnv<'_>, action: impl FnOnce(&mut JNIEnv<'_>) -> jbyteArray) -> jbyteArray {
    match catch_unwind(AssertUnwindSafe(|| action(env))) {
        Ok(result) => result,
        Err(_) => byte_array(env, &status_payload(NexusStatus::Internal, &[])),
    }
}

fn guarded_ptr<T>(fallback: T, action: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(action)).unwrap_or(fallback)
}

/// JNI load hook. A version mismatch is rejected by returning `JNI_ERR`.
///
/// # Safety
///
/// Called by the JVM with a valid `JavaVM*` and reserved pointer.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn JNI_OnLoad(_vm: *mut c_void, _reserved: *mut c_void) -> jint {
    if nexus_core::ABI_VERSION == crate::nexus_core_abi_version() {
        JNI_VERSION_1_6
    } else {
        JNI_ERR
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_nexus_core_client_NexusNative_nativeAbiVersion(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jint {
    nexus_core::ABI_VERSION as jint
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_nexus_core_client_NexusNative_nativeVersion(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jstring {
    guarded_ptr(ptr::null_mut(), || string_or_null(&mut env, env!("CARGO_PKG_VERSION")))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_nexus_core_client_NexusNative_nativeCreate(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    config: JByteArray<'_>,
    local_secret: JByteArray<'_>,
) -> jlongArray {
    guarded_ptr(ptr::null_mut(), || {
        let (Some(config), Some(secret)) = (read_array(&mut env, &config), read_array(&mut env, &local_secret)) else {
            return status_handle(&mut env, NexusStatus::InvalidArgument, 0);
        };
        let (status, handle) = create_handle(&config, &secret);
        status_handle(&mut env, status, handle)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_nexus_core_client_NexusNative_nativeStart(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    handle: jlong,
) -> jint {
    guarded_int(|| status_int(start_call(handle as u64)))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_nexus_core_client_NexusNative_nativeStop(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    handle: jlong,
) -> jint {
    guarded_int(|| status_int(stop_call(handle as u64)))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_nexus_core_client_NexusNative_nativeDestroy(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    handle: jlong,
) -> jint {
    guarded_int(|| status_int(destroy_call(handle as u64)))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_nexus_core_client_NexusNative_nativeInvoke(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    handle: jlong,
    method_id: jint,
    request: JByteArray<'_>,
) -> jbyteArray {
    guarded_bytes(&mut env, |env| {
        let Some(request) = read_array(env, &request) else {
            return byte_array(env, &status_payload(NexusStatus::InvalidArgument, &[]));
        };
        let (status, payload) = invoke_call(handle as u64, method_id as u32, &request);
        byte_array(env, &status_payload(status, &payload))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_nexus_core_client_NexusNative_nativeNextEvent(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    handle: jlong,
    timeout_ms: jint,
) -> jbyteArray {
    guarded_bytes(&mut env, |env| {
        let timeout = u32::try_from(timeout_ms).unwrap_or(0);
        let (status, payload) = next_event_call(handle as u64, timeout);
        byte_array(env, &status_payload(status, &payload))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_nexus_core_client_NexusNative_nativeCancel(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    handle: jlong,
    op_id: jlong,
) -> jint {
    guarded_int(|| status_int(cancel_call(handle as u64, op_id as u64)))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_nexus_core_client_NexusNative_nativeImportBegin(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    handle: jlong,
    display_name: JByteArray<'_>,
    expected_size: jlong,
) -> jlongArray {
    guarded_ptr(ptr::null_mut(), || {
        let Some(name) = read_array(&mut env, &display_name) else {
            return status_handle(&mut env, NexusStatus::InvalidArgument, 0);
        };
        let Ok(name) = std::str::from_utf8(&name) else {
            return status_handle(&mut env, NexusStatus::InvalidArgument, 0);
        };
        let (status, import_id) = import_begin_call(handle as u64, name, expected_size as u64);
        status_handle(&mut env, status, import_id)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_nexus_core_client_NexusNative_nativeImportWrite(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    handle: jlong,
    import_id: jlong,
    data: JByteArray<'_>,
) -> jint {
    guarded_int(|| {
        let Some(data) = read_array(&mut env, &data) else {
            return status_int(NexusStatus::InvalidArgument);
        };
        status_int(import_write_call(handle as u64, import_id as u64, &data))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_nexus_core_client_NexusNative_nativeImportFinish(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    handle: jlong,
    import_id: jlong,
) -> jbyteArray {
    guarded_bytes(&mut env, |env| {
        let (status, payload) = import_finish_call(handle as u64, import_id as u64);
        byte_array(env, &status_payload(status, &payload))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_nexus_core_client_NexusNative_nativeImportAbort(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    handle: jlong,
    import_id: jlong,
) -> jint {
    guarded_int(|| status_int(import_abort_call(handle as u64, import_id as u64)))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_nexus_core_client_NexusNative_nativeExportOpen(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    handle: jlong,
    content_id: JByteArray<'_>,
) -> jlongArray {
    guarded_ptr(ptr::null_mut(), || {
        let Some(content_id) = read_array(&mut env, &content_id) else {
            return status_handle(&mut env, NexusStatus::InvalidArgument, 0);
        };
        let (status, export_id) = export_open_call(handle as u64, &content_id);
        status_handle(&mut env, status, export_id)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_nexus_core_client_NexusNative_nativeExportRead(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    handle: jlong,
    export_id: jlong,
    max_len: jint,
) -> jbyteArray {
    guarded_bytes(&mut env, |env| {
        let max_len = u32::try_from(max_len).unwrap_or(0);
        let (status, payload) = export_read_call(handle as u64, export_id as u64, max_len);
        byte_array(env, &status_payload(status, &payload))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_nexus_core_client_NexusNative_nativeExportClose(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    handle: jlong,
    export_id: jlong,
) -> jint {
    guarded_int(|| status_int(export_close_call(handle as u64, export_id as u64)))
}
