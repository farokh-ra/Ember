//! Stage 15 integration: explicit voluntary hosting, serving after the
//! original publisher withdraws, pause/resume/remove lifecycle, hosting
//! policy gates, and full re-verification.
//!
//! The replica host imports the same bytes locally (simulating a copy acquired
//! out of band) so a successful download from it can only come from the active
//! replica, not from a retained download reference.

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
        let path = std::env::temp_dir().join(format!("nexus-core-stage15-{label}-{}-{sequence}", std::process::id()));
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
        quota_bytes: 1024 * 1024 * 1024,
        enable_test_methods: false,
    }
}

fn create_engine(root: &Path, key: u8, server: bool) -> Engine {
    Engine::create(config(root, server), &[key; 32]).expect("create engine")
}

fn start(root: &Path, key: u8, server: bool) -> Engine {
    let local_key = LocalEncryptionKey::new([key; 32]);
    IdentityVault::new(root.to_path_buf())
        .expect("identity vault")
        .create(&local_key)
        .expect("create identity");
    let engine = create_engine(root, key, server);
    engine.start().expect("start engine");
    engine
}

fn restart(root: &Path, key: u8, server: bool) -> Engine {
    let engine = create_engine(root, key, server);
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

fn publish(engine: &Engine, content_id: &str, title: &str) -> String {
    let created = success(invoke(
        engine,
        0x0501,
        params(vec![
            ("content_id", Value::Text(content_id.to_owned())),
            ("display_filename", Value::Text(title.to_owned())),
            ("description", Value::Text("A Stage 15 integration file".to_owned())),
            ("category", Value::Text("documents".to_owned())),
            (
                "tags",
                Value::Array(vec![
                    Value::Text("stage15".to_owned()),
                    Value::Text("hosting".to_owned()),
                ]),
            ),
        ]),
    ));
    let publication_id = text_field(&created, "publication_id");
    let event = wait_for_event(engine, "publication.published", Duration::from_secs(30));
    assert_eq!(event.op_id, Some(unsigned_field(&created, "op_id")));
    assert!(bool_field(&event.data, "record_verified"));
    assert!(bool_field(&event.data, "provider_announced"), "{:?}", event.data);
    publication_id
}

fn add_replica(engine: &Engine, publication_id: &str, source_peer: Option<&str>) -> Value {
    let mut entries = vec![
        ("publication_id", Value::Text(publication_id.to_owned())),
        ("confirmation", Value::Bool(true)),
    ];
    if let Some(peer) = source_peer {
        entries.push(("source_peer", Value::Text(peer.to_owned())));
    }
    success(invoke(engine, 0x0901, params(entries)))
}

fn replica_list(engine: &Engine, include_removed: bool) -> Value {
    success(invoke(
        engine,
        0x0902,
        params(vec![("include_removed", Value::Bool(include_removed))]),
    ))
}

fn start_download(engine: &Engine, content_id: &str, peer_id: &str, name: &str) -> String {
    let started = success(invoke(
        engine,
        0x0801,
        params(vec![
            ("content_id", Value::Text(content_id.to_owned())),
            ("peer_id", Value::Text(peer_id.to_owned())),
            ("display_name", Value::Text(name.to_owned())),
        ]),
    ));
    text_field(&started, "download_id")
}

fn wait_for_download_outcome(engine: &Engine, download_id: &str, within: Duration) -> (String, Value) {
    let deadline = Instant::now() + within;
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let timeout = u32::try_from(remaining.as_millis().min(200)).unwrap_or(200);
        if let Some(bytes) = engine.next_event(timeout).expect("read event") {
            let event = decode_event(&bytes);
            let matches_id = map_get(&event.data, "download_id").and_then(Value::as_text) == Some(download_id);
            if matches_id && matches!(event.event_type.as_str(), "download.completed" | "download.failed") {
                return (event.event_type, event.data);
            }
        }
    }
    panic!("timed out waiting for download outcome {download_id}");
}

fn download_from(engine: &Engine, content_id: &str, peer_id: &str, name: &str, expected: &[u8]) {
    let download_id = start_download(engine, content_id, peer_id, name);
    let (outcome, data) = wait_for_download_outcome(engine, &download_id, Duration::from_secs(60));
    assert_eq!(outcome, "download.completed", "{data:?}");
    assert_eq!(export(engine, content_id), expected);
}

