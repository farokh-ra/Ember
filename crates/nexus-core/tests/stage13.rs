//! Stage 13 integration: two real engines publish signed profiles through the
//! DHT, verify and cache each other's records, surface username-label
//! collisions, replace a cached revision with a newer one, and serve a stale
//! cache when the publisher is unreachable.
//!
//! The publisher and a storage peer are DHT servers; the reader is a client
//! bootstrapped to both over loopback. The reader never stores DHT records
//! itself, so once both servers stop it can only serve its local cache and
//! must label that cache stale. Usernames are deliberately spelled with
//! different case to prove collisions are detected case-insensitively.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::sleep;
use std::time::{Duration, Instant};

use nexus_core::{CoreConfig, DomainCode, Engine, LogLevel, MAX_EVENT_BYTES, Response, decode_response};
use nexus_crypto::{CborLimits, Value, decode};
use nexus_identity::{IdentityVault, LocalEncryptionKey};

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(label: &str) -> Self {
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("nexus-core-stage13-{label}-{}-{sequence}", std::process::id()));
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
    wait_for_routing_peers(engine, 1);
}

fn wait_for_routing_peers(engine: &Engine, expected: u64) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let current = status(engine);
        if unsigned_field(&current, "dht_routing_peers") >= expected && bool_field(&current, "dht_bootstrapped") {
            return;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for {expected} DHT peers: {current:?}");
        }
        sleep(Duration::from_millis(50));
    }
}

fn identity_id(engine: &Engine) -> String {
    let local = success(invoke(engine, 0x0301, empty_params()));
    text_field(&local, "identity_id")
}

fn publish(engine: &Engine, entries: Vec<(&str, Value)>) -> Value {
    success(invoke(engine, 0x0302, params(entries)))
}

fn lookup(engine: &Engine, identity_id: &str) -> u64 {
    let response = success(invoke(
        engine,
        0x0303,
        params(vec![("identity_id", Value::Text(identity_id.to_owned()))]),
    ));
    unsigned_field(&response, "op_id")
}

fn lookup_record(event: &RecordedEvent) -> &Value {
    map_get(&event.data, "record").unwrap_or_else(|| panic!("lookup event has no record: {:?}", event.data))
}

fn collision_ids(event: &RecordedEvent) -> Vec<String> {
    array_field(&event.data, "username_collisions")
        .iter()
        .map(|collision| text_field(collision, "identity_id"))
        .collect()
}

