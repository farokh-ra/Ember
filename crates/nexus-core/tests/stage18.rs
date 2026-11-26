//! Stage 18 integration: local blocking, local abuse reports, safe-file
//! notices, and blocked-source enforcement.
//!
//! These tests run the core offline unless networking is explicitly needed.
//! Blocking is local policy: nothing here is uploaded or published.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
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
        let path = std::env::temp_dir().join(format!("nexus-core-stage18-{label}-{}-{sequence}", std::process::id()));
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
        network_enabled: false,
        dht_mode: nexus_core::DhtMode::Client,
        relay_fallback: false,
        relay_server: false,
        search_provider: false,
        quota_bytes: 1024 * 1024 * 1024,
        enable_test_methods: false,
    }
}

fn start(root: &Path) -> Engine {
    let local_key = LocalEncryptionKey::new([7; 32]);
    let _ = IdentityVault::new(root.to_path_buf())
        .expect("identity vault")
        .create(&local_key);
    let engine = Engine::create(config(root), &[7; 32]).expect("create engine");
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
    let response = invoke_raw(engine, method_id, parameters).expect("method invoke");
    decode_response(&response).expect("response envelope")
}

fn invoke_raw(engine: &Engine, method_id: u32, parameters: Value) -> Result<Vec<u8>, nexus_core::CoreError> {
    let request = Value::Map(vec![
        (Value::Text("v".to_owned()), Value::Unsigned(1)),
        (Value::Text("params".to_owned()), parameters),
    ])
    .to_canonical_bytes()
    .expect("request CBOR");
    engine.invoke(method_id, &request)
}

fn success(response: Response) -> Value {
    match response {
        Response::Success(value) => value,
        Response::Failure(error) => panic!("unexpected failure: {error:?}"),
    }
}

fn failure(response: Response) -> nexus_core::DomainError {
    match response {
        Response::Success(value) => panic!("unexpected success: {value:?}"),
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
    match map_get(value, key) {
        Some(Value::Bool(flag)) => *flag,
        _ => panic!("missing bool field {key}"),
    }
}

fn array_field(value: &Value, key: &str) -> Vec<Value> {
    match map_get(value, key) {
        Some(Value::Array(items)) => items.clone(),
        _ => panic!("missing array field {key}"),
    }
}

struct RecordedEvent {
    data: Value,
}

fn wait_for_event(engine: &Engine, wanted: &str, within: Duration) -> RecordedEvent {
    let deadline = Instant::now() + within;
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let timeout = u32::try_from(remaining.as_millis().min(200)).unwrap_or(200);
        if let Some(bytes) = engine.next_event(timeout).expect("read event") {
            let value = decode(&bytes, CborLimits::new(16, 512, MAX_EVENT_BYTES)).expect("event CBOR");
            let event_type = text_field(&value, "type");
            if event_type == wanted {
                return RecordedEvent {
                    data: map_get(&value, "data").expect("event data").clone(),
                };
            }
        }
    }
    panic!("timed out waiting for {wanted}");
}

#[test]
fn local_blocks_round_trip_and_emit_events() {
    let root = TempRoot::new("blocks");
    let engine = start(&root.path);

    let listed = success(invoke(&engine, 0x0C01, empty_params()));
    assert_eq!(unsigned_field(&listed, "count"), 0);

    let blocked = success(invoke(
        &engine,
        0x0C02,
        params(vec![
            ("peer_id", Value::Text("12D3KooWblockedpeer".to_owned())),
            ("reason", Value::Text("spam".to_owned())),
            ("note", Value::Text("repeat unsolicited messages".to_owned())),
        ]),
    ));
    assert_eq!(text_field(&blocked, "reason"), "spam");
    assert!(bool_field(&blocked, "local_only"));
    let event = wait_for_event(&engine, "security.peer_blocked", Duration::from_secs(2));
    assert_eq!(text_field(&event.data, "peer_id"), "12D3KooWblockedpeer");

    let listed = success(invoke(&engine, 0x0C01, empty_params()));
    assert_eq!(unsigned_field(&listed, "count"), 1);
    assert_eq!(array_field(&listed, "peers").len(), 1);

    success(invoke(
        &engine,
        0x0C02,
        params(vec![
            ("peer_id", Value::Text("12D3KooWblockedpeer".to_owned())),
            ("reason", Value::Text("harassment".to_owned())),
        ]),
    ));
    let listed = success(invoke(&engine, 0x0C01, empty_params()));
    assert_eq!(unsigned_field(&listed, "count"), 1, "re-blocking updates in place");
    assert_eq!(text_field(&array_field(&listed, "peers")[0], "reason"), "harassment");

    let removed = success(invoke(
        &engine,
        0x0C03,
        params(vec![("peer_id", Value::Text("12D3KooWblockedpeer".to_owned()))]),
    ));
    assert!(bool_field(&removed, "removed"));
    wait_for_event(&engine, "security.peer_unblocked", Duration::from_secs(2));

    let removed = success(invoke(
        &engine,
        0x0C03,
        params(vec![("peer_id", Value::Text("12D3KooWblockedpeer".to_owned()))]),
    ));
    assert!(!bool_field(&removed, "removed"));
    assert_eq!(
        unsigned_field(&success(invoke(&engine, 0x0C01, empty_params())), "count"),
        0
    );
}

#[test]
fn blocks_persist_across_restart() {
    let root = TempRoot::new("persist");
    {
        let engine = start(&root.path);
        success(invoke(
            &engine,
            0x0C02,
            params(vec![
                ("peer_id", Value::Text("12D3KooWpersistent".to_owned())),
                ("reason", Value::Text("scam".to_owned())),
            ]),
        ));
        engine.stop().expect("stop");
    }

    let engine = start(&root.path);
    let listed = success(invoke(&engine, 0x0C01, empty_params()));
    assert_eq!(unsigned_field(&listed, "count"), 1);
    assert_eq!(text_field(&array_field(&listed, "peers")[0], "reason"), "scam");
}

