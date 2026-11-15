/*
 * Ember C ABI harness.
 *
 * Exercises the same lifecycle, invoke, event, streaming, panic, error, and
 * ABI-mismatch contract as the Kotlin JNI and JNA adapters. Every function is
 * reached through the public header; the harness has no knowledge of Rust
 * internals.
 *
 * Build and run through scripts/build-native.ps1.
 */

#define _CRT_SECURE_NO_WARNINGS

#include <direct.h>
#include <errno.h>
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <windows.h>

#include "nexus_core.h"

#define STREAM_CHUNK ((size_t)1024 * 1024)
#define IMPORT_BYTES ((size_t)2500000)
#define LIFECYCLE_CYCLES 100

static int g_failures = 0;

#define CHECK(condition, ...)                                                                                          \
    do {                                                                                                               \
        if (!(condition)) {                                                                                            \
            fprintf(stderr, "FAIL %s:%d: ", __FILE__, __LINE__);                                                       \
            fprintf(stderr, __VA_ARGS__);                                                                              \
            fputc('\n', stderr);                                                                                       \
            g_failures += 1;                                                                                           \
        }                                                                                                              \
    } while (0)

/* ------------------------------------------------------------------------- */
/* Canonical CBOR writer                                                     */
/* ------------------------------------------------------------------------- */

typedef struct Writer {
    uint8_t *data;
    size_t len;
    size_t cap;
    int ok;
} Writer;

static void w_init(Writer *writer, uint8_t *data, size_t cap) {
    writer->data = data;
    writer->len = 0;
    writer->cap = cap;
    writer->ok = 1;
}

static void w_byte(Writer *writer, uint8_t value) {
    if (!writer->ok || writer->len >= writer->cap) {
        writer->ok = 0;
        return;
    }
    writer->data[writer->len++] = value;
}

static void w_raw(Writer *writer, const uint8_t *data, size_t len) {
    if (!writer->ok || len > writer->cap - writer->len) {
        writer->ok = 0;
        return;
    }
    memcpy(writer->data + writer->len, data, len);
    writer->len += len;
}

static void w_head(Writer *writer, uint8_t major, uint64_t value) {
    uint8_t prefix = (uint8_t)(major << 5);
    if (value < 24) {
        w_byte(writer, (uint8_t)(prefix | value));
    } else if (value <= 0xFF) {
        w_byte(writer, (uint8_t)(prefix | 24));
        w_byte(writer, (uint8_t)value);
    } else if (value <= 0xFFFF) {
        w_byte(writer, (uint8_t)(prefix | 25));
        w_byte(writer, (uint8_t)(value >> 8));
        w_byte(writer, (uint8_t)value);
    } else if (value <= 0xFFFFFFFFu) {
        w_byte(writer, (uint8_t)(prefix | 26));
        w_byte(writer, (uint8_t)(value >> 24));
        w_byte(writer, (uint8_t)(value >> 16));
        w_byte(writer, (uint8_t)(value >> 8));
        w_byte(writer, (uint8_t)value);
    } else {
        w_byte(writer, (uint8_t)(prefix | 27));
        for (int shift = 56; shift >= 0; shift -= 8) {
            w_byte(writer, (uint8_t)(value >> shift));
        }
    }
}

static void w_uint(Writer *writer, uint64_t value) { w_head(writer, 0, value); }

static void w_text(Writer *writer, const char *text) {
    size_t len = strlen(text);
    w_head(writer, 3, len);
    w_raw(writer, (const uint8_t *)text, len);
}

static void w_bool(Writer *writer, int value) { w_byte(writer, value ? 0xF5 : 0xF4); }

static void w_map(Writer *writer, size_t entries) { w_head(writer, 5, entries); }

/* ------------------------------------------------------------------------- */
/* Minimal canonical CBOR reader for generated envelopes and summaries        */
/* ------------------------------------------------------------------------- */

typedef struct Cursor {
    const uint8_t *position;
    const uint8_t *end;
} Cursor;

static int c_byte(Cursor *cursor, uint8_t *out) {
    if (cursor->position >= cursor->end) {
        return 0;
    }
    *out = *cursor->position++;
    return 1;
}

