//! Stage 14 integration: a publisher imports and explicitly publishes a file,
//! a second engine discovers the signed record through the DHT, downloads the
//! content, and observes the signed withdrawal after unpublishing.
//!
//! The publisher and the reader are DHT servers so the publisher can store its
//! record with a reachable peer even before any wider network exists.
//! Signature status, local serving, and content availability are asserted
//! separately so no step can claim more than the network can deliver.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::sleep;
use std::time::{Duration, Instant};

use nexus_core::{
    CoreConfig, DomainCode, Engine, LogLevel, MAX_EVENT_BYTES, MAX_STREAM_CHUNK, Response, decode_response,
};
use nexus_crypto::{CborLimits, Value, decode};
use nexus_identity::{IdentityVault, LocalEncryptionKey};

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(label: &str) -> Self {
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("nexus-core-stage14-{label}-{}-{sequence}", std::process::id()));
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

fn config(root: &Path, server: bool) -> CoreConfig {
    CoreConfig {
        data_root: root.to_path_buf(),
        app_version: "0.1.0-test".to_owned(),
        locale: "en".to_owned(),
        log_level: LogLevel::Info,
        network_enabled: true,
        dht_mode: if server {
            nexus_core::DhtMode::Server
        } else {
            nexus_core::DhtMode::Client
        },
        relay_fallback: false,
        relay_server: false,
        search_provider: false,
        quota_bytes: 1024 * 1024 * 1024,
        enable_test_methods: false,
    }
}

