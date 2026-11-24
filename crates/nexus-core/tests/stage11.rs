//! Stage 11 integration: community relay lists, connectivity diagnosis, and
//! a direct→relay fallback through the core command/event bus.
//!
//! A loopback relay fixture carries the circuit; both engines are ordinary
//! clients. No public service is contacted.

use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::sleep;
use std::time::{Duration, Instant};

use nexus_core::{CoreConfig, DomainCode, Engine, LogLevel, MAX_EVENT_BYTES, Response, decode_response};
use nexus_crypto::{CborLimits, Value, decode};
use nexus_identity::LocalEncryptionKey;
use nexus_net::{DEFAULT_RELAY_MAX_CIRCUIT_BYTES, Network, NetworkConfig};

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(label: &str) -> Self {
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("nexus-core-stage11-{label}-{}-{sequence}", std::process::id()));
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

/// Keeps the relay fixture's data directory alive for the whole test.
struct Relay {
    network: Network,
    address: String,
    _root: TempRoot,
}

impl Relay {
    fn start() -> Self {
        let root = TempRoot::new("relay-fixture");
        let mut config = NetworkConfig::new(root.path.clone());
        config.listen = vec!["/ip4/127.0.0.1/tcp/0".parse().expect("relay listen")];
        config.relay_server_enabled = true;
        config.relay_reservation_timeout = Duration::from_secs(3);
        config.autonat_enabled = false;
        config.dcutr_enabled = false;
        let network = Network::start(config, &LocalEncryptionKey::new([9_u8; 32]), Arc::new(())).expect("start relay");
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let status = network.status();
            if let Some(address) = status
                .listen_addresses
                .into_iter()
                .find(|address| address.contains("/tcp/"))
            {
                let address = if address.ends_with(&format!("/p2p/{}", network.peer_id())) {
                    address
                } else {
                    format!("{address}/p2p/{}", network.peer_id())
                };
                return Self {
                    network,
                    address,
                    _root: root,
                };
            }
            if Instant::now() >= deadline {
                panic!("timed out waiting for the relay listener");
            }
            sleep(Duration::from_millis(50));
        }
    }

    fn peer_id(&self) -> String {
        self.network.peer_id().to_string()
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
        relay_fallback: true,
        relay_server: false,
        search_provider: false,
        quota_bytes: 1024 * 1024 * 1024,
        enable_test_methods: false,
    }
}