static int c_head(Cursor *cursor, uint8_t *major, uint64_t *value) {
    uint8_t initial = 0;
    if (!c_byte(cursor, &initial)) {
        return 0;
    }
    *major = (uint8_t)(initial >> 5);
    uint8_t additional = initial & 0x1F;
    if (additional < 24) {
        *value = additional;
        return 1;
    }
    size_t count = 0;
    switch (additional) {
    case 24:
        count = 1;
        break;
    case 25:
        count = 2;
        break;
    case 26:
        count = 4;
        break;
    case 27:
        count = 8;
        break;
    default:
        return 0;
    }
    uint64_t result = 0;
    for (size_t index = 0; index < count; index++) {
        uint8_t next = 0;
        if (!c_byte(cursor, &next)) {
            return 0;
        }
        result = (result << 8) | next;
    }
    *value = result;
    return 1;
}

static int c_skip(Cursor *cursor, int depth) {
    if (depth > 32) {
        return 0;
    }
    uint8_t major = 0;
    uint64_t value = 0;
    if (!c_head(cursor, &major, &value)) {
        return 0;
    }
    switch (major) {
    case 0:
    case 1:
    case 7:
        return 1;
    case 2:
    case 3:
        if (value > (uint64_t)(cursor->end - cursor->position)) {
            return 0;
        }
        cursor->position += value;
        return 1;
    case 4:
        if (value > 1000000) {
            return 0;
        }
        for (uint64_t index = 0; index < value; index++) {
            if (!c_skip(cursor, depth + 1)) {
                return 0;
            }
        }
        return 1;
    case 5:
        if (value > 1000000) {
            return 0;
        }
        for (uint64_t index = 0; index < value * 2; index++) {
            if (!c_skip(cursor, depth + 1)) {
                return 0;
            }
        }
        return 1;
    default:
        return 0;
    }
}

static int c_text(Cursor *cursor, char *out, size_t cap) {
    uint8_t major = 0;
    uint64_t value = 0;
    if (!c_head(cursor, &major, &value) || major != 3 || value >= cap) {
        return 0;
    }
    if (value > (uint64_t)(cursor->end - cursor->position)) {
        return 0;
    }
    memcpy(out, cursor->position, (size_t)value);
    out[value] = '\0';
    cursor->position += value;
    return 1;
}

static int c_uint(Cursor *cursor, uint64_t *out) {
    uint8_t major = 0;
    if (!c_head(cursor, &major, out) || major != 0) {
        return 0;
    }
    return 1;
}

static int c_bool(Cursor *cursor, int *out) {
    uint8_t major = 0;
    uint64_t value = 0;
    if (!c_head(cursor, &major, &value) || major != 7 || (value != 20 && value != 21)) {
        return 0;
    }
    *out = value == 21;
    return 1;
}

static int c_is_null(Cursor *cursor) {
    Cursor copy = *cursor;
    uint8_t major = 0;
    uint64_t value = 0;
    if (!c_head(&copy, &major, &value) || major != 7 || value != 22) {
        return 0;
    }
    *cursor = copy;
    return 1;
}

static int c_map_find(Cursor map, const char *key, Cursor *out) {
    Cursor cursor = map;
    uint8_t major = 0;
    uint64_t count = 0;
    if (!c_head(&cursor, &major, &count) || major != 5) {
        return 0;
    }
    for (uint64_t index = 0; index < count; index++) {
        Cursor value = cursor;
        char name[64];
        if (!c_text(&value, name, sizeof(name))) {
            return 0;
        }
        if (strcmp(name, key) == 0) {
            *out = value;
            return 1;
        }
        if (!c_skip(&value, 0)) {
            return 0;
        }
        cursor = value;
    }
    return 0;
}

/* ------------------------------------------------------------------------- */
/* Request builders                                                          */
/* ------------------------------------------------------------------------- */

static size_t build_config(uint8_t *buffer, size_t cap, const char *data_root, uint64_t abi_version) {
    Writer writer;
    w_init(&writer, buffer, cap);
    w_map(&writer, 8);
    w_text(&writer, "locale");
    w_text(&writer, "en-US");
    w_text(&writer, "data_root");
    w_text(&writer, data_root);
    w_text(&writer, "log_level");
    w_text(&writer, "info");
    w_text(&writer, "abi_version");
    w_uint(&writer, abi_version);
    w_text(&writer, "app_version");
    w_text(&writer, "stage-08");
    w_text(&writer, "quota_bytes");
    w_uint(&writer, 10ull * 1024ull * 1024ull * 1024ull);
    w_text(&writer, "network_enabled");
    w_bool(&writer, 0);
    w_text(&writer, "enable_test_methods");
    w_bool(&writer, 1);
    return writer.ok ? writer.len : 0;
}

