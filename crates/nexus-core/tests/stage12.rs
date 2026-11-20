//! Stage 12 integration: two real engines exchange a file through
//! `/nexus/file/1`, the download manager verifies it, and the receiver keeps
//! the verified content in its managed library.
//!
//! Both engines listen on loopback only. The source imports its file through
//! the public streaming import API; the receiver connects with a multiaddr from
//! the source status method and downloads by content ID.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::sleep;
use std::time::{Duration, Instant};

use nexus_core::{
    CoreConfig, DomainCode, Engine, LogLevel, MAX_EVENT_BYTES, MAX_STREAM_CHUNK, Response, decode_response,
};
use nexus_crypto::to_hex;
use nexus_crypto::{CborLimits, Value, decode};

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(label: &str) -> Self {
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("nexus-core-stage12-{label}-{}-{sequence}", std::process::id()));
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
        quota_bytes: 1024 * 1024 * 1024,
        enable_test_methods: false,
    }
}

fn start(root: &Path, key: u8) -> Engine {
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

fn connect(engine: &Engine, multiaddr: &str) -> u64 {
    let response = success(invoke(
        engine,
        0x0607,
        params(vec![("multiaddr", Value::Text(multiaddr.to_owned()))]),
    ));
    unsigned_field(&response, "op_id")
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

#[test]
fn engines_exchange_a_verified_file_and_keep_it_in_the_library() {
    let source_root = TempRoot::new("source");
    let destination_root = TempRoot::new("destination");
    let content: Vec<u8> = (0..(2 * 1024 * 1024 + 321_123))
        .map(|index| u8::try_from((index * 29 + 7) % 251).unwrap_or(0))
        .collect();

    let source = start(&source_root.path, 31);
    let destination = start(&destination_root.path, 32);
    wait_for_ready(&source);
    wait_for_ready(&destination);

    let (content_id, manifest_root) = import_file(&source, "stage12-source.bin", &content);
    assert_eq!(content_id.len(), 64);
    assert_eq!(manifest_root.len(), 64);

    let source_peer = text_field(&status(&source), "peer_id");
    let connect_op = connect(&destination, &dial_address(&source));
    let connected = wait_for_event(&destination, "network.connect_completed", Duration::from_secs(20));
    assert_eq!(connected.op_id, Some(connect_op));
    assert_eq!(
        map_get(&connected.data, "peer_id").and_then(Value::as_text),
        Some(source_peer.as_str())
    );

    let started = success(invoke(
        &destination,
        0x0801,
        params(vec![
            ("content_id", Value::Text(content_id.clone())),
            ("peer_id", Value::Text(source_peer.clone())),
            ("display_name", Value::Text("stage12-copy.bin".to_owned())),
        ]),
    ));
    let download_id = text_field(&started, "download_id");
    assert!(map_get(&started, "accepted").and_then(Value::as_bool) == Some(true));

    let completed = wait_for_event(&destination, "download.completed", Duration::from_secs(60));
    assert_eq!(
        map_get(&completed.data, "download_id").and_then(Value::as_text),
        Some(download_id.as_str())
    );
    assert_eq!(
        map_get(&completed.data, "content_id").and_then(Value::as_text),
        Some(content_id.as_str())
    );
    assert_eq!(
        map_get(&completed.data, "size").and_then(Value::as_unsigned),
        Some(u64::try_from(content.len()).expect("size"))
    );

    assert_eq!(export(&destination, &content_id), content);
    let list = success(invoke(&destination, 0x0802, empty_params()));
    match map_get(&list, "items") {
        Some(Value::Array(items)) => assert!(items.is_empty(), "completed downloads leave the queue"),
        other => panic!("missing download list items: {other:?}"),
    }

    // A second request for content the source cannot serve must fail with a
    // stable typed error instead of creating a partial row.
    let missing = success(invoke(
        &destination,
        0x0801,
        params(vec![
            ("content_id", Value::Text(to_hex(&[0xAB_u8; 32]))),
            ("peer_id", Value::Text(source_peer)),
        ]),
    ));
    assert!(map_get(&missing, "accepted").and_then(Value::as_bool) == Some(true));
    let failed = wait_for_event(&destination, "download.failed", Duration::from_secs(30));
    assert_eq!(
        map_get(&failed.data, "error")
            .and_then(|error| map_get(error, "code"))
            .and_then(Value::as_text),
        Some(DomainCode::NotFound.as_str())
    );

    destination.stop().expect("stop destination");
    source.stop().expect("stop source");
}