#[test]
fn reports_stay_local_and_validate_input() {
    let root = TempRoot::new("reports");
    let engine = start(&root.path);

    let report = success(invoke(
        &engine,
        0x0C04,
        params(vec![
            ("peer_id", Value::Text("12D3KooWreported".to_owned())),
            ("category", Value::Text("harassment".to_owned())),
            ("note", Value::Text("targeted messages".to_owned())),
        ]),
    ));
    assert!(unsigned_field(&report, "report_id") > 0);
    assert_eq!(text_field(&report, "category"), "harassment");
    assert!(bool_field(&report, "local_only"));

    let listed = success(invoke(&engine, 0x0C05, params(vec![("limit", Value::Unsigned(1))])));
    assert_eq!(unsigned_field(&listed, "count"), 1);
    assert_eq!(unsigned_field(&listed, "total"), 1);
    assert!(bool_field(&listed, "local_only"));

    let invalid = invoke_raw(
        &engine,
        0x0C04,
        params(vec![
            ("peer_id", Value::Text("12D3KooWreported".to_owned())),
            ("category", Value::Text("not-a-category".to_owned())),
        ]),
    )
    .expect_err("unknown category must be rejected");
    assert!(matches!(invalid, nexus_core::CoreError::InvalidArgument(_)));

    let oversized = invoke_raw(
        &engine,
        0x0C04,
        params(vec![
            ("peer_id", Value::Text("12D3KooWreported".to_owned())),
            ("note", Value::Text("x".repeat(513))),
        ]),
    )
    .expect_err("oversized note must be rejected");
    assert!(matches!(oversized, nexus_core::CoreError::InvalidArgument(_)));

    let malformed = invoke_raw(
        &engine,
        0x0C02,
        params(vec![("peer_id", Value::Text("peer with spaces".to_owned()))]),
    )
    .expect_err("malformed peer ID must be rejected");
    assert!(matches!(malformed, nexus_core::CoreError::Store(_)));
}

#[test]
fn blocked_peers_are_rejected_as_download_sources() {
    let root = TempRoot::new("sources");
    let engine = start(&root.path);
    success(invoke(
        &engine,
        0x0C02,
        params(vec![("peer_id", Value::Text("12D3KooWsource".to_owned()))]),
    ));

    let content_id = "ab".repeat(32);
    let blocked = failure(invoke(
        &engine,
        0x0801,
        params(vec![
            ("content_id", Value::Text(content_id.clone())),
            ("peer_id", Value::Text("12D3KooWsource".to_owned())),
        ]),
    ));
    assert_eq!(blocked.code, DomainCode::PermissionDenied);

    success(invoke(
        &engine,
        0x0C03,
        params(vec![("peer_id", Value::Text("12D3KooWsource".to_owned()))]),
    ));
    let unblocked = failure(invoke(
        &engine,
        0x0801,
        params(vec![
            ("content_id", Value::Text(content_id)),
            ("peer_id", Value::Text("12D3KooWsource".to_owned())),
        ]),
    ));
    assert_eq!(
        unblocked.code,
        DomainCode::NetworkUnavailable,
        "an allowed source now reaches the network requirement"
    );
}

#[test]
fn malformed_and_oversized_requests_are_rejected_without_panicking() {
    let root = TempRoot::new("malformed");
    let engine = start(&root.path);
    for payload in [
        Vec::new(),
        vec![0xff, 0xff, 0xff, 0xff],
        vec![0x9f; 64],
        vec![0_u8; nexus_core::MAX_REQUEST_BYTES + 1],
    ] {
        engine
            .invoke(0x0101, &payload)
            .expect_err("malformed request must be rejected");
    }

    let unknown = invoke_raw(&engine, 0x7fff, empty_params()).expect_err("unknown method");
    assert!(matches!(unknown, nexus_core::CoreError::Unsupported(0x7fff)));
}

#[test]
fn security_methods_require_a_running_core() {
    let root = TempRoot::new("stopped");
    let engine = Engine::create(config(&root.path), &[7; 32]).expect("create engine");
    for method in [0x0C01, 0x0C02, 0x0C03, 0x0C04, 0x0C05, 0x0C06] {
        let error = invoke_raw(&engine, method, empty_params()).expect_err("core is not running");
        assert!(matches!(error, nexus_core::CoreError::NotRunning));
    }
}

#[test]
fn notices_state_no_execution_and_key_risk() {
    let root = TempRoot::new("notices");
    let engine = start(&root.path);
    let notices = success(invoke(&engine, 0x0C06, empty_params()));
    assert!(!bool_field(&notices, "auto_execute"));
    assert_eq!(text_field(&notices, "previews"), "delegated");
    let ids: Vec<String> = array_field(&notices, "notices")
        .iter()
        .map(|notice| text_field(notice, "id"))
        .collect();
    assert!(ids.contains(&"no_auto_execution".to_owned()));
    assert!(ids.contains(&"key_backup".to_owned()));
    assert!(ids.contains(&"public_visibility".to_owned()));
    assert!(ids.contains(&"local_blocking".to_owned()));

    let caps = success(invoke(&engine, 0x0101, empty_params()));
    let names: Vec<String> = array_field(&caps, "capabilities")
        .iter()
        .filter_map(Value::as_text)
        .map(str::to_owned)
        .collect();
    assert!(names.contains(&"security.block_add".to_owned()));
    assert!(names.contains(&"security.report_list".to_owned()));
}