static size_t build_echo_request(uint8_t *buffer, size_t cap, const char *payload) {
    Writer writer;
    w_init(&writer, buffer, cap);
    w_map(&writer, 2);
    w_text(&writer, "v");
    w_uint(&writer, 1);
    w_text(&writer, "params");
    w_map(&writer, 1);
    w_text(&writer, "payload");
    w_text(&writer, payload);
    return writer.ok ? writer.len : 0;
}

static size_t build_fault_request(
    uint8_t *buffer,
    size_t cap,
    const char *fault,
    int asynchronous,
    uint64_t steps,
    uint64_t delay_ms
) {
    Writer writer;
    w_init(&writer, buffer, cap);
    w_map(&writer, 2);
    w_text(&writer, "v");
    w_uint(&writer, 1);
    w_text(&writer, "params");
    w_map(&writer, asynchronous ? 3 : 1);
    w_text(&writer, "fault");
    w_text(&writer, fault);
    if (asynchronous) {
        w_text(&writer, "steps");
        w_uint(&writer, steps);
        w_text(&writer, "delay_ms");
        w_uint(&writer, delay_ms);
    }
    return writer.ok ? writer.len : 0;
}

/* ------------------------------------------------------------------------- */
/* Envelope helpers                                                          */
/* ------------------------------------------------------------------------- */

static int response_ok(const NexusBuffer *buffer, int *ok) {
    Cursor root = {buffer->ptr, buffer->ptr + buffer->len};
    Cursor value;
    return c_map_find(root, "ok", &value) && c_bool(&value, ok);
}

static int response_payload_text(const NexusBuffer *buffer, char *out, size_t cap) {
    Cursor root = {buffer->ptr, buffer->ptr + buffer->len};
    Cursor result;
    Cursor payload;
    if (!c_map_find(root, "result", &result) || !c_map_find(result, "payload", &payload)) {
        return 0;
    }
    return c_text(&payload, out, cap);
}

static int response_op_id(const NexusBuffer *buffer, uint64_t *op_id) {
    Cursor root = {buffer->ptr, buffer->ptr + buffer->len};
    Cursor result;
    Cursor value;
    if (!c_map_find(root, "result", &result) || !c_map_find(result, "op_id", &value)) {
        return 0;
    }
    return c_uint(&value, op_id);
}

static int response_domain_error(const NexusBuffer *buffer, char *code, size_t cap, int *retryable) {
    Cursor root = {buffer->ptr, buffer->ptr + buffer->len};
    Cursor error;
    Cursor value;
    if (!c_map_find(root, "error", &error) || !c_map_find(error, "code", &value) || !c_text(&value, code, cap)) {
        return 0;
    }
    if (!c_map_find(error, "retryable", &value) || !c_bool(&value, retryable)) {
        return 0;
    }
    return 1;
}

typedef struct Event {
    uint64_t seq;
    char type[48];
    int has_op;
    uint64_t op_id;
} Event;

static int parse_event(const NexusBuffer *buffer, Event *event) {
    Cursor root = {buffer->ptr, buffer->ptr + buffer->len};
    Cursor value;
    if (!c_map_find(root, "seq", &value) || !c_uint(&value, &event->seq)) {
        return 0;
    }
    if (!c_map_find(root, "type", &value) || !c_text(&value, event->type, sizeof(event->type))) {
        return 0;
    }
    if (!c_map_find(root, "op_id", &value)) {
        return 0;
    }
    if (c_is_null(&value)) {
        event->has_op = 0;
        event->op_id = 0;
        return 1;
    }
    event->has_op = 1;
    return c_uint(&value, &event->op_id);
}

static int hex_decode(const char *hex, uint8_t *out, size_t out_len) {
    if (strlen(hex) != out_len * 2) {
        return 0;
    }
    for (size_t index = 0; index < out_len; index++) {
        unsigned value = 0;
        if (sscanf(hex + index * 2, "%2x", &value) != 1) {
            return 0;
        }
        out[index] = (uint8_t)value;
    }
    return 1;
}