fn start(root: &Path) -> Engine {
    let engine = Engine::create(config(root), &[12_u8; 32]).expect("create engine");
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

fn failure(response: Response) -> nexus_core::DomainError {
    match response {
        Response::Success(value) => panic!("expected a failure, got {value:?}"),
        Response::Failure(error) => error,
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

fn text_array(value: &Value, key: &str) -> Vec<String> {
    match map_get(value, key) {
        Some(Value::Array(items)) => items.iter().filter_map(Value::as_text).map(str::to_owned).collect(),
        other => panic!("missing array field {key}: {other:?}"),
    }
}

fn optional_unsigned_field(value: &Value, key: &str) -> Option<u64> {
    map_get(value, key).and_then(Value::as_unsigned)
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

fn status(engine: &Engine) -> Value {
    success(invoke(engine, 0x0601, empty_params()))
}

fn peer_id(engine: &Engine) -> String {
    text_field(&status(engine), "peer_id")
}

fn relay_list(engine: &Engine) -> Value {
    success(invoke(engine, 0x060A, empty_params()))
}

fn relay_add(engine: &Engine, address: &str) -> Response {
    invoke(
        engine,
        0x060B,
        params(vec![("multiaddr", Value::Text(address.to_owned()))]),
    )
}

fn relay_remove(engine: &Engine, address: &str) -> Response {
    invoke(
        engine,
        0x060C,
        params(vec![("multiaddr", Value::Text(address.to_owned()))]),
    )
}

fn connectivity(engine: &Engine) -> Value {
    success(invoke(engine, 0x060D, empty_params()))
}

fn wait_for_reservation(engine: &Engine) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let current = status(engine);
        if unsigned_field(&current, "relay_reservations") >= 1 {
            return;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for a relay reservation: {current:?}");
        }
        sleep(Duration::from_millis(50));
    }
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

fn closed_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve port");
    let port = listener.local_addr().expect("local address").port();
    drop(listener);
    port
}

fn broken_address(peer: &str) -> String {
    format!("/ip4/127.0.0.1/tcp/{}/p2p/{peer}", closed_port())
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
fn relay_list_methods_persist_and_reject_missing_peer_ids() {
    let root = TempRoot::new("relay-list");
    let relay = Relay::start();
    let engine = start(&root.path);

    assert!(text_array(&relay_list(&engine), "relays").is_empty());

    let added = success(relay_add(&engine, &relay.address));
    assert_eq!(text_array(&added, "relays"), vec![relay.address.clone()]);

    let invalid = failure(relay_add(&engine, "/ip4/127.0.0.1/tcp/4001"));
    assert_eq!(invalid.code, DomainCode::InvalidArgument);

    engine.stop().expect("stop engine");
    drop(engine);

    let restarted = start(&root.path);
    assert_eq!(
        text_array(&relay_list(&restarted), "relays"),
        vec![relay.address.clone()],
        "the relay list must survive a restart"
    );

    let removed = success(relay_remove(&restarted, &relay.address));
    assert!(text_array(&removed, "relays").is_empty());
    assert!(text_array(&relay_list(&restarted), "relays").is_empty());

    let unknown = failure(relay_remove(&restarted, &relay.address));
    assert_eq!(unknown.code, DomainCode::NotFound);

    restarted.stop().expect("stop restarted engine");
}

#[test]
fn connectivity_reports_disabled_and_idle_states_with_a_hint() {
    let root = TempRoot::new("connectivity-disabled");
    let mut disabled_config = config(&root.path);
    disabled_config.network_enabled = false;
    let disabled = Engine::create(disabled_config, &[13_u8; 32]).expect("create disabled engine");
    disabled.start().expect("start disabled engine");

    let report = connectivity(&disabled);
    assert_eq!(text_field(&report, "connectivity"), "disabled");
    assert_eq!(text_field(&report, "state"), "disabled");
    assert!(!bool_field(&report, "relay_fallback_enabled"));
    assert_eq!(unsigned_field(&report, "relays_configured"), 0);
    assert!(text_field(&report, "hint").contains("disabled"));
    disabled.stop().expect("stop disabled engine");

    let idle_root = TempRoot::new("connectivity-idle");
    let idle = start(&idle_root.path);
    wait_for_ready(&idle);
    let report = connectivity(&idle);
    assert_eq!(text_field(&report, "connectivity"), "no_peers");
    assert!(text_field(&report, "hint").contains("no community relay"));
    assert_eq!(unsigned_field(&report, "relays_configured"), 0);
    assert!(matches!(map_get(&report, "nat_status"), Some(Value::Text(_))));
    idle.stop().expect("stop idle engine");
}

#[test]
fn a_failed_direct_connect_falls_back_to_the_configured_relay() {
    let root = TempRoot::new("relay-init");
    let target_root = TempRoot::new("relay-target");
    let relay = Relay::start();

    let initiator = start(&root.path);
    let target = start(&target_root.path);
    success(relay_add(&initiator, &relay.address));
    success(relay_add(&target, &relay.address));
    wait_for_reservation(&initiator);
    wait_for_reservation(&target);

    let target_peer = peer_id(&target);
    let connect_op = connect(&initiator, &broken_address(&target_peer));
    let completed = wait_for_event(&initiator, "network.connect_completed", Duration::from_secs(20));
    assert_eq!(completed.op_id, Some(connect_op));
    assert_eq!(text_field(&completed.data, "peer_id"), target_peer);
    assert_eq!(text_field(&completed.data, "path"), "relay");
    assert_eq!(text_field(&completed.data, "relay_peer_id"), relay.peer_id());

    let payload = b"ember stage 11 relayed message";
    let test_op = send_test(&initiator, &target_peer, payload);
    let message = wait_for_event(&initiator, "network.test_message", Duration::from_secs(20));
    assert_eq!(message.op_id, Some(test_op));
    assert_eq!(bytes_field(&message.data, "payload"), payload);

    let report = connectivity(&initiator);
    assert_eq!(text_field(&report, "connectivity"), "relay");
    assert_eq!(unsigned_field(&report, "relays_configured"), 1);
    assert_eq!(unsigned_field(&report, "relays_connected"), 1);
    assert!(unsigned_field(&report, "relay_reservations") >= 1);
    assert!(unsigned_field(&report, "relay_circuits_established") >= 1);
    assert!(unsigned_field(&report, "fallback_attempts") >= 1);
    assert!(unsigned_field(&report, "fallback_successes") >= 1);
    assert_eq!(
        optional_unsigned_field(&report, "relay_data_limit_bytes"),
        Some(DEFAULT_RELAY_MAX_CIRCUIT_BYTES)
    );
    assert!(text_field(&report, "hint").contains("relay"));

    let peers = success(invoke(&initiator, 0x0602, empty_params()));
    let target_peer_value = match map_get(&peers, "peers") {
        Some(Value::Array(peers)) => peers
            .iter()
            .find(|peer| text_field(peer, "peer_id") == target_peer)
            .expect("the target must be listed as a peer"),
        other => panic!("missing peers array: {other:?}"),
    };
    // DCUtR may promote the relayed circuit to a direct loopback path after
    // the connect event, so the peer snapshot reports the current best path.
    assert_eq!(text_field(target_peer_value, "state"), "connected");
    assert!(matches!(
        text_field(target_peer_value, "path").as_str(),
        "relay" | "direct"
    ));

    let current = status(&initiator);
    assert!(bool_field(&current, "relay_client_enabled"));
    assert!(bool_field(&current, "relay_fallback_enabled"));
    assert!(!bool_field(&current, "relay_server_enabled"));

    initiator.stop().expect("stop initiator");
    target.stop().expect("stop target");
}

#[test]
fn an_unreachable_peer_reports_no_viable_path() {
    let root = TempRoot::new("relay-nopath");
    let stranger_root = TempRoot::new("relay-stranger");
    let relay = Relay::start();

    let initiator = start(&root.path);
    success(relay_add(&initiator, &relay.address));
    wait_for_reservation(&initiator);

    // This node never reserves on the relay, so no circuit can reach it.
    let stranger = start(&stranger_root.path);
    let stranger_peer = peer_id(&stranger);

    let connect_op = connect(&initiator, &broken_address(&stranger_peer));
    let failed = wait_for_event(&initiator, "network.connect_failed", Duration::from_secs(20));
    assert_eq!(failed.op_id, Some(connect_op));
    let error = map_get(&failed.data, "error").expect("connect failure error");
    assert_eq!(text_field(error, "code"), "network_unavailable");
    assert_eq!(
        text_field(error, "message"),
        "No direct or relayed path to the peer is available."
    );

    let current = status(&initiator);
    assert_eq!(
        text_field(&current, "last_error"),
        "no_viable_path",
        "the status must explain the failed path search"
    );
    assert!(unsigned_field(&current, "fallback_attempts") >= 1);
    assert_eq!(unsigned_field(&current, "fallback_successes"), 0);

    initiator.stop().expect("stop initiator");
    stranger.stop().expect("stop stranger");
}
