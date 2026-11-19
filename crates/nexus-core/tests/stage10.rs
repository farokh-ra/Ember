//! Stage 10 integration: `network.discover` and DHT status reporting.
//!
//! Every test binds loopback listeners. Peers join through an ordinary
//! bootstrap address; no central account service is involved.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::sleep;
use std::time::{Duration, Instant};

use nexus_core::{CoreConfig, Engine, LogLevel, MAX_EVENT_BYTES, Response, decode_response};
use nexus_crypto::{CborLimits, Value, decode};

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(label: &str) -> Self {
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("nexus-core-stage10-{label}-{}-{sequence}", std::process::id()));
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
        relay_fallback: true,
        relay_server: false,
        quota_bytes: 1024 * 1024 * 1024,
        enable_test_methods: false,
    }
}

fn start(root: &Path, server: bool) -> Engine {
    let engine = Engine::create(config(root, server), &[10_u8; 32]).expect("create engine");
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

fn invoke(engine: &Engine, method_id: u32, params: Value) -> Response {
    let request = Value::Map(vec![
        (Value::Text("v".to_owned()), Value::Unsigned(1)),
        (Value::Text("params".to_owned()), params),
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

fn bool_field(value: &Value, key: &str) -> bool {
    map_get(value, key)
        .and_then(Value::as_bool)
        .unwrap_or_else(|| panic!("missing bool field {key}"))
}

fn bytes_field(value: &Value, key: &str) -> Vec<u8> {
    match map_get(value, key) {
        Some(Value::Bytes(bytes)) => bytes.clone(),
        other => panic!("missing byte string field {key}: {other:?}"),
    }
}

fn array_field<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    match map_get(value, key) {
        Some(Value::Array(items)) => items,
        other => panic!("missing array field {key}: {other:?}"),
    }
}

fn text_array(value: &Value, key: &str) -> Vec<String> {
    array_field(value, key)
        .iter()
        .filter_map(Value::as_text)
        .map(str::to_owned)
        .collect()
}

struct RecordedEvent {
    event_type: String,
    op_id: Option<u64>,
    data: Value,
}

fn decode_event(bytes: &[u8]) -> RecordedEvent {
    let value = decode(bytes, CborLimits::new(16, 512, MAX_EVENT_BYTES)).expect("event CBOR");
    let event_type = text_field(&value, "type");
    let op_id = map_get(&value, "op_id").and_then(Value::as_unsigned);
    let data = map_get(&value, "data").expect("event data").clone();
    RecordedEvent {
        event_type,
        op_id,
        data,
    }
}

fn wait_for_event(engine: &Engine, event_type: &str, within: Duration) -> RecordedEvent {
    let deadline = Instant::now() + within;
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let timeout = u32::try_from(remaining.as_millis().min(200)).unwrap_or(200);
        let event = engine
            .next_event(timeout)
            .expect("read event")
            .map(|bytes| decode_event(&bytes));
        if let Some(event) = event
            && event.event_type == event_type
        {
            return event;
        }
    }
    panic!("timed out waiting for {event_type}");
}

/// Replaces the wildcard listener address with loopback for dialing.
fn loopback(address: &str) -> String {
    address.replacen("/ip4/0.0.0.0/", "/ip4/127.0.0.1/", 1)
}

fn status(engine: &Engine) -> Value {
    success(invoke(engine, 0x0601, empty_params()))
}

fn wait_for_listen(engine: &Engine, needle: &str) -> (String, String) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let current = status(engine);
        let peer_id = text_field(&current, "peer_id");
        let address = text_array(&current, "listen_addresses")
            .into_iter()
            .find(|address| address.contains(needle));
        if let Some(address) = address {
            return (peer_id, address);
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for a {needle} listener: {current:?}");
        }
        sleep(Duration::from_millis(50));
    }
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

fn discover(engine: &Engine, target: Option<Value>) -> u64 {
    let entries = match target {
        Some(target) => vec![("target", target)],
        None => Vec::new(),
    };
    let response = success(invoke(engine, 0x0609, params(entries)));
    unsigned_field(&response, "op_id")
}

fn discovered_peer_ids(data: &Value) -> Vec<String> {
    array_field(data, "peers")
        .iter()
        .map(|peer| text_field(peer, "peer_id"))
        .collect()
}

#[test]
fn discovery_joins_through_bootstrap_and_reports_dht_status() {
    let root_a = TempRoot::new("discover-a");
    let root_b = TempRoot::new("discover-b");
    let a = start(&root_a.path, false);
    let b = start(&root_b.path, true);

    let (peer_b, tcp_b) = wait_for_listen(&b, "/tcp/");
    let bootstrap = format!("{}/p2p/{peer_b}", loopback(&tcp_b));

    success(invoke(
        &a,
        0x0604,
        params(vec![("multiaddr", Value::Text(bootstrap.clone()))]),
    ));
    wait_for_dht_peer(&a);

    let op_id = discover(&a, None);
    let completed = wait_for_event(&a, "network.discovery_completed", Duration::from_secs(20));
    assert_eq!(completed.op_id, Some(op_id));
    assert!(matches!(map_get(&completed.data, "target"), Some(Value::Null)));
    let peer_ids = discovered_peer_ids(&completed.data);
    assert!(
        peer_ids.iter().any(|peer| peer == &peer_b),
        "discovery must include the bootstrap peer: {peer_ids:?}"
    );
    assert!(unsigned_field(&completed.data, "routing_peers") >= 1);
    assert!(unsigned_field(&completed.data, "elapsed_ms") < 30_000);

    let targeted = discover(&a, Some(Value::Bytes(vec![7_u8; 32])));
    let completed = wait_for_event(&a, "network.discovery_completed", Duration::from_secs(20));
    assert_eq!(completed.op_id, Some(targeted));
    assert_eq!(bytes_field(&completed.data, "target"), vec![7_u8; 32]);
    assert!(
        discovered_peer_ids(&completed.data).iter().any(|peer| peer == &peer_b),
        "targeted discovery must route to the bootstrap peer"
    );

    let current = status(&a);
    assert!(matches!(text_field(&current, "dht_mode").as_str(), "client" | "server"));
    assert!(unsigned_field(&current, "dht_routing_peers") >= 1);
    assert!(bool_field(&current, "dht_bootstrapped"));
    assert!(unsigned_field(&current, "dht_queries_started") >= 2);
    assert_eq!(unsigned_field(&current, "dht_queries_failed"), 0);
    assert_eq!(unsigned_field(&current, "dht_records_rejected"), 0);

    let bootstrap_list = success(invoke(&a, 0x0603, empty_params()));
    assert!(text_array(&bootstrap_list, "bootstrap").contains(&bootstrap));

    a.stop().expect("stop a");
    b.stop().expect("stop b");
}

#[test]
fn discovery_rejects_malformed_targets_without_starting_an_operation() {
    let root = TempRoot::new("discover-invalid");
    let engine = start(&root.path, false);

    let short = engine
        .invoke(
            0x0609,
            &Value::Map(vec![
                (Value::Text("v".to_owned()), Value::Unsigned(1)),
                (
                    Value::Text("params".to_owned()),
                    params(vec![("target", Value::Bytes(vec![0_u8; 31]))]),
                ),
            ])
            .to_canonical_bytes()
            .expect("request CBOR"),
        )
        .expect_err("short target must be rejected");
    assert!(matches!(short, nexus_core::CoreError::InvalidArgument(_)));

    let wrong_type = engine
        .invoke(
            0x0609,
            &Value::Map(vec![
                (Value::Text("v".to_owned()), Value::Unsigned(1)),
                (
                    Value::Text("params".to_owned()),
                    params(vec![("target", Value::Text("not bytes".to_owned()))]),
                ),
            ])
            .to_canonical_bytes()
            .expect("request CBOR"),
        )
        .expect_err("non-byte target must be rejected");
    assert!(matches!(wrong_type, nexus_core::CoreError::InvalidArgument(_)));

    let current = status(&engine);
    assert_eq!(unsigned_field(&current, "dht_queries_started"), 0);

    engine.stop().expect("stop engine");
}