/* ------------------------------------------------------------------------- */
/* Filesystem helpers                                                        */
/* ------------------------------------------------------------------------- */

static int directory_exists(const char *path) {
    DWORD attributes = GetFileAttributesA(path);
    return attributes != INVALID_FILE_ATTRIBUTES && (attributes & FILE_ATTRIBUTE_DIRECTORY) != 0;
}

static void make_temp_root(char *out, size_t cap) {
    char temp[MAX_PATH];
    DWORD length = GetTempPathA(MAX_PATH, temp);
    if (length == 0 || length >= MAX_PATH) {
        fprintf(stderr, "FAIL: GetTempPathA failed\n");
        exit(1);
    }
    for (unsigned attempt = 0; attempt < 1000; attempt++) {
        int written = snprintf(
            out,
            cap,
            "%snexus-c-harness-%lu%s%u",
            temp,
            (unsigned long)GetCurrentProcessId(),
            attempt == 0 ? "" : "-",
            attempt
        );
        if (written <= 0 || (size_t)written >= cap) {
            fprintf(stderr, "FAIL: temp path is too long\n");
            exit(1);
        }
        if (_mkdir(out) == 0) {
            return;
        }
        if (errno != EEXIST) {
            fprintf(stderr, "FAIL: cannot create %s (errno %d)\n", out, errno);
            exit(1);
        }
    }
    fprintf(stderr, "FAIL: cannot create a unique temp directory\n");
    exit(1);
}

static void remove_tree(const char *path) {
    char pattern[1024];
    if (snprintf(pattern, sizeof(pattern), "%s\\*", path) >= (int)sizeof(pattern)) {
        return;
    }
    WIN32_FIND_DATAA entry;
    HANDLE find = FindFirstFileA(pattern, &entry);
    if (find != INVALID_HANDLE_VALUE) {
        do {
            if (strcmp(entry.cFileName, ".") == 0 || strcmp(entry.cFileName, "..") == 0) {
                continue;
            }
            char child[1024];
            if (snprintf(child, sizeof(child), "%s\\%s", path, entry.cFileName) >= (int)sizeof(child)) {
                continue;
            }
            if ((entry.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) != 0) {
                remove_tree(child);
            } else {
                DeleteFileA(child);
            }
        } while (FindNextFileA(find, &entry));
        FindClose(find);
    }
    RemoveDirectoryA(path);
}

/* ------------------------------------------------------------------------- */
/* Contract steps                                                            */
/* ------------------------------------------------------------------------- */

static void wait_for_terminal(
    uint64_t handle,
    uint64_t op_id,
    const char *expected,
    long min_progress,
    long max_progress
) {
    uint64_t last_seq = 0;
    long progress = 0;
    int terminal = 0;
    for (int attempt = 0; attempt < 600 && !terminal; attempt++) {
        NexusBuffer buffer = {0};
        NexusStatus status = nexus_core_next_event(handle, 5000, &buffer);
        if (status == NEXUS_STATUS_TIMEOUT) {
            CHECK(0, "timed out waiting for %s", expected);
            return;
        }
        if (status != NEXUS_STATUS_OK) {
            CHECK(0, "next_event failed with status %d", (int)status);
            return;
        }
        Event event;
        if (!parse_event(&buffer, &event)) {
            CHECK(0, "event envelope could not be parsed");
            nexus_core_free_buffer(buffer);
            return;
        }
        CHECK(event.seq > last_seq, "event %s sequence did not increase", event.type);
        if (event.seq > last_seq) {
            last_seq = event.seq;
        }
        if (strcmp(event.type, "test.progress") == 0) {
            CHECK(event.has_op && event.op_id == op_id, "progress event lost its operation ID");
            progress += 1;
        }
        if (strcmp(event.type, "test.completed") == 0 || strcmp(event.type, "test.failed") == 0 ||
            strcmp(event.type, "test.cancelled") == 0) {
            CHECK(event.has_op && event.op_id == op_id, "terminal event lost its operation ID");
            CHECK(strcmp(event.type, expected) == 0, "expected terminal %s, observed %s", expected, event.type);
            terminal = 1;
        }
        nexus_core_free_buffer(buffer);
    }
    CHECK(terminal, "terminal %s was not observed", expected);
    if (min_progress >= 0) {
        CHECK(
            progress >= min_progress && progress <= max_progress,
            "expected %ld..%ld coalesced progress events, observed %ld",
            min_progress,
            max_progress,
            progress
        );
    }
}