fn download_fails(engine: &Engine, content_id: &str, peer_id: &str, name: &str) {
    let download_id = start_download(engine, content_id, peer_id, name);
    let (outcome, data) = wait_for_download_outcome(engine, &download_id, Duration::from_secs(60));
    assert_eq!(outcome, "download.failed", "{data:?}");
    assert!(map_get(&data, "error").is_some(), "failure carries an error: {data:?}");
}

fn blob_path(root: &Path, content_id: &str) -> PathBuf {
    root.join("store")
        .join("blobs")
        .join(&content_id[0..2])
        .join(&content_id[2..4])
        .join(content_id)
}

#[test]
fn explicit_replica_serves_after_withdrawal_and_removal_preserves_records() {
    let origin_root = TempRoot::new("origin");
    let host_root = TempRoot::new("host");
    let reader_root = TempRoot::new("reader");

    let origin = start(&origin_root.path, 81, true);
    let host = start(&host_root.path, 82, true);
    let reader = start(&reader_root.path, 83, false);
    wait_for_ready(&origin);
    wait_for_ready(&host);
    wait_for_ready(&reader);

    let origin_peer = text_field(&status(&origin), "peer_id");
    let host_peer = text_field(&status(&host), "peer_id");
    bootstrap(&host, &dial_address(&origin));
    wait_for_dht_peer(&host);
    bootstrap(&reader, &dial_address(&host));
    wait_for_dht_peer(&reader);

    let content = b"Stage 15 hosts explicit copies.\nA replica keeps the signed publication intact.\n".to_vec();
    let content_id = import_file(&origin, "stage15-origin.txt", &content);
    let publication_id = publish(&origin, &content_id, "stage15-origin.txt");

    // The host discovers the signed record, then explicitly imports and hosts
    // the same bytes. Hosting is never automatic.
    let lookup_op = lookup(&host, &publication_id);
    let discovered = wait_for_event(&host, "publication.lookup_completed", Duration::from_secs(60));
    assert_eq!(discovered.op_id, Some(lookup_op));
    assert!(bool_field(&discovered.data, "found"));
    let host_content_id = import_file(&host, "stage15-hosted.txt", &content);
    assert_eq!(host_content_id, content_id);

    let added = add_replica(&host, &publication_id, Some(&origin_peer));
    let replica = map_get(&added, "replica").expect("replica result");
    let replica_id = text_field(replica, "replica_id");
    assert_eq!(text_field(replica, "state"), "active");
    assert_eq!(text_field(replica, "publication_id"), publication_id);
    assert_eq!(text_field(replica, "source_peer"), origin_peer);
    assert!(bool_field(&added, "attribution_preserved"));
    let active = wait_for_event(&host, "replica.active", Duration::from_secs(30));
    assert_eq!(active.op_id, Some(unsigned_field(&added, "op_id")));
    assert!(bool_field(&active.data, "announced"), "{:?}", active.data);
    assert!(bool_field(&active.data, "verified"));
    assert!(bool_field(&active.data, "attribution_preserved"));

    let listed = replica_list(&host, false);
    let replicas = array_field(&listed, "replicas");
    assert_eq!(replicas.len(), 1);
    assert_eq!(text_field(&replicas[0], "replica_id"), replica_id);
    assert!(bool_field(&replicas[0], "announced"));
    assert!(bool_field(&replicas[0], "serving"));
    assert!(bool_field(&replicas[0], "attribution_preserved"));
    assert_eq!(
        unsigned_field(map_get(&listed, "usage").expect("usage"), "active_count"),
        1
    );

    // The original publisher withdraws. The host's verified replica is now the
    // only advertised copy, and it serves a fresh peer.
    success(invoke(
        &origin,
        0x0502,
        params(vec![
            ("publication_id", Value::Text(publication_id.clone())),
            ("confirmation", Value::Bool(true)),
        ]),
    ));
    wait_for_event(&origin, "publication.unpublished", Duration::from_secs(30));

    download_from(&reader, &content_id, &host_peer, "stage15-copy.txt", &content);

    let listed = replica_list(&host, false);
    let replica = &array_field(&listed, "replicas")[0];
    assert!(
        map_get(replica, "last_seen_at_ms")
            .and_then(Value::as_unsigned)
            .is_some(),
        "serving updates last seen: {replica:?}"
    );

    // Pausing stops serving immediately; resuming verifies and serves again.
    let paused = success(invoke(
        &host,
        0x0903,
        params(vec![("replica_id", Value::Text(replica_id.clone()))]),
    ));
    assert_eq!(
        text_field(map_get(&paused, "replica").expect("replica"), "state"),
        "paused"
    );
    wait_for_event(&host, "replica.paused", Duration::from_secs(30));
    download_fails(&reader, &content_id, &host_peer, "stage15-paused.txt");

    let resumed = success(invoke(
        &host,
        0x0904,
        params(vec![("replica_id", Value::Text(replica_id.clone()))]),
    ));
    assert_eq!(
        text_field(map_get(&resumed, "replica").expect("replica"), "state"),
        "active"
    );
    wait_for_event(&host, "replica.resumed", Duration::from_secs(30));
    wait_for_event(&host, "replica.active", Duration::from_secs(30));
    download_from(&reader, &content_id, &host_peer, "stage15-resumed.txt", &content);

    // Removing the replica stops this device serving but leaves the original
    // signed record and its attribution untouched.
    let removed = success(invoke(
        &host,
        0x0905,
        params(vec![
            ("replica_id", Value::Text(replica_id.clone())),
            ("confirmation", Value::Bool(true)),
        ]),
    ));
    assert_eq!(
        text_field(map_get(&removed, "replica").expect("replica"), "state"),
        "removed"
    );
    let removal = wait_for_event(&host, "replica.removed", Duration::from_secs(30));
    assert!(bool_field(&removal.data, "publication_preserved"));
    assert!(bool_field(&removal.data, "attribution_preserved"));
    assert_eq!(text_field(&removal.data, "publication_id"), publication_id);
    download_fails(&reader, &content_id, &host_peer, "stage15-removed.txt");

    let cached = publication_get(&host, &publication_id);
    assert!(bool_field(&cached, "found"));
    assert!(bool_field(&cached, "signature_verified"));
    let cached_record = map_get(&cached, "publication").expect("cached publication");
    assert_eq!(text_field(cached_record, "content_id"), content_id);
    assert_eq!(text_field(cached_record, "title"), "stage15-origin.txt");
    assert!(!bool_field(cached_record, "local"));

    let original = publication_get(&origin, &publication_id);
    assert!(bool_field(&original, "found"));
    assert!(bool_field(&original, "signature_verified"));
    let original_record = map_get(&original, "publication").expect("original publication");
    assert_eq!(text_field(original_record, "title"), "stage15-origin.txt");
    assert!(bool_field(original_record, "tombstoned"));
    assert!(!bool_field(original_record, "serving"));

    let all = replica_list(&host, true);
    let all_replicas = array_field(&all, "replicas");
    assert_eq!(all_replicas.len(), 1);
    assert_eq!(text_field(&all_replicas[0], "state"), "removed");
    assert!(!bool_field(&all_replicas[0], "announced"));
    assert_eq!(
        unsigned_field(map_get(&all, "usage").expect("usage"), "active_count"),
        0
    );

    // Re-adding after removal is explicit and reuses the deterministic row.
    assert_eq!(
        failure_code(invoke(
            &host,
            0x0901,
            params(vec![("publication_id", Value::Text(publication_id.clone()))]),
        )),
        DomainCode::Conflict
    );
    let readded = add_replica(&host, &publication_id, Some(&origin_peer));
    let readded_replica = map_get(&readded, "replica").expect("replica");
    assert_eq!(text_field(readded_replica, "replica_id"), replica_id);
    assert_eq!(text_field(readded_replica, "state"), "active");
    let active = wait_for_event(&host, "replica.active", Duration::from_secs(30));
    assert!(bool_field(&active.data, "verified"));

    reader.stop().expect("stop reader");
    host.stop().expect("stop host");
    origin.stop().expect("stop origin");
}

