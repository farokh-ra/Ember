//! Stage 17 integration: one download takes chunks from two independent real
//! engines, verifies every chunk against the signed manifest, and survives a
//! source that cannot serve the file.
//!
//! All engines listen on loopback only. Both sources import identical bytes, so
//! they share a content ID and manifest root, but each publishes under its own
//! identity. The receiver connects to both and downloads by content ID with
//! both peers listed as sources.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::sleep;
use std::time::{Duration, Instant};

use nexus_core::{CoreConfig, Engine, LogLevel, MAX_EVENT_BYTES, MAX_STREAM_CHUNK, Response, decode_response};
use nexus_crypto::{CborLimits, Value, decode};
use nexus_identity::{IdentityVault, LocalEncryptionKey};

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(label: &str) -> Self {
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("nexus-core-stage17-{label}-{}-{sequence}", std::process::id()));
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

fn config(root: &Path) -> CoreConfig {
    CoreConfig {
        data_root: root.to_path_buf(),
        app_version: "0.1.0-test".to_owned(),
        locale: "en".to_owned(),
        log_level: LogLevel::Info,
        network_enabled: true,
        dht_mode: nexus_core::DhtMode::Client,
        relay_fallback: false,
        relay_server: false,
        search_provider: false,
        quota_bytes: 1024 * 1024 * 1024,
        enable_test_methods: false,
    }
}

fn start(root: &Path, key: u8) -> Engine {
    let local_key = LocalEncryptionKey::new([key; 32]);
    IdentityVault::new(root.to_path_buf())
        .expect("identity vault")
        .create(&local_key)
        .expect("create identity");
    let engine = Engine::create(config(root), &[key; 32]).expect("create engine");
    engine.start().expect("start engine");
    engine
}

fn params(entries: Vec<(&str, Value)>) -> Value {
    Value::Map(
        entries
            .into_iter()
            .map(|(key, value)| (Value::Text(key.to_owned()), value))
            .collect(),
    )
}

fn empty_params() -> Value {
    Value::Map(Vec::new())
}

fn invoke(engine: &Engine, method_id: u32, parameters: Value) -> Response {
    let request = Value::Map(vec![
        (Value::Text("v".to_owned()), Value::Unsigned(1)),
        (Value::Text("params".to_owned()), parameters),
    ])
    .to_canonical_bytes()
    .expect("request CBOR");
    let response = engine.invoke(method_id, &request).expect("method invoke");
    decode_response(&response).expect("response envelope")
}

fn success(response: Response) -> Value {
    match response {
        Response::Success(value) => value,
        Response::Failure(error) => panic!("unexpected failure: {error:?}"),
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

fn text_field(value: &Value, key: &str) -> String {
    map_get(value, key)
        .and_then(Value::as_text)
        .unwrap_or_else(|| panic!("missing text field {key}"))
        .to_owned()
}

fn unsigned_field(value: &Value, key: &str) -> u64 {
    map_get(value, key)
        .and_then(Value::as_unsigned)
        .unwrap_or_else(|| panic!("missing unsigned field {key}"))
}

struct RecordedEvent {
    event_type: String,
    op_id: Option<u64>,
    data: Value,
}

fn decode_event(bytes: &[u8]) -> RecordedEvent {
    let value = decode(bytes, CborLimits::new(16, 512, MAX_EVENT_BYTES)).expect("event CBOR");
    RecordedEvent {
        event_type: text_field(&value, "type"),
        op_id: map_get(&value, "op_id").and_then(Value::as_unsigned),
        data: map_get(&value, "data").expect("event data").clone(),
    }
}

fn wait_for_event(engine: &Engine, event_type: &str, within: Duration) -> RecordedEvent {
    let deadline = Instant::now() + within;
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let timeout = u32::try_from(remaining.as_millis().min(200)).unwrap_or(200);
        if let Some(bytes) = engine.next_event(timeout).expect("read event") {
            let event = decode_event(&bytes);
            if event.event_type == event_type {
                return event;
            }
        }
    }
    panic!("timed out waiting for {event_type}");
}

fn status(engine: &Engine) -> Value {
    success(invoke(engine, 0x0601, empty_params()))
}

fn wait_for_ready(engine: &Engine) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let current = status(engine);
        if matches!(text_field(&current, "state").as_str(), "degraded" | "online") {
            return;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for a ready network: {current:?}");
        }
        sleep(Duration::from_millis(50));
    }
}