static void invoke_echo(uint64_t handle, const char *payload) {
    uint8_t request[512];
    size_t request_len = build_echo_request(request, sizeof(request), payload);
    CHECK(request_len > 0, "echo request builder failed");
    NexusBuffer response = {0};
    NexusStatus status = nexus_core_invoke(handle, 0x0B01, request, request_len, &response);
    CHECK(status == NEXUS_STATUS_OK, "echo invoke status %d", (int)status);
    int ok = 0;
    CHECK(response_ok(&response, &ok) && ok, "echo response was not successful");
    char echoed[128];
    CHECK(response_payload_text(&response, echoed, sizeof(echoed)), "echo payload was missing");
    CHECK(strcmp(echoed, payload) == 0, "echo mismatch: %s", echoed);
    nexus_core_free_buffer(response);
}

static void verify_domain_error(uint64_t handle) {
    uint8_t request[256];
    size_t request_len = build_fault_request(request, sizeof(request), "not_found", 0, 0, 0);
    NexusBuffer response = {0};
    NexusStatus status = nexus_core_invoke(handle, 0x0B02, request, request_len, &response);
    CHECK(status == NEXUS_STATUS_OK, "domain faults travel inside the envelope");
    int ok = 1;
    CHECK(response_ok(&response, &ok) && !ok, "domain response must not be ok");
    char code[64];
    int retryable = 1;
    CHECK(response_domain_error(&response, code, sizeof(code), &retryable), "domain error envelope was malformed");
    CHECK(strcmp(code, "not_found") == 0, "unexpected domain code %s", code);
    CHECK(!retryable, "not_found must not be retryable");
    nexus_core_free_buffer(response);
}

static void verify_panic_containment(uint64_t handle) {
    uint8_t request[256];
    size_t request_len = build_fault_request(request, sizeof(request), "panic", 0, 0, 0);
    NexusBuffer response = {0};
    NexusStatus status = nexus_core_invoke(handle, 0x0B02, request, request_len, &response);
    CHECK(status == NEXUS_STATUS_INTERNAL, "panic must become INTERNAL, observed %d", (int)status);
    CHECK(response.ptr == NULL && response.len == 0, "panic must not return a buffer");
    invoke_echo(handle, "after-panic");
}

static void verify_async_progress(uint64_t handle) {
    uint8_t request[256];
    size_t request_len = build_fault_request(request, sizeof(request), "async_progress", 1, 4, 5);
    NexusBuffer response = {0};
    NexusStatus status = nexus_core_invoke(handle, 0x0B02, request, request_len, &response);
    CHECK(status == NEXUS_STATUS_OK, "async invoke status %d", (int)status);
    uint64_t op_id = 0;
    CHECK(response_op_id(&response, &op_id), "async response did not include op_id");
    nexus_core_free_buffer(response);
    wait_for_terminal(handle, op_id, "test.completed", 1, 4);

    status = nexus_core_cancel(handle, op_id);
    CHECK(status == NEXUS_STATUS_OK, "cancel of a terminal operation is harmless");
}

static void verify_cancellation(uint64_t handle) {
    uint8_t request[256];
    size_t request_len = build_fault_request(request, sizeof(request), "async", 1, 100, 20);
    NexusBuffer response = {0};
    NexusStatus status = nexus_core_invoke(handle, 0x0B02, request, request_len, &response);
    CHECK(status == NEXUS_STATUS_OK, "cancel invoke status %d", (int)status);
    uint64_t op_id = 0;
    CHECK(response_op_id(&response, &op_id), "cancellation response did not include op_id");
    nexus_core_free_buffer(response);
    status = nexus_core_cancel(handle, op_id);
    CHECK(status == NEXUS_STATUS_OK, "cancel status %d", (int)status);
    wait_for_terminal(handle, op_id, "test.cancelled", 0, 100);
}

