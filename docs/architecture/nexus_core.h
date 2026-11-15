/*
 * Ember core C ABI — draft for Stage 02, implemented in Stage 08.
 *
 * ABI version: 1
 *
 * Rules:
 *   - All buffers are length-delimited byte buffers; strings are UTF-8.
 *   - Output buffers are allocated by the core and must be released exactly
 *     once with nexus_core_free_buffer().
 *   - Input buffers are borrowed for the duration of the call only.
 *   - Handles are opaque registry tokens, never raw pointers.
 *   - No function may be called on the UI thread.
 *   - Rust panics never unwind across this ABI.
 */

#ifndef NEXUS_CORE_H
#define NEXUS_CORE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#if defined(_WIN32) && defined(NEXUS_CORE_BUILD)
#define NEXUS_API __declspec(dllexport)
#elif defined(__GNUC__) && defined(NEXUS_CORE_BUILD)
#define NEXUS_API __attribute__((visibility("default")))
#else
#define NEXUS_API
#endif

#define NEXUS_ABI_VERSION 1u

typedef enum NexusStatus {
    NEXUS_STATUS_OK = 0,
    NEXUS_STATUS_TIMEOUT = 1,
    NEXUS_STATUS_INVALID_HANDLE = 2,
    NEXUS_STATUS_INVALID_ARGUMENT = 3,
    NEXUS_STATUS_ABI_MISMATCH = 4,
    NEXUS_STATUS_QUEUE_CLOSED = 5,
    NEXUS_STATUS_INTERNAL = 6,
    NEXUS_STATUS_BUFFER_TOO_LARGE = 7,
    NEXUS_STATUS_UNSUPPORTED = 8
} NexusStatus;

typedef struct NexusBuffer {
    uint8_t *ptr;
    size_t len;
} NexusBuffer;

/* Returns NEXUS_ABI_VERSION. */
NEXUS_API uint32_t nexus_core_abi_version(void);

/* Returns a static semantic-version string valid for the process lifetime. */
NEXUS_API const char *nexus_core_version(void);

/*
 * Creates a core instance.
 *
 * config_cbor is a CBOR map with app-private paths, locale, network, storage,
 * and logging settings. It must not contain secret material.
 *
 * local_secret is exactly 32 bytes and is zeroized by the core after use.
 * The host must zero its own copy immediately after this call.
 */
NEXUS_API NexusStatus nexus_core_create(
    const uint8_t *config_cbor,
    size_t config_len,
    const uint8_t *local_secret,
    size_t local_secret_len,
    uint64_t *out_handle);

/* Starts the core. Idempotent. */
NEXUS_API NexusStatus nexus_core_start(uint64_t handle);

/* Stops the core, pausing transfers and flushing durable state. Idempotent. */
NEXUS_API NexusStatus nexus_core_stop(uint64_t handle);

/* Stops if needed and releases the handle. */
NEXUS_API NexusStatus nexus_core_destroy(uint64_t handle);

/*
 * Invokes a stable method ID with a CBOR request envelope.
 * The response buffer must be freed with nexus_core_free_buffer().
 */
NEXUS_API NexusStatus nexus_core_invoke(
    uint64_t handle,
    uint32_t method_id,
    const uint8_t *request_cbor,
    size_t request_len,
    NexusBuffer *out_response);

/*
 * Waits up to timeout_ms for the next CBOR event envelope.
 * Returns NEXUS_STATUS_TIMEOUT with an empty buffer when no event arrives.
 * Only one consumer may call this function for a given handle.
 */
NEXUS_API NexusStatus nexus_core_next_event(
    uint64_t handle,
    uint32_t timeout_ms,
    NexusBuffer *out_event);

/* Requests cooperative cancellation of an asynchronous operation. */
NEXUS_API NexusStatus nexus_core_cancel(uint64_t handle, uint64_t op_id);

/* Frees a buffer returned by the core. A null pointer with zero length is safe. */
NEXUS_API void nexus_core_free_buffer(NexusBuffer buffer);

/*
 * Streaming import. One import ID is written sequentially by one host task.
 * The display name is metadata only and never becomes a filesystem path.
 * expected_size is required and limited to 4 GiB.
 */
NEXUS_API NexusStatus nexus_core_import_begin(
    uint64_t handle,
    const uint8_t *display_name_utf8,
    size_t display_name_len,
    uint64_t expected_size,
    uint64_t *out_import_id);

/* Writes at most 1 MiB per call. */
NEXUS_API NexusStatus nexus_core_import_write(
    uint64_t handle,
    uint64_t import_id,
    const uint8_t *data,
    size_t len);

/*
 * Finalizes the import and returns a CBOR summary:
 * { content_id, size, manifest_root, chunk_size, display_name }.
 */
NEXUS_API NexusStatus nexus_core_import_finish(
    uint64_t handle,
    uint64_t import_id,
    NexusBuffer *out_summary);

/* Aborts an import and removes its temporary file. */
NEXUS_API NexusStatus nexus_core_import_abort(
    uint64_t handle,
    uint64_t import_id);

/*
 * Opens a read cursor for a locally verified content ID.
 * content_id is exactly 32 bytes.
 */
NEXUS_API NexusStatus nexus_core_export_open(
    uint64_t handle,
    const uint8_t *content_id,
    size_t content_id_len,
    uint64_t *out_export_id);

/*
 * Reads at most max_len bytes (max 1 MiB). A zero-length output means EOF.
 * The host writes the bytes to a user-selected destination and frees the
 * buffer with nexus_core_free_buffer().
 */
NEXUS_API NexusStatus nexus_core_export_read(
    uint64_t handle,
    uint64_t export_id,
    uint32_t max_len,
    NexusBuffer *out_chunk);

/* Closes an export cursor. */
NEXUS_API NexusStatus nexus_core_export_close(
    uint64_t handle,
    uint64_t export_id);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* NEXUS_CORE_H */