fn dial_address(engine: &Engine) -> String {
    let current = status(engine);
    let address = match map_get(&current, "listen_addresses") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_text)
            .find(|address| address.contains("/tcp/"))
            .map(str::to_owned),
        _ => None,
    }
    .expect("a loopback TCP listen address");
    let peer_id = text_field(&current, "peer_id");
    if address.ends_with(&format!("/p2p/{peer_id}")) {
        address
    } else {
        format!("{address}/p2p/{peer_id}")
    }
}

fn connect(engine: &Engine, multiaddr: &str, peer_id: &str) {
    let response = success(invoke(
        engine,
        0x0607,
        params(vec![("multiaddr", Value::Text(multiaddr.to_owned()))]),
    ));
    let op_id = unsigned_field(&response, "op_id");
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let timeout = u32::try_from(remaining.as_millis().min(200)).unwrap_or(200);
        if let Some(bytes) = engine.next_event(timeout).expect("read event") {
            let event = decode_event(&bytes);
            if event.event_type == "network.connect_completed" && event.op_id == Some(op_id) {
                assert_eq!(map_get(&event.data, "peer_id").and_then(Value::as_text), Some(peer_id));
                return;
            }
        }
    }
    panic!("connect to {peer_id} timed out");
}

fn import_file(engine: &Engine, name: &str, content: &[u8]) -> (String, String) {
    let import_id = engine
        .import_begin(name, u64::try_from(content.len()).expect("size"))
        .expect("begin import");
    for chunk in content.chunks(MAX_STREAM_CHUNK) {
        engine.import_write(import_id, chunk).expect("write import");
    }
    let summary = engine.import_finish(import_id).expect("finish import");
    let value = decode(&summary, CborLimits::new(4, 16, 4_096)).expect("import summary");
    (text_field(&value, "content_id"), text_field(&value, "manifest_root"))
}

fn publish_file(engine: &Engine, content_id: &str, display_filename: &str) {
    success(invoke(
        engine,
        0x0501,
        params(vec![
            ("content_id", Value::Text(content_id.to_owned())),
            ("display_filename", Value::Text(display_filename.to_owned())),
            ("description", Value::Text("Stage 17 integration file".to_owned())),
            ("category", Value::Text("data".to_owned())),
            ("tags", Value::Array(vec![Value::Text("stage17".to_owned())])),
        ]),
    ));
    wait_for_event(engine, "publication.published", Duration::from_secs(30));
}

fn decode_hex_32(hex: &str) -> [u8; 32] {
    assert_eq!(hex.len(), 64, "content ID must be 32-byte hex");
    let mut bytes = [0_u8; 32];
    for (index, slot) in bytes.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).expect("hex byte");
    }
    bytes
}

fn export(engine: &Engine, content_id: &str) -> Vec<u8> {
    let export_id = engine.export_open(&decode_hex_32(content_id)).expect("open export");
    let mut bytes = Vec::new();
    loop {
        let chunk = engine.export_read(export_id, MAX_STREAM_CHUNK).expect("read export");
        if chunk.is_empty() {
            break;
        }
        bytes.extend_from_slice(&chunk);
    }
    engine.export_close(export_id);
    bytes
}

fn content() -> Vec<u8> {
    (0..(3 * 1024 * 1024 + 321_123))
        .map(|index| u8::try_from((index * 37 + 11) % 251).unwrap_or(0))
        .collect()
}

fn source_successes(progress: &[Value]) -> Vec<(String, u64)> {
    let Some(last) = progress.last() else {
        return Vec::new();
    };
    let Some(Value::Array(items)) = map_get(last, "sources") else {
        return Vec::new();
    };
    items
        .iter()
        .map(|item| {
            (
                text_field(item, "peer_id"),
                map_get(item, "successes").and_then(Value::as_unsigned).unwrap_or(0),
            )
        })
        .collect()
}

fn wait_for_download(engine: &Engine, download_id: &str, within: Duration) -> Vec<Value> {
    let deadline = Instant::now() + within;
    let mut progress = Vec::new();
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let timeout = u32::try_from(remaining.as_millis().min(200)).unwrap_or(200);
        if let Some(bytes) = engine.next_event(timeout).expect("read event") {
            let event = decode_event(&bytes);
            match event.event_type.as_str() {
                "download.progress" => progress.push(event.data),
                "download.completed"
                    if map_get(&event.data, "download_id").and_then(Value::as_text) == Some(download_id) =>
                {
                    return progress;
                }
                "download.failed"
                    if map_get(&event.data, "download_id").and_then(Value::as_text) == Some(download_id) =>
                {
                    panic!("download failed: {:?}", event.data);
                }
                _ => {}
            }
        }
    }
    panic!("timed out waiting for download {download_id}");
}