static void verify_streaming(uint64_t handle) {
    uint8_t *data = malloc(IMPORT_BYTES);
    CHECK(data != NULL, "could not allocate streaming data");
    if (data == NULL) {
        return;
    }
    for (size_t index = 0; index < IMPORT_BYTES; index++) {
        data[index] = (uint8_t)((index * 31 + 7) % 251);
    }

    uint64_t import_id = 0;
    const char *name = "harness.bin";
    NexusStatus status = nexus_core_import_begin(handle, (const uint8_t *)name, strlen(name), IMPORT_BYTES, &import_id);
    CHECK(status == NEXUS_STATUS_OK, "import_begin status %d", (int)status);
    for (size_t offset = 0; offset < IMPORT_BYTES && status == NEXUS_STATUS_OK; offset += STREAM_CHUNK) {
        size_t length = IMPORT_BYTES - offset < STREAM_CHUNK ? IMPORT_BYTES - offset : STREAM_CHUNK;
        status = nexus_core_import_write(handle, import_id, data + offset, length);
        CHECK(status == NEXUS_STATUS_OK, "import_write status %d at offset %zu", (int)status, offset);
    }

    NexusBuffer summary = {0};
    status = nexus_core_import_finish(handle, import_id, &summary);
    CHECK(status == NEXUS_STATUS_OK, "import_finish status %d", (int)status);
    Cursor summary_cursor = {summary.ptr, summary.ptr + summary.len};
    Cursor value;
    char hex[130];
    CHECK(
        c_map_find(summary_cursor, "content_id", &value) && c_text(&value, hex, sizeof(hex)),
        "import summary did not include content_id"
    );
    uint8_t content_id[32];
    CHECK(hex_decode(hex, content_id, sizeof(content_id)), "content ID was not 32-byte hex");
    CHECK(c_map_find(summary_cursor, "size", &value) && c_uint(&value, &import_id) && import_id == IMPORT_BYTES,
          "import summary size mismatch");
    char manifest[130];
    CHECK(
        c_map_find(summary_cursor, "manifest_root", &value) && c_text(&value, manifest, sizeof(manifest)) &&
            manifest[0] != '\0',
        "import summary did not include manifest_root"
    );
    nexus_core_free_buffer(summary);

    uint64_t export_id = 0;
    status = nexus_core_export_open(handle, content_id, sizeof(content_id), &export_id);
    CHECK(status == NEXUS_STATUS_OK, "export_open status %d", (int)status);
    size_t total = 0;
    int eof = 0;
    while (!eof && status == NEXUS_STATUS_OK) {
        NexusBuffer chunk = {0};
        status = nexus_core_export_read(handle, export_id, (uint32_t)STREAM_CHUNK, &chunk);
        CHECK(status == NEXUS_STATUS_OK, "export_read status %d", (int)status);
        if (status != NEXUS_STATUS_OK) {
            break;
        }
        if (chunk.len == 0) {
            eof = 1;
        } else {
            CHECK(chunk.len <= STREAM_CHUNK, "export chunk exceeded 1 MiB");
            CHECK(total + chunk.len <= IMPORT_BYTES, "export produced too many bytes");
            if (total + chunk.len <= IMPORT_BYTES) {
                CHECK(memcmp(chunk.ptr, data + total, chunk.len) == 0, "export bytes differ at offset %zu", total);
            }
            total += chunk.len;
        }
        nexus_core_free_buffer(chunk);
    }
    CHECK(eof, "export cursor did not reach EOF");
    CHECK(total == IMPORT_BYTES, "export produced %zu bytes", total);
    CHECK(nexus_core_export_close(handle, export_id) == NEXUS_STATUS_OK, "export_close failed");
    free(data);
}

static void lifecycle_cycles(const uint8_t *config, size_t config_len, const uint8_t *secret) {
    for (int cycle = 0; cycle < LIFECYCLE_CYCLES; cycle++) {
        uint64_t handle = 0;
        NexusStatus status = nexus_core_create(config, config_len, secret, 32, &handle);
        CHECK(status == NEXUS_STATUS_OK && handle != 0, "cycle %d create status %d", cycle, (int)status);
        if (status != NEXUS_STATUS_OK) {
            return;
        }
        status = nexus_core_start(handle);
        CHECK(status == NEXUS_STATUS_OK, "cycle %d start status %d", cycle, (int)status);
        status = nexus_core_stop(handle);
        CHECK(status == NEXUS_STATUS_OK, "cycle %d stop status %d", cycle, (int)status);
        status = nexus_core_destroy(handle);
        CHECK(status == NEXUS_STATUS_OK, "cycle %d destroy status %d", cycle, (int)status);
        if (cycle == 24 || cycle == 49 || cycle == 74 || cycle == 99) {
            printf("  lifecycle cycles: %d/%d\n", cycle + 1, LIFECYCLE_CYCLES);
        }
    }
}

