//! Stage 09 integration: two cores connect and exchange a test message.
//!
//! Every test binds loopback listeners. Nothing here claims public
//! reachability, and no test depends on an external service.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::sleep;
use std::time::{Duration, Instant};

use nexus_core::{CoreConfig, DomainCode, DomainError, Engine, LogLevel, MAX_EVENT_BYTES, Response, decode_response};
use nexus_crypto::{CborLimits, Value, decode};

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(label: &str) -> Self {
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("nexus-core-stage09-{label}-{}-{sequence}", std::process::id()));
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
        quota_bytes: 1024 * 1024 * 1024,
        enable_test_methods: false,
    }
}

fn start(root: &Path) -> Engine {
    let engine = Engine::create(config(root), &[9_u8; 32]).expect("create engine");
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

fn failure(response: Response) -> DomainError {
    match response {
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

fn bytes_field(value: &Value, key: &str) -> Vec<u8> {
    match map_get(value, key) {
        Some(Value::Bytes(bytes)) => bytes.clone(),
        other => panic!("missing byte string field {key}: {other:?}"),
    }
}

fn text_array(value: &Value, key: &str) -> Vec<String> {
    match map_get(value, key) {
        Some(Value::Array(items)) => items.iter().filter_map(Value::as_text).map(str::to_owned).collect(),
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

fn wait_for_peer_count(engine: &Engine, expected: u64) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let current = status(engine);
        if unsigned_field(&current, "peer_count") >= expected {
            return;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for {expected} peers: {current:?}");
        }
        sleep(Duration::from_millis(50));
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

fn send_test(engine: &Engine, peer_id: &str, payload: &[u8]) -> u64 {
    let response = success(invoke(
        engine,
        0x0608,
        params(vec![
            ("peer_id", Value::Text(peer_id.to_owned())),
            ("payload", Value::Bytes(payload.to_vec())),
        ]),
    ));
    unsigned_field(&response, "op_id")
}

#[test]
fn two_cores_exchange_a_test_message_over_quic_and_tcp() {
    let root_a = TempRoot::new("exchange-a");
    let root_b = TempRoot::new("exchange-b");
    let a = start(&root_a.path);
    let b = start(&root_b.path);

    let (peer_b, quic_b) = wait_for_listen(&b, "/quic-v1");
    let (peer_a, _) = wait_for_listen(&a, "/quic-v1");
    assert_ne!(peer_a, peer_b, "each device has its own peer identity");

    let quic_target = format!("{}/p2p/{peer_b}", loopback(&quic_b));
    let connect_op = connect(&a, &quic_target);
    let connected = wait_for_event(&a, "network.connect_completed", Duration::from_secs(20));
    assert_eq!(connected.op_id, Some(connect_op));
    assert_eq!(text_field(&connected.data, "peer_id"), peer_b);
    assert_eq!(text_field(&connected.data, "transport"), "quic");
    assert_eq!(text_field(&connected.data, "multiaddr"), quic_target);

    let quic_payload = b"ember stage 09 over quic";
    let test_op = send_test(&a, &peer_b, quic_payload);
    let message = wait_for_event(&a, "network.test_message", Duration::from_secs(20));
    assert_eq!(message.op_id, Some(test_op));
    assert_eq!(text_field(&message.data, "peer_id"), peer_b);
    assert_eq!(bytes_field(&message.data, "payload"), quic_payload);

    let (_, tcp_b) = wait_for_listen(&b, "/tcp/");
    let tcp_target = format!("{}/p2p/{peer_b}", loopback(&tcp_b));
    let connect_op = connect(&a, &tcp_target);
    let connected = wait_for_event(&a, "network.connect_completed", Duration::from_secs(20));
    assert_eq!(connected.op_id, Some(connect_op));
    assert_eq!(text_field(&connected.data, "peer_id"), peer_b);
    assert_eq!(text_field(&connected.data, "transport"), "tcp");

    let tcp_payload = b"ember stage 09 over tcp/noise/yamux";
    let test_op = send_test(&a, &peer_b, tcp_payload);
    let message = wait_for_event(&a, "network.test_message", Duration::from_secs(20));
    assert_eq!(message.op_id, Some(test_op));
    assert_eq!(bytes_field(&message.data, "payload"), tcp_payload);

    wait_for_peer_count(&a, 1);
    wait_for_peer_count(&b, 1);
    let status = status(&a);
    assert_eq!(unsigned_field(&status, "peer_count"), 1);
    assert!(unsigned_field(&status, "connection_count") >= 2);
    assert_eq!(text_array(&status, "transports"), vec!["quic", "tcp"]);
    assert!(text_array(&status, "confirmed_addresses").is_empty());

    let health = success(invoke(&a, 0x0102, empty_params()));
    assert_eq!(text_field(&health, "network_state"), "online");

    let snapshot = success(invoke(
        &a,
        0x0103,
        params(vec![(
            "sections",
            Value::Array(vec![Value::Text("network".to_owned())]),
        )]),
    ));
    let network = map_get(&snapshot, "network").expect("network section");
    assert_eq!(text_field(network, "state"), "online");
    assert_eq!(text_field(network, "peer_id"), peer_a);
    assert_eq!(unsigned_field(network, "peer_count"), 1);
    assert!(text_array(network, "confirmed_addresses").is_empty());

    a.stop().expect("stop a");
    b.stop().expect("stop b");
}

#[test]
fn a_mismatched_peer_identity_fails_without_stopping_the_core() {
    let root_a = TempRoot::new("mismatch-a");
    let root_b = TempRoot::new("mismatch-b");
    let root_c = TempRoot::new("mismatch-c");
    let a = start(&root_a.path);
    let b = start(&root_b.path);
    let c = start(&root_c.path);

    let (peer_b, tcp_b) = wait_for_listen(&b, "/tcp/");
    let (peer_c, _) = wait_for_listen(&c, "/tcp/");
    assert_ne!(peer_b, peer_c);

    // Dial B's socket while claiming C's authenticated identity.
    let target = format!("{}/p2p/{peer_c}", loopback(&tcp_b));
    let connect_op = connect(&a, &target);
    let failed = wait_for_event(&a, "network.connect_failed", Duration::from_secs(20));
    assert_eq!(failed.op_id, Some(connect_op));
    assert_eq!(text_field(&failed.data, "multiaddr"), target);

    let error = map_get(&failed.data, "error").expect("error field");
    assert_eq!(text_field(error, "code"), DomainCode::VerificationFailed.as_str());
    let details = map_get(error, "details").expect("details field");
    assert_eq!(text_field(details, "net_code"), "wrong_peer_id");

    let peers = success(invoke(&a, 0x0602, empty_params()));
    let connected_to_c = match map_get(&peers, "peers") {
        Some(Value::Array(items)) => items
            .iter()
            .any(|peer| text_field(peer, "peer_id") == peer_c && text_field(peer, "state") == "connected"),
        other => panic!("peers array: {other:?}"),
    };
    assert!(!connected_to_c, "a mismatched identity is never connected");

    // The failed dial is bounded: the core stays running and usable.
    let health = success(invoke(&a, 0x0102, empty_params()));
    assert_eq!(text_field(&health, "state"), "running");
    let target = format!("{}/p2p/{peer_b}", loopback(&tcp_b));
    connect(&a, &target);
    let connected = wait_for_event(&a, "network.connect_completed", Duration::from_secs(20));
    assert_eq!(text_field(&connected.data, "peer_id"), peer_b);
    send_test(&a, &peer_b, b"authenticated after a rejection");
    let message = wait_for_event(&a, "network.test_message", Duration::from_secs(20));
    assert_eq!(
        bytes_field(&message.data, "payload"),
        b"authenticated after a rejection"
    );

    a.stop().expect("stop a");
    b.stop().expect("stop b");
    c.stop().expect("stop c");
}

#[test]
fn a_network_start_failure_leaves_the_core_usable() {
    let root = TempRoot::new("start-failure");
    let network_directory = root.path.join("network");
    fs::create_dir_all(&network_directory).expect("network directory");
    fs::write(network_directory.join("peer_key.enc"), b"not a peer key").expect("corrupt peer key");

    let engine = start(&root.path);
    let health = success(invoke(&engine, 0x0102, empty_params()));
    assert_eq!(text_field(&health, "state"), "running");
    assert_eq!(text_field(&health, "network_state"), "unavailable");

    let status = status(&engine);
    assert_eq!(text_field(&status, "state"), "unavailable");
    assert_eq!(
        map_get(&status, "last_error").and_then(Value::as_text),
        Some("peer_key_invalid")
    );
    assert!(text_array(&status, "listen_addresses").is_empty());

    let start_failure = wait_for_event(&engine, "network.state", Duration::from_secs(5));
    assert_eq!(text_field(&start_failure.data, "state"), "unavailable");
    assert_eq!(text_field(&start_failure.data, "reason"), "peer_key_invalid");

    let response = invoke(
        &engine,
        0x0607,
        params(vec![(
            "multiaddr",
            Value::Text("/ip4/127.0.0.1/tcp/9/p2p/12D3KooWDpJ7As7BWAwRMfu1VU2WCqNjvq387JEYKDBj4kx6nXTN".to_owned()),
        )]),
    );
    assert_eq!(failure(response).code, DomainCode::NetworkUnavailable);

    let response = invoke(
        &engine,
        0x0604,
        params(vec![("multiaddr", Value::Text("/ip4/127.0.0.1/tcp/9".to_owned()))]),
    );
    assert_eq!(failure(response).code, DomainCode::NetworkUnavailable);

    let health = success(invoke(&engine, 0x0102, empty_params()));
    assert_eq!(text_field(&health, "state"), "running");
    engine.stop().expect("stop engine");
}

#[test]
fn network_config_failures_return_domain_errors() {
    let root = TempRoot::new("domain-errors");
    let engine = start(&root.path);

    let response = invoke(
        &engine,
        0x0604,
        params(vec![("multiaddr", Value::Text("/ip4/127.0.0.1/tcp/4001".to_owned()))]),
    );
    assert_eq!(failure(response).code, DomainCode::InvalidArgument);

    let response = invoke(
        &engine,
        0x0608,
        params(vec![
            ("peer_id", Value::Text("not-a-peer".to_owned())),
            ("payload", Value::Bytes(vec![1_u8; 4 * 1024 + 1])),
        ]),
    );
    assert_eq!(failure(response).code, DomainCode::InvalidArgument);

    engine.stop().expect("stop engine");
}