#[test]
fn one_download_combines_verified_chunks_from_two_peers() {
    let source_a_root = TempRoot::new("source-a");
    let source_b_root = TempRoot::new("source-b");
    let destination_root = TempRoot::new("destination");
    let bytes = content();

    let source_a = start(&source_a_root.path, 41);
    let source_b = start(&source_b_root.path, 42);
    let destination = start(&destination_root.path, 43);
    wait_for_ready(&source_a);
    wait_for_ready(&source_b);
    wait_for_ready(&destination);

    let (content_id, manifest_root) = import_file(&source_a, "stage17-a.bin", &bytes);
    let (content_b, manifest_b) = import_file(&source_b, "stage17-b.bin", &bytes);
    assert_eq!(content_b, content_id, "identical bytes share a content ID");
    assert_eq!(manifest_b, manifest_root, "identical bytes share a manifest root");
    publish_file(&source_a, &content_id, "stage17-a.bin");
    publish_file(&source_b, &content_id, "stage17-b.bin");

    let peer_a = text_field(&status(&source_a), "peer_id");
    let peer_b = text_field(&status(&source_b), "peer_id");
    connect(&destination, &dial_address(&source_a), &peer_a);
    connect(&destination, &dial_address(&source_b), &peer_b);

    let started = success(invoke(
        &destination,
        0x0801,
        params(vec![
            ("content_id", Value::Text(content_id.clone())),
            (
                "sources",
                Value::Array(vec![Value::Text(peer_a.clone()), Value::Text(peer_b.clone())]),
            ),
            ("display_name", Value::Text("stage17-copy.bin".to_owned())),
        ]),
    ));
    let download_id = text_field(&started, "download_id");
    assert_eq!(map_get(&started, "accepted").and_then(Value::as_bool), Some(true));

    let progress = wait_for_download(&destination, &download_id, Duration::from_secs(120));
    assert_eq!(export(&destination, &content_id), bytes);

    let successes = source_successes(&progress);
    assert_eq!(successes.len(), 2, "both peers should be reported: {successes:?}");
    assert!(
        successes.iter().all(|(_, count)| *count > 0),
        "both peers should have delivered authenticated chunks: {successes:?}"
    );
    let bulk: u64 = successes.iter().map(|(_, count)| *count).sum();
    let last = progress.last().expect("a progress event");
    assert_eq!(unsigned_field(last, "verified_chunks"), bulk);
    assert_eq!(unsigned_field(last, "total_chunks"), bulk);

    destination.stop().expect("stop destination");
    source_b.stop().expect("stop source b");
    source_a.stop().expect("stop source a");
}

#[test]
fn a_peer_that_cannot_serve_the_file_does_not_block_the_other() {
    let source_a_root = TempRoot::new("serving");
    let source_b_root = TempRoot::new("empty");
    let destination_root = TempRoot::new("receiver");
    let bytes = content();

    let source_a = start(&source_a_root.path, 51);
    let source_b = start(&source_b_root.path, 52);
    let destination = start(&destination_root.path, 53);
    wait_for_ready(&source_a);
    wait_for_ready(&source_b);
    wait_for_ready(&destination);

    let (content_id, _) = import_file(&source_a, "stage17-serving.bin", &bytes);
    publish_file(&source_a, &content_id, "stage17-serving.bin");

    let peer_a = text_field(&status(&source_a), "peer_id");
    let peer_b = text_field(&status(&source_b), "peer_id");
    connect(&destination, &dial_address(&source_a), &peer_a);
    connect(&destination, &dial_address(&source_b), &peer_b);

    let started = success(invoke(
        &destination,
        0x0801,
        params(vec![
            ("content_id", Value::Text(content_id.clone())),
            (
                "sources",
                Value::Array(vec![Value::Text(peer_b.clone()), Value::Text(peer_a.clone())]),
            ),
            ("display_name", Value::Text("stage17-survivor.bin".to_owned())),
        ]),
    ));
    let download_id = text_field(&started, "download_id");

    let progress = wait_for_download(&destination, &download_id, Duration::from_secs(120));
    assert_eq!(export(&destination, &content_id), bytes);
    let successes = source_successes(&progress);
    let serving = successes
        .iter()
        .find(|(peer, _)| peer == &peer_a)
        .map_or(0, |(_, count)| *count);
    assert!(serving > 0, "the serving peer should have delivered chunks");
    let last = progress.last().expect("a progress event");
    assert_eq!(
        unsigned_field(last, "verified_chunks"),
        unsigned_field(last, "total_chunks")
    );

    destination.stop().expect("stop destination");
    source_b.stop().expect("stop empty source");
    source_a.stop().expect("stop serving source");
}