fn start(root: &Path, key: u8, server: bool) -> Engine {
    let local_key = LocalEncryptionKey::new([key; 32]);
    IdentityVault::new(root.to_path_buf())
        .expect("identity vault")
        .create(&local_key)
        .expect("create identity");
    let engine = Engine::create(config(root, server), &[key; 32]).expect("create engine");
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

fn failure_code(response: Response) -> DomainCode {
    match response {
        Response::Failure(error) => error.code,
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

fn bool_field(value: &Value, key: &str) -> bool {
    map_get(value, key)
        .and_then(Value::as_bool)
        .unwrap_or_else(|| panic!("missing bool field {key}"))
}

fn array_field<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    match map_get(value, key) {
        Some(Value::Array(items)) => items,
        other => panic!("missing array field {key}: {other:?}"),
    }
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

fn bootstrap(engine: &Engine, multiaddr: &str) {
    success(invoke(
        engine,
        0x0604,
        params(vec![("multiaddr", Value::Text(multiaddr.to_owned()))]),
    ));
}

fn wait_for_dht_peer(engine: &Engine) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let current = status(engine);
        if unsigned_field(&current, "dht_routing_peers") >= 1 && bool_field(&current, "dht_bootstrapped") {
            return;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for a DHT peer: {current:?}");
        }
        sleep(Duration::from_millis(50));
    }
}

fn import_file(engine: &Engine, name: &str, content: &[u8]) -> String {
    let import_id = engine
        .import_begin(name, u64::try_from(content.len()).expect("size"))
        .expect("begin import");
    for chunk in content.chunks(MAX_STREAM_CHUNK) {
        engine.import_write(import_id, chunk).expect("write import");
    }
    let summary = engine.import_finish(import_id).expect("finish import");
    let value = decode(&summary, CborLimits::new(4, 16, 4_096)).expect("import summary");
    text_field(&value, "content_id")
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

fn publication_get(engine: &Engine, publication_id: &str) -> Value {
    success(invoke(
        engine,
        0x0503,
        params(vec![("publication_id", Value::Text(publication_id.to_owned()))]),
    ))
}

fn lookup(engine: &Engine, publication_id: &str) -> u64 {
    let response = success(invoke(
        engine,
        0x0505,
        params(vec![("publication_id", Value::Text(publication_id.to_owned()))]),
    ));
    unsigned_field(&response, "op_id")
}

fn lookup_record(event: &RecordedEvent) -> &Value {
    map_get(&event.data, "record").unwrap_or_else(|| panic!("lookup event has no record: {:?}", event.data))
}

fn preview(value: &Value) -> &Value {
    map_get(value, "preview").unwrap_or_else(|| panic!("missing preview: {value:?}"))
}

fn assert_safe_text_preview(value: &Value) {
    let preview = preview(value);
    assert_eq!(text_field(preview, "kind"), "text");
    assert!(bool_field(preview, "supported"));
    assert!(!bool_field(preview, "auto_execute"));
    assert_eq!(text_field(preview, "reason"), "plain_text");
}

#[test]
fn publisher_announces_serves_and_withdraws_a_signed_publication() {
    let publisher_root = TempRoot::new("publisher");
    let reader_root = TempRoot::new("reader");

    let publisher = start(&publisher_root.path, 71, true);
    let reader = start(&reader_root.path, 72, true);
    wait_for_ready(&publisher);
    wait_for_ready(&reader);

    let publisher_address = dial_address(&publisher);
    bootstrap(&reader, &publisher_address);
    wait_for_dht_peer(&reader);

    let publisher_peer = text_field(&status(&publisher), "peer_id");
    let content: Vec<u8> =
        b"Stage 14 publishes explicit files.\nSignature, serving, and availability are separate facts.\n".to_vec();
    let content_id = import_file(&publisher, "stage14-notes.txt", &content);
    assert_eq!(content_id.len(), 64);

    // Publishing stores and announces the signed record and provider record.
    let created = success(invoke(
        &publisher,
        0x0501,
        params(vec![
            ("content_id", Value::Text(content_id.clone())),
            ("display_filename", Value::Text("stage14-notes.txt".to_owned())),
            ("description", Value::Text("A Stage 14 integration note".to_owned())),
            ("category", Value::Text("documents".to_owned())),
            (
                "tags",
                Value::Array(vec![Value::Text("stage14".to_owned()), Value::Text("notes".to_owned())]),
            ),
        ]),
    ));
    let publication_id = text_field(&created, "publication_id");
    assert_eq!(publication_id.len(), 64);
    let published = wait_for_event(&publisher, "publication.published", Duration::from_secs(30));
    assert_eq!(published.op_id, Some(unsigned_field(&created, "op_id")));
    assert_eq!(
        map_get(&published.data, "publication_id").and_then(Value::as_text),
        Some(publication_id.as_str())
    );
    assert!(bool_field(&published.data, "record_verified"));
    assert!(bool_field(&published.data, "announced"), "{:?}", published.data);
    assert!(bool_field(&published.data, "provider_announced"));
    assert!(bool_field(&published.data, "serving"));
    assert_eq!(text_field(&published.data, "availability"), "local");
    assert_safe_text_preview(&published.data);

    // The publisher reports signed metadata and serving state separately.
    let local = publication_get(&publisher, &publication_id);
    assert!(bool_field(&local, "found"));
    assert!(bool_field(&local, "signature_verified"));
    assert!(bool_field(&local, "serving"));
    assert_eq!(text_field(&local, "availability"), "local");
    let record = map_get(&local, "publication").expect("local publication");
    assert_eq!(text_field(record, "content_id"), content_id);
    assert_eq!(text_field(record, "title"), "stage14-notes.txt");
    assert_eq!(text_field(record, "filename"), "stage14-notes.txt");
    assert_eq!(text_field(record, "description"), "A Stage 14 integration note");
    assert_eq!(text_field(record, "category"), "documents");
    assert_eq!(
        unsigned_field(record, "size_bytes"),
        u64::try_from(content.len()).expect("size")
    );
    let tags = array_field(record, "tags")
        .iter()
        .map(|tag| tag.as_text().expect("tag").to_owned())
        .collect::<Vec<_>>();
    assert_eq!(tags, vec!["stage14".to_owned(), "notes".to_owned()]);
    assert!(bool_field(record, "local"));
    assert!(bool_field(record, "serving"));
    assert!(bool_field(record, "signature_verified"));
    assert_safe_text_preview(record);

    let listed = success(invoke(&publisher, 0x0504, empty_params()));
    assert_eq!(unsigned_field(&listed, "total"), 1);
    let items = array_field(&listed, "publications");
    assert_eq!(items.len(), 1);
    assert_eq!(text_field(&items[0], "publication_id"), publication_id);
    assert!(bool_field(&items[0], "serving"));
    assert!(bool_field(&items[0], "signature_verified"));
    assert_safe_text_preview(&items[0]);

    // The reader discovers and verifies the record through the DHT.
    let lookup_op = lookup(&reader, &publication_id);
    let completed = wait_for_event(&reader, "publication.lookup_completed", Duration::from_secs(60));
    assert_eq!(completed.op_id, Some(lookup_op));
    assert!(bool_field(&completed.data, "found"));
    assert_eq!(text_field(&completed.data, "source"), "network");
    assert_eq!(text_field(&completed.data, "availability"), "available_now");
    assert!(!bool_field(&completed.data, "stale"));
    assert!(!bool_field(&completed.data, "conflict"));
    let record = lookup_record(&completed);
    assert_eq!(text_field(record, "publication_id"), publication_id);
    assert_eq!(text_field(record, "content_id"), content_id);
    assert!(bool_field(record, "signature_verified"));
    assert!(!bool_field(record, "local"));
    assert!(!bool_field(record, "serving"));
    assert_safe_text_preview(record);
    let providers = array_field(&completed.data, "providers");
    assert!(
        providers
            .iter()
            .any(|provider| text_field(provider, "peer_id") == publisher_peer),
        "publisher must be listed as a provider: {:?}",
        completed.data
    );

    let cached = publication_get(&reader, &publication_id);
    assert!(bool_field(&cached, "found"));
    assert!(bool_field(&cached, "signature_verified"));
    assert!(!bool_field(&cached, "serving"));
    assert_eq!(text_field(&cached, "availability"), "available_now");
    let cached_record = map_get(&cached, "publication").expect("cached publication");
    assert_eq!(text_field(cached_record, "title"), "stage14-notes.txt");
    assert!(!bool_field(cached_record, "local"));

    // A discovered record is enough to download from the online source. The
    // reader is already connected to the publisher through bootstrap.
    let started = success(invoke(
        &reader,
        0x0801,
        params(vec![
            ("content_id", Value::Text(content_id.clone())),
            ("peer_id", Value::Text(publisher_peer.clone())),
            ("display_name", Value::Text("stage14-copy.txt".to_owned())),
        ]),
    ));
    let download_id = text_field(&started, "download_id");
    let completed = wait_for_event(&reader, "download.completed", Duration::from_secs(60));
    assert_eq!(
        map_get(&completed.data, "download_id").and_then(Value::as_text),
        Some(download_id.as_str())
    );
    assert_eq!(
        map_get(&completed.data, "content_id").and_then(Value::as_text),
        Some(content_id.as_str())
    );
    assert_eq!(export(&reader, &content_id), content);

    // Only the owner can withdraw, and withdrawal requires confirmation.
    assert_eq!(
        failure_code(invoke(
            &reader,
            0x0502,
            params(vec![
                ("publication_id", Value::Text(publication_id.clone())),
                ("confirmation", Value::Bool(true)),
            ]),
        )),
        DomainCode::Conflict
    );
    assert_eq!(
        failure_code(invoke(
            &publisher,
            0x0502,
            params(vec![("publication_id", Value::Text(publication_id.clone()))]),
        )),
        DomainCode::Conflict
    );

    let unpublished = success(invoke(
        &publisher,
        0x0502,
        params(vec![
            ("publication_id", Value::Text(publication_id.clone())),
            ("confirmation", Value::Bool(true)),
        ]),
    ));
    assert_eq!(text_field(&unpublished, "tombstone_id"), format!("{publication_id}:1"));
    let withdrawn = wait_for_event(&publisher, "publication.unpublished", Duration::from_secs(30));
    assert_eq!(withdrawn.op_id, Some(unsigned_field(&unpublished, "op_id")));
    assert!(bool_field(&withdrawn.data, "tombstone_announced"));
    assert!(bool_field(&withdrawn.data, "provider_withdrawn"));
    assert!(!bool_field(&withdrawn.data, "serving"));
    assert!(bool_field(&withdrawn.data, "local_bytes_retained"));
    assert!(!bool_field(&withdrawn.data, "record_removed"));
    assert_eq!(text_field(&withdrawn.data, "availability"), "record_only");
    assert!(text_field(&withdrawn.data, "limits").contains("not deleted"));

    // The local view keeps the record and bytes but stops claiming service.
    let local = publication_get(&publisher, &publication_id);
    assert!(bool_field(&local, "signature_verified"));
    assert!(!bool_field(&local, "serving"));
    assert_eq!(text_field(&local, "availability"), "record_only");
    let record = map_get(&local, "publication").expect("local publication");
    assert!(bool_field(record, "tombstoned"));
    assert!(!bool_field(record, "serving"));
    assert_eq!(text_field(record, "availability"), "record_only");
    let tombstone = map_get(&local, "tombstone").expect("tombstone");
    assert_eq!(text_field(tombstone, "tombstone_id"), format!("{publication_id}:1"));
    assert!(bool_field(tombstone, "signature_verified"));
    assert!(text_field(tombstone, "limits").contains("not deleted"));
    assert_eq!(export(&publisher, &content_id), content);

    // The reader learns the signed withdrawal and stops promising availability.
    let lookup_op = lookup(&reader, &publication_id);
    let completed = wait_for_event(&reader, "publication.lookup_completed", Duration::from_secs(60));
    assert_eq!(completed.op_id, Some(lookup_op));
    assert!(bool_field(&completed.data, "found"));
    assert_eq!(text_field(&completed.data, "availability"), "record_only");
    assert!(!bool_field(&completed.data, "conflict"));
    let record = lookup_record(&completed);
    assert!(bool_field(record, "tombstoned"));
    assert!(!bool_field(record, "serving"));
    assert_eq!(text_field(record, "availability"), "record_only");
    assert!(bool_field(record, "signature_verified"));
    let tombstone = map_get(record, "tombstone").expect("cached tombstone");
    assert!(bool_field(tombstone, "signature_verified"));

    let cached = publication_get(&reader, &publication_id);
    assert_eq!(text_field(&cached, "availability"), "record_only");
    let cached_record = map_get(&cached, "publication").expect("cached publication");
    assert!(bool_field(cached_record, "tombstoned"));
    assert!(bool_field(cached_record, "signature_verified"));

    reader.stop().expect("stop reader");
    publisher.stop().expect("stop publisher");
}