#[test]
fn hosting_limits_block_serving_and_persist_across_restart() {
    let origin_root = TempRoot::new("limits-origin");
    let reader_root = TempRoot::new("limits-reader");

    let origin = start(&origin_root.path, 84, true);
    let reader = start(&reader_root.path, 85, false);
    wait_for_ready(&origin);
    wait_for_ready(&reader);
    bootstrap(&reader, &dial_address(&origin));
    wait_for_dht_peer(&reader);

    let origin_peer = text_field(&status(&origin), "peer_id");
    let content = b"Stage 15 hosting policy gates every served file.\n".to_vec();
    let content_id = import_file(&origin, "stage15-limits.txt", &content);
    let added = success(invoke(
        &origin,
        0x0901,
        params(vec![
            ("content_id", Value::Text(content_id.clone())),
            ("confirmation", Value::Bool(true)),
        ]),
    ));
    let replica_id = text_field(map_get(&added, "replica").expect("replica"), "replica_id");
    wait_for_event(&origin, "replica.active", Duration::from_secs(30));
    download_from(&reader, &content_id, &origin_peer, "stage15-baseline.txt", &content);

    // Metered networks are opt-in. The host keeps the replica but stops
    // advertising and serving until the user allows metered use.
    let blocked = success(invoke(
        &origin,
        0x0908,
        params(vec![
            ("metered", Value::Bool(true)),
            ("allow_metered", Value::Bool(false)),
            ("upload_bytes_per_second", Value::Unsigned(512 * 1024)),
        ]),
    ));
    assert!(!bool_field(&blocked, "serving_allowed"));
    assert_eq!(text_field(&blocked, "blocked_reason"), "metered_network");
    let limits = map_get(&blocked, "limits").expect("limits");
    assert_eq!(unsigned_field(limits, "upload_bytes_per_second"), 512 * 1024);
    assert!(!bool_field(limits, "allow_metered"));
    let changed = wait_for_event(&origin, "hosting.limits_changed", Duration::from_secs(30));
    let hosting = map_get(&changed.data, "hosting").expect("hosting");
    assert_eq!(text_field(hosting, "blocked_reason"), "metered_network");
    let listed = replica_list(&origin, false);
    assert!(!bool_field(&array_field(&listed, "replicas")[0], "serving"));
    download_fails(&reader, &content_id, &origin_peer, "stage15-metered.txt");

    let allowed = success(invoke(
        &origin,
        0x0908,
        params(vec![
            ("metered", Value::Bool(true)),
            ("allow_metered", Value::Bool(true)),
        ]),
    ));
    assert!(bool_field(&allowed, "serving_allowed"));
    download_from(
        &reader,
        &content_id,
        &origin_peer,
        "stage15-metered-allowed.txt",
        &content,
    );

    // Battery thresholds are also opt-in and never block an unknown level.
    let low = success(invoke(
        &origin,
        0x0908,
        params(vec![
            ("battery_min_percent", Value::Unsigned(50)),
            ("battery_percent", Value::Unsigned(20)),
        ]),
    ));
    assert!(!bool_field(&low, "serving_allowed"));
    assert_eq!(text_field(&low, "blocked_reason"), "battery_low");
    download_fails(&reader, &content_id, &origin_peer, "stage15-battery.txt");

    let recovered = success(invoke(
        &origin,
        0x0908,
        params(vec![("battery_percent", Value::Unsigned(80))]),
    ));
    assert!(bool_field(&recovered, "serving_allowed"));
    download_from(&reader, &content_id, &origin_peer, "stage15-battery-ok.txt", &content);

    assert_eq!(
        failure_code(invoke(
            &origin,
            0x0908,
            params(vec![("upload_bytes_per_second", Value::Unsigned(1))]),
        )),
        DomainCode::InvalidArgument
    );

    let current = success(invoke(&origin, 0x0907, empty_params()));
    let limits = map_get(&current, "limits").expect("limits");
    assert_eq!(unsigned_field(limits, "upload_bytes_per_second"), 512 * 1024);
    assert!(bool_field(limits, "allow_metered"));
    assert_eq!(unsigned_field(limits, "battery_min_percent"), 50);

    origin.stop().expect("stop origin");
    drop(origin);
    let origin = restart(&origin_root.path, 84, true);
    wait_for_ready(&origin);
    let persisted = success(invoke(&origin, 0x0907, empty_params()));
    let limits = map_get(&persisted, "limits").expect("persisted limits");
    assert_eq!(unsigned_field(limits, "upload_bytes_per_second"), 512 * 1024);
    assert!(bool_field(limits, "allow_metered"));
    assert_eq!(unsigned_field(limits, "battery_min_percent"), 50);
    assert!(bool_field(&persisted, "serving_allowed"));
    assert_eq!(map_get(&persisted, "blocked_reason").and_then(Value::as_text), None);
    let listed = replica_list(&origin, false);
    assert_eq!(
        text_field(&array_field(&listed, "replicas")[0], "replica_id"),
        replica_id
    );
    assert!(bool_field(&array_field(&listed, "replicas")[0], "serving"));

    origin.stop().expect("stop origin");
    reader.stop().expect("stop reader");
}