int main(void) {
    printf("Ember C ABI harness\n");
    uint32_t abi = nexus_core_abi_version();
    printf("  ABI version: %" PRIu32 "\n", abi);
    CHECK(abi == NEXUS_ABI_VERSION, "ABI version mismatch");
    const char *version = nexus_core_version();
    CHECK(version != NULL && version[0] != '\0', "library version was empty");
    printf("  library version: %s\n", version == NULL ? "(null)" : version);

    uint8_t secret[32];
    for (size_t index = 0; index < sizeof(secret); index++) {
        secret[index] = (uint8_t)(0x40 + index);
    }

    char root[512];
    make_temp_root(root, sizeof(root));
    printf("  data root: %s\n", root);

    uint8_t bad_config[512];
    char mismatch_root[600];
    snprintf(mismatch_root, sizeof(mismatch_root), "%s\\abi-mismatch", root);
    size_t bad_len = build_config(bad_config, sizeof(bad_config), mismatch_root, NEXUS_ABI_VERSION + 1);
    uint64_t handle = 0;
    NexusStatus status = nexus_core_create(bad_config, bad_len, secret, sizeof(secret), &handle);
    CHECK(status == NEXUS_STATUS_ABI_MISMATCH, "ABI mismatch status %d", (int)status);
    CHECK(handle == 0, "ABI mismatch must not return a handle");
    CHECK(!directory_exists(mismatch_root), "ABI mismatch must be rejected before storage work");

    status = nexus_core_create(bad_config, bad_len, secret, 31, &handle);
    CHECK(status == NEXUS_STATUS_INVALID_ARGUMENT, "short secret status %d", (int)status);

    uint8_t config[512];
    size_t config_len = build_config(config, sizeof(config), root, NEXUS_ABI_VERSION);
    CHECK(config_len > 0, "config builder failed");

    printf("  lifecycle: %d create/start/stop/destroy cycles\n", LIFECYCLE_CYCLES);
    lifecycle_cycles(config, config_len, secret);

    status = nexus_core_create(config, config_len, secret, sizeof(secret), &handle);
    CHECK(status == NEXUS_STATUS_OK && handle != 0, "create status %d", (int)status);
    status = nexus_core_start(handle);
    CHECK(status == NEXUS_STATUS_OK, "start status %d", (int)status);
    CHECK(nexus_core_start(handle) == NEXUS_STATUS_OK, "start must be idempotent");
    CHECK(nexus_core_start(handle + 100000) == NEXUS_STATUS_INVALID_HANDLE, "unknown handles must be rejected");

    invoke_echo(handle, "stage-08-c-harness");
    verify_domain_error(handle);
    verify_panic_containment(handle);
    verify_async_progress(handle);
    verify_cancellation(handle);
    verify_streaming(handle);

    status = nexus_core_stop(handle);
    CHECK(status == NEXUS_STATUS_OK, "stop status %d", (int)status);
    CHECK(nexus_core_stop(handle) == NEXUS_STATUS_OK, "stop must be idempotent");
    status = nexus_core_start(handle);
    CHECK(status == NEXUS_STATUS_OK, "restart status %d", (int)status);
    invoke_echo(handle, "restarted");
    status = nexus_core_destroy(handle);
    CHECK(status == NEXUS_STATUS_OK, "destroy status %d", (int)status);
    CHECK(nexus_core_destroy(handle) == NEXUS_STATUS_INVALID_HANDLE, "destroy must release the handle");
    CHECK(nexus_core_destroy(0) == NEXUS_STATUS_INVALID_HANDLE, "zero is never a valid handle");

    remove_tree(root);

    if (g_failures != 0) {
        fprintf(stderr, "\nC harness failed with %d error(s).\n", g_failures);
        return 1;
    }
    printf("\nC harness passed.\n");
    return 0;
}