#[test]
fn engines_publish_verify_supersede_and_label_stale_profiles() {
    let publisher_root = TempRoot::new("publisher");
    let storage_root = TempRoot::new("storage");
    let reader_root = TempRoot::new("reader");

    let publisher = start(&publisher_root.path, 51, true);
    let storage = start(&storage_root.path, 53, true);
    let reader = start(&reader_root.path, 52, false);
    wait_for_ready(&publisher);
    wait_for_ready(&storage);
    wait_for_ready(&reader);

    let publisher_address = dial_address(&publisher);
    bootstrap(&storage, &publisher_address);
    wait_for_dht_peer(&storage);
    let storage_address = dial_address(&storage);
    bootstrap(&reader, &publisher_address);
    bootstrap(&reader, &storage_address);
    wait_for_routing_peers(&reader, 2);

    let publisher_id = identity_id(&publisher);
    let reader_id = identity_id(&reader);
    assert_ne!(publisher_id, reader_id);

    // The publisher signs and announces revision 1.
    let accepted = publish(
        &publisher,
        vec![
            ("display_name", Value::Text("Ada Stage 13".to_owned())),
            ("username", Value::Text("ada".to_owned())),
            ("bio", Value::Text("signed profile publisher".to_owned())),
        ],
    );
    assert_eq!(unsigned_field(&accepted, "revision"), 1);
    assert_eq!(text_field(&accepted, "state"), "pending");
    let published = wait_for_event(&publisher, "profile.published", Duration::from_secs(30));
    assert_eq!(published.op_id, Some(unsigned_field(&accepted, "op_id")));
    assert!(bool_field(&published.data, "discoverable"), "{:?}", published.data);
    assert_eq!(text_field(&published.data, "announcement"), "published");
    assert!(bool_field(&published.data, "record_verified"));

    // The reader saves a local draft first. Usernames are labels, so the same
    // label with different case is allowed on both identities.
    let draft = publish(
        &reader,
        vec![
            ("display_name", Value::Text("Beta Stage 13".to_owned())),
            ("username", Value::Text("ADA".to_owned())),
            ("publish", Value::Bool(false)),
        ],
    );
    assert_eq!(unsigned_field(&draft, "revision"), 1);
    assert_eq!(text_field(&draft, "state"), "draft");
    let local = success(invoke(&reader, 0x0301, empty_params()));
    assert!(bool_field(&local, "has_profile"));
    let local_profile = map_get(&local, "profile").expect("local profile");
    assert_eq!(text_field(local_profile, "state"), "draft");
    assert!(!bool_field(local_profile, "discoverable"));
    assert_eq!(text_field(local_profile, "username"), "ADA");

    // A stale expected revision is rejected without changing the draft.
    match invoke(
        &reader,
        0x0302,
        params(vec![
            ("display_name", Value::Text("Beta Changed".to_owned())),
            ("expected_revision", Value::Unsigned(0)),
        ]),
    ) {
        Response::Failure(error) => assert_eq!(error.code, DomainCode::Conflict),
        Response::Success(value) => panic!("stale expected revision must conflict: {value:?}"),
    }
    let local = success(invoke(&reader, 0x0301, empty_params()));
    let local_profile = map_get(&local, "profile").expect("local profile");
    assert_eq!(unsigned_field(local_profile, "revision"), 1);

    // Publishing without new fields republishes the exact signed draft and
    // does not increment its revision.
    let republished = publish(&reader, vec![("publish", Value::Bool(true))]);
    assert_eq!(unsigned_field(&republished, "revision"), 1);
    let published = wait_for_event(&reader, "profile.published", Duration::from_secs(30));
    assert_eq!(published.op_id, Some(unsigned_field(&republished, "op_id")));
    assert!(bool_field(&published.data, "discoverable"));
    assert_eq!(unsigned_field(&published.data, "revision"), 1);

    // The reader retrieves the publisher's signed record and sees the reader's
    // own profile as a case-insensitive username collision.
    let lookup_op = lookup(&reader, &publisher_id);
    let completed = wait_for_event(&reader, "profile.lookup_completed", Duration::from_secs(30));
    assert_eq!(completed.op_id, Some(lookup_op));
    assert!(bool_field(&completed.data, "found"));
    assert_eq!(text_field(&completed.data, "source"), "network");
    assert_eq!(text_field(&completed.data, "freshness"), "current");
    assert!(!bool_field(&completed.data, "stale"));
    assert!(bool_field(&completed.data, "username_is_label"));
    let record = lookup_record(&completed);
    assert_eq!(text_field(record, "identity_id"), publisher_id);
    assert_eq!(unsigned_field(record, "revision"), 1);
    assert_eq!(text_field(record, "display_name"), "Ada Stage 13");
    assert_eq!(text_field(record, "username"), "ada");
    assert_eq!(text_field(record, "state"), "remote");
    assert!(bool_field(record, "record_verified"));
    assert_eq!(collision_ids(&completed), vec![reader_id.clone()]);

    // The publisher retrieves the reader's record and sees its own profile as
    // the collision, proving the label is not reserved to either identity.
    let lookup_op = lookup(&publisher, &reader_id);
    let completed = wait_for_event(&publisher, "profile.lookup_completed", Duration::from_secs(30));
    assert_eq!(completed.op_id, Some(lookup_op));
    assert_eq!(text_field(&completed.data, "source"), "network");
    let record = lookup_record(&completed);
    assert_eq!(text_field(record, "identity_id"), reader_id);
    assert_eq!(text_field(record, "username"), "ADA");
    assert_eq!(collision_ids(&completed), vec![publisher_id.clone()]);

    // A newer revision supersedes the cached one.
    let updated = publish(
        &publisher,
        vec![("display_name", Value::Text("Ada Stage 13 v2".to_owned()))],
    );
    assert_eq!(unsigned_field(&updated, "revision"), 2);
    let published = wait_for_event(&publisher, "profile.published", Duration::from_secs(30));
    assert_eq!(published.op_id, Some(unsigned_field(&updated, "op_id")));
    assert!(bool_field(&published.data, "discoverable"));

    let lookup_op = lookup(&reader, &publisher_id);
    let completed = wait_for_event(&reader, "profile.lookup_completed", Duration::from_secs(30));
    assert_eq!(completed.op_id, Some(lookup_op));
    let record = lookup_record(&completed);
    assert_eq!(unsigned_field(record, "revision"), 2);
    assert_eq!(text_field(record, "display_name"), "Ada Stage 13 v2");
    assert_eq!(text_field(record, "freshness"), "current");

    // With both servers offline, the reader can only serve its cache and must
    // label it stale instead of pretending the lookup succeeded.
    publisher.stop().expect("stop publisher");
    storage.stop().expect("stop storage");
    let lookup_op = lookup(&reader, &publisher_id);
    let completed = wait_for_event(&reader, "profile.lookup_completed", Duration::from_secs(60));
    assert_eq!(completed.op_id, Some(lookup_op));
    assert!(bool_field(&completed.data, "found"));
    assert_eq!(text_field(&completed.data, "source"), "cache");
    assert!(bool_field(&completed.data, "stale"));
    assert_eq!(text_field(&completed.data, "freshness"), "stale");
    let record = lookup_record(&completed);
    assert_eq!(unsigned_field(record, "revision"), 2);
    assert!(bool_field(record, "stale"));

    reader.stop().expect("stop reader");
}