#[test]
fn verification_detects_corruption_and_paused_replicas_stay_paused() {
    let root = TempRoot::new("verify");
    let engine = start(&root.path, 86, true);
    wait_for_ready(&engine);

    let content = b"Stage 15 re-verifies the complete hosted file.\n".to_vec();
    let content_id = import_file(&engine, "stage15-verify.txt", &content);
    let added = success(invoke(
        &engine,
        0x0901,
        params(vec![
            ("content_id", Value::Text(content_id.clone())),
            ("confirmation", Value::Bool(true)),
        ]),
    ));
    let replica_id = text_field(map_get(&added, "replica").expect("replica"), "replica_id");
    wait_for_event(&engine, "replica.active", Duration::from_secs(30));

    // A paused replica is still verified, but verification never resumes it.
    success(invoke(
        &engine,
        0x0903,
        params(vec![("replica_id", Value::Text(replica_id.clone()))]),
    ));
    wait_for_event(&engine, "replica.paused", Duration::from_secs(30));
    success(invoke(
        &engine,
        0x0906,
        params(vec![("replica_id", Value::Text(replica_id.clone()))]),
    ));
    let verified = wait_for_event(&engine, "replica.verified", Duration::from_secs(30));
    assert!(bool_field(&verified.data, "verified"));
    assert!(!bool_field(&verified.data, "announced"));
    assert_eq!(text_field(&verified.data, "state"), "paused");
    let listed = replica_list(&engine, false);
    let replica = &array_field(&listed, "replicas")[0];
    assert_eq!(text_field(replica, "state"), "paused");
    assert!(!bool_field(replica, "serving"));

    success(invoke(
        &engine,
        0x0904,
        params(vec![("replica_id", Value::Text(replica_id.clone()))]),
    ));
    wait_for_event(&engine, "replica.resumed", Duration::from_secs(30));
    wait_for_event(&engine, "replica.active", Duration::from_secs(30));

    // Corrupt bytes in place, then ask for a full verification.
    let path = blob_path(&root.path, &content_id);
    let mut corrupt = content.clone();
    corrupt[0] ^= 0xff;
    fs::write(&path, &corrupt).expect("corrupt blob");
    success(invoke(
        &engine,
        0x0906,
        params(vec![("replica_id", Value::Text(replica_id.clone()))]),
    ));
    let failed = wait_for_event(&engine, "replica.verification_failed", Duration::from_secs(30));
    assert_eq!(text_field(&failed.data, "replica_id"), replica_id);
    let listed = replica_list(&engine, false);
    let replica = &array_field(&listed, "replicas")[0];
    assert_eq!(text_field(replica, "state"), "verification_failed");
    assert!(!bool_field(replica, "announced"));
    assert_eq!(unsigned_field(replica, "failure_count"), 1);
    assert!(map_get(replica, "last_error").and_then(Value::as_text).is_some());

    // Resume verifies first and refuses to advertise a corrupt copy.
    assert_eq!(
        failure_code(invoke(
            &engine,
            0x0904,
            params(vec![("replica_id", Value::Text(replica_id.clone()))]),
        )),
        DomainCode::VerificationFailed
    );
    wait_for_event(&engine, "replica.failed", Duration::from_secs(30));

    // Restoring the exact bytes lets an explicit resume recover the replica.
    fs::write(&path, &content).expect("restore blob");
    success(invoke(
        &engine,
        0x0904,
        params(vec![("replica_id", Value::Text(replica_id.clone()))]),
    ));
    wait_for_event(&engine, "replica.resumed", Duration::from_secs(30));
    wait_for_event(&engine, "replica.active", Duration::from_secs(30));
    let listed = replica_list(&engine, false);
    let replica = &array_field(&listed, "replicas")[0];
    assert_eq!(text_field(replica, "state"), "active");
    assert!(bool_field(replica, "announced"));
    assert_eq!(unsigned_field(replica, "failure_count"), 0);

    // A restart re-verifies every active replica before advertising it. A
    // corrupt copy is detected by the startup pass.
    fs::write(&path, &corrupt).expect("corrupt blob again");
    engine.stop().expect("stop engine");
    drop(engine);
    let engine = restart(&root.path, 86, true);
    wait_for_ready(&engine);
    let failed = wait_for_event(&engine, "replica.verification_failed", Duration::from_secs(60));
    assert_eq!(text_field(&failed.data, "replica_id"), replica_id);
    let completed = wait_for_event(&engine, "replica.maintenance_completed", Duration::from_secs(60));
    assert_eq!(unsigned_field(&completed.data, "checked"), 1);
    assert_eq!(unsigned_field(&completed.data, "failed"), 1);
    let listed = replica_list(&engine, false);
    let replica = &array_field(&listed, "replicas")[0];
    assert_eq!(text_field(replica, "state"), "verification_failed");
    assert!(!bool_field(replica, "announced"));
    assert!(!bool_field(replica, "serving"));

    engine.stop().expect("stop engine");
}
