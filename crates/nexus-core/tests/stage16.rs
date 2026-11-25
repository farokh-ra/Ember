//! Stage 16 integration: verified local search, Explore, honest coverage,
//! generation-bound pagination, offline/provider-loss behaviour, and optional
//! distributed index providers.
//!
//! Local search reads only the derived index built from verified records.
//! Provider results are re-verified by the core before they reach an event.

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
        let path = std::env::temp_dir().join(format!("nexus-core-stage16-{label}-{}-{sequence}", std::process::id()));
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

fn config(root: &Path, network: bool, search_provider: bool, server: bool) -> CoreConfig {
    CoreConfig {
        data_root: root.to_path_buf(),
        app_version: "0.1.0-test".to_owned(),
        locale: "en".to_owned(),
        log_level: LogLevel::Info,
        network_enabled: network,
        dht_mode: if server {
            nexus_core::DhtMode::Server
        } else {
            nexus_core::DhtMode::Client
        },
        relay_fallback: false,
        relay_server: false,
        search_provider,
        quota_bytes: 1024 * 1024 * 1024,
        enable_test_methods: false,
    }
}

fn start(root: &Path, key: u8, network: bool, search_provider: bool, server: bool) -> Engine {
    let local_key = LocalEncryptionKey::new([key; 32]);
    IdentityVault::new(root.to_path_buf())
        .expect("identity vault")
        .create(&local_key)
        .expect("create identity");
    let engine = Engine::create(config(root, network, search_provider, server), &[key; 32]).expect("create engine");
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

fn coverage(value: &Value) -> &Value {
    map_get(value, "coverage").unwrap_or_else(|| panic!("missing coverage: {value:?}"))
}

fn has_note(coverage: &Value, note: &str) -> bool {
    array_field(coverage, "notes")
        .iter()
        .filter_map(Value::as_text)
        .any(|candidate| candidate == note)
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

fn wait_for_search(engine: &Engine, op_id: u64, within: Duration) -> (Vec<Value>, Value) {
    let deadline = Instant::now() + within;
    let mut pages = Vec::new();
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let timeout = u32::try_from(remaining.as_millis().min(200)).unwrap_or(200);
        if let Some(bytes) = engine.next_event(timeout).expect("read event") {
            let event = decode_event(&bytes);
            if event.op_id != Some(op_id) {
                continue;
            }
            match event.event_type.as_str() {
                "search.result_page" => pages.push(event.data),
                "search.completed" => return (pages, event.data),
                _ => {}
            }
        }
    }
    panic!("timed out waiting for search {op_id}");
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

fn wait_for_online(engine: &Engine) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let current = status(engine);
        if text_field(&current, "state") == "online" {
            return;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for an online network: {current:?}");
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

fn wait_for_publication(engine: &Engine, op_id: u64, publication_id: &str) -> RecordedEvent {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let timeout = u32::try_from(remaining.as_millis().min(200)).unwrap_or(200);
        if let Some(bytes) = engine.next_event(timeout).expect("read event") {
            let event = decode_event(&bytes);
            if event.op_id != Some(op_id) {
                continue;
            }
            match event.event_type.as_str() {
                "publication.published" => {
                    assert_eq!(
                        map_get(&event.data, "publication_id").and_then(Value::as_text),
                        Some(publication_id)
                    );
                    return event;
                }
                other if other.ends_with("_failed") || other.ends_with(".failed") => {
                    panic!("publication failed: {other}: {:?}", event.data);
                }
                _ => {}
            }
        }
    }
    panic!("timed out waiting for publication {publication_id}");
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

fn publish(engine: &Engine, filename: &str, description: &str, category: &str, tags: &[&str]) -> String {
    let content = format!("Stage 16 search fixture: {filename}\n{description}\nend of fixture\n");
    let content_id = import_file(engine, filename, content.as_bytes());
    let created = success(invoke(
        engine,
        0x0501,
        params(vec![
            ("content_id", Value::Text(content_id)),
            ("display_filename", Value::Text(filename.to_owned())),
            ("description", Value::Text(description.to_owned())),
            ("category", Value::Text(category.to_owned())),
            (
                "tags",
                Value::Array(tags.iter().map(|tag| Value::Text((*tag).to_owned())).collect()),
            ),
        ]),
    ));
    let publication_id = text_field(&created, "publication_id");
    let op_id = unsigned_field(&created, "op_id");
    wait_for_publication(engine, op_id, &publication_id);
    publication_id
}

fn search(engine: &Engine, query: &str, extra: Vec<(&str, Value)>) -> Value {
    let mut entries = vec![("query", Value::Text(query.to_owned()))];
    entries.extend(extra);
    success(invoke(engine, 0x0702, params(entries)))
}

fn result_id(item: &Value) -> String {
    text_field(item, "record_id")
}

fn publish_fixtures(engine: &Engine) {
    publish(
        engine,
        "field-note-alpha.txt",
        "A field note about alpha and careful observation",
        "documents",
        &["stage16", "notes"],
    );
    publish(
        engine,
        "field-note-beta.txt",
        "A field note about beta and bounded queries",
        "documents",
        &["stage16", "notes"],
    );
    publish(
        engine,
        "field-guide-gamma.txt",
        "A field guide to gamma",
        "documents",
        &["stage16"],
    );
    publish(
        engine,
        "field-archive-delta.zip",
        "A field archive of delta",
        "archives",
        &["stage16"],
    );
    publish(
        engine,
        "coast-image.png",
        "Coastline at dusk",
        "images",
        &["stage16", "coast"],
    );
}

#[test]
fn local_search_filters_paginates_and_labels_coverage_honestly() {
    let root = TempRoot::new("local");
    let engine = start(&root.path, 41, false, false, false);
    publish_fixtures(&engine);

    let info = success(invoke(&engine, 0x0101, empty_params()));
    let capabilities = array_field(&info, "capabilities");
    assert!(capabilities.iter().any(|name| name.as_text() == Some("search.query")));
    assert!(
        capabilities
            .iter()
            .any(|name| name.as_text() == Some("discovery.explore"))
    );

    let first = search(&engine, "field", vec![("limit", Value::Unsigned(2))]);
    assert_eq!(unsigned_field(&first, "total_matches"), 4);
    assert_eq!(array_field(&first, "results").len(), 2);
    assert_eq!(text_field(&first, "sort"), "relevance");
    let first_page = coverage(&first);
    assert_eq!(text_field(first_page, "label"), "local_only");
    assert!(!bool_field(first_page, "may_be_incomplete"));
    assert!(unsigned_field(first_page, "indexed_at_ms") > 0);
    assert_eq!(unsigned_field(first_page, "deadline_ms"), 10_000);
    let first_cursor = text_field(&first, "next_cursor");
    let first_op = unsigned_field(&first, "op_id");

    let (pages, completed) = wait_for_search(&engine, first_op, Duration::from_secs(10));
    assert!(pages.is_empty());
    assert_eq!(text_field(&completed, "query_hash"), text_field(&first, "query_hash"));
    assert_eq!(unsigned_field(&completed, "new_results"), 0);
    assert_eq!(unsigned_field(&completed, "conflict_count"), 0);
    assert!(!bool_field(&completed, "cancelled"));
    assert_eq!(text_field(coverage(&completed), "label"), "local_only");

    let second = success(invoke(
        &engine,
        0x0702,
        params(vec![
            ("query", Value::Text("field".to_owned())),
            ("limit", Value::Unsigned(2)),
            ("cursor", Value::Text(first_cursor.clone())),
        ]),
    ));
    assert_eq!(array_field(&second, "results").len(), 2);
    let first_ids: Vec<String> = array_field(&first, "results").iter().map(result_id).collect();
    let second_ids: Vec<String> = array_field(&second, "results").iter().map(result_id).collect();
    assert!(first_ids.iter().all(|id| !second_ids.contains(id)));

    let by_category = search(&engine, "coast", vec![("category", Value::Text("images".to_owned()))]);
    assert_eq!(array_field(&by_category, "results").len(), 1);
    assert_eq!(
        text_field(&array_field(&by_category, "results")[0], "title"),
        "coast-image.png"
    );

    let by_tags = search(
        &engine,
        "field",
        vec![("tags", Value::Array(vec![Value::Text("notes".to_owned())]))],
    );
    assert_eq!(unsigned_field(&by_tags, "total_matches"), 2);

    let profiles_only = search(&engine, "field", vec![("kind", Value::Text("profile".to_owned()))]);
    assert_eq!(array_field(&profiles_only, "results").len(), 0);

    let mismatch = invoke(
        &engine,
        0x0702,
        params(vec![
            ("query", Value::Text("coast".to_owned())),
            ("cursor", Value::Text(first_cursor.clone())),
        ]),
    );
    assert_eq!(failure_code(mismatch), DomainCode::Conflict);

    publish(
        &engine,
        "field-note-epsilon.txt",
        "A field note published after the first page",
        "documents",
        &["stage16"],
    );
    let stale = invoke(
        &engine,
        0x0702,
        params(vec![
            ("query", Value::Text("field".to_owned())),
            ("limit", Value::Unsigned(2)),
            ("cursor", Value::Text(first_cursor)),
        ]),
    );
    assert_eq!(failure_code(stale), DomainCode::Conflict);

    let oversized = invoke(&engine, 0x0702, params(vec![("query", Value::Text("x".repeat(129)))]));
    assert_eq!(failure_code(oversized), DomainCode::InvalidArgument);

    let unknown_cancel = success(invoke(
        &engine,
        0x0703,
        params(vec![("op_id", Value::Unsigned(999_999))]),
    ));
    assert_eq!(text_field(&unknown_cancel, "outcome"), "unknown");
    assert!(!bool_field(&unknown_cancel, "cancelled"));

    let terminal_cancel = success(invoke(
        &engine,
        0x0703,
        params(vec![("op_id", Value::Unsigned(first_op))]),
    ));
    assert_eq!(text_field(&terminal_cancel, "outcome"), "already_terminal");
}

#[test]
fn explore_categories_recent_and_random_are_local_and_bounded() {
    let root = TempRoot::new("explore");
    let engine = start(&root.path, 42, false, false, false);
    publish_fixtures(&engine);

    let categories = success(invoke(&engine, 0x0701, empty_params()));
    assert_eq!(text_field(&categories, "mode"), "categories");
    assert_eq!(text_field(coverage(&categories), "label"), "local_only");
    let counts = array_field(&categories, "categories");
    let documents = counts
        .iter()
        .find(|entry| text_field(entry, "category") == "documents")
        .expect("documents count");
    assert_eq!(unsigned_field(documents, "count"), 3);
    let images = counts
        .iter()
        .find(|entry| text_field(entry, "category") == "images")
        .expect("images count");
    assert_eq!(unsigned_field(images, "count"), 1);
    assert!(array_field(&categories, "results").is_empty());

    let recent = success(invoke(
        &engine,
        0x0701,
        params(vec![
            ("mode", Value::Text("recent".to_owned())),
            ("limit", Value::Unsigned(2)),
        ]),
    ));
    assert_eq!(array_field(&recent, "results").len(), 2);
    let seed = unsigned_field(&recent, "seed");
    let cursor = text_field(&recent, "next_cursor");
    let recent_page_two = success(invoke(
        &engine,
        0x0701,
        params(vec![
            ("mode", Value::Text("recent".to_owned())),
            ("limit", Value::Unsigned(2)),
            ("seed", Value::Unsigned(seed)),
            ("cursor", Value::Text(cursor)),
        ]),
    ));
    let page_one_ids: Vec<String> = array_field(&recent, "results").iter().map(result_id).collect();
    let page_two_ids: Vec<String> = array_field(&recent_page_two, "results").iter().map(result_id).collect();
    assert!(page_one_ids.iter().all(|id| !page_two_ids.contains(id)));

    let random = success(invoke(
        &engine,
        0x0701,
        params(vec![
            ("mode", Value::Text("random".to_owned())),
            ("limit", Value::Unsigned(10)),
            ("seed", Value::Unsigned(7)),
        ]),
    ));
    let random_again = success(invoke(
        &engine,
        0x0701,
        params(vec![
            ("mode", Value::Text("random".to_owned())),
            ("limit", Value::Unsigned(10)),
            ("seed", Value::Unsigned(7)),
        ]),
    ));
    let random_ids: Vec<String> = array_field(&random, "results").iter().map(result_id).collect();
    let random_again_ids: Vec<String> = array_field(&random_again, "results").iter().map(result_id).collect();
    assert_eq!(random_ids, random_again_ids);
    assert_eq!(random_ids.len(), 5);
    let mut sorted = random_ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), 5);
}

#[test]
fn provider_loss_keeps_local_results_and_reports_reduced_coverage() {
    let provider_root = TempRoot::new("loss-provider");
    let requester_root = TempRoot::new("loss-requester");
    let provider = start(&provider_root.path, 43, true, false, true);
    let requester = start(&requester_root.path, 44, true, false, true);
    wait_for_ready(&provider);
    wait_for_ready(&requester);
    bootstrap(&requester, &dial_address(&provider));
    wait_for_online(&requester);
    wait_for_dht_peer(&requester);

    publish(
        &requester,
        "provider-loss-local.txt",
        "A local report that must survive provider loss",
        "documents",
        &["stage16"],
    );
    let response = search(&requester, "provider loss", vec![("limit", Value::Unsigned(5))]);
    assert!(!array_field(&response, "results").is_empty());
    assert!(bool_field(&response, "providers_pending"));
    let op_id = unsigned_field(&response, "op_id");
    let (pages, completed) = wait_for_search(&requester, op_id, Duration::from_secs(20));
    assert!(pages.is_empty());
    let terminal = coverage(&completed);
    assert_eq!(text_field(terminal, "label"), "local_only");
    assert!(!bool_field(terminal, "distributed"));
    assert!(has_note(terminal, "no_index_providers"));
    assert_eq!(unsigned_field(terminal, "providers_queried"), 0);
    assert!(!bool_field(&completed, "cancelled"));
}

#[test]
fn distributed_provider_results_are_verified_and_bounded() {
    let provider_root = TempRoot::new("distributed-provider");
    let requester_root = TempRoot::new("distributed-requester");
    let provider = start(&provider_root.path, 45, true, true, true);
    let requester = start(&requester_root.path, 46, true, false, true);
    wait_for_ready(&provider);
    wait_for_ready(&requester);
    bootstrap(&requester, &dial_address(&provider));
    wait_for_online(&requester);
    wait_for_dht_peer(&requester);

    publish(
        &provider,
        "distributed-zephyr-report.txt",
        "A distributed zephyr field report",
        "documents",
        &["stage16", "zephyr"],
    );

    let deadline = Instant::now() + Duration::from_secs(45);
    let mut found = None;
    let mut terminal_coverage = None;
    while Instant::now() < deadline {
        let response = search(&requester, "zephyr", vec![("limit", Value::Unsigned(5))]);
        assert!(array_field(&response, "results").is_empty());
        let op_id = unsigned_field(&response, "op_id");
        let (pages, completed) = wait_for_search(&requester, op_id, Duration::from_secs(20));
        terminal_coverage = Some(coverage(&completed).clone());
        for page in pages {
            for item in array_field(&page, "items") {
                if text_field(item, "title").contains("zephyr") {
                    found = Some(item.clone());
                }
            }
        }
        if found.is_some() {
            break;
        }
        sleep(Duration::from_secs(2));
    }
    let item = found.unwrap_or_else(|| panic!("no provider result before deadline: {terminal_coverage:?}"));
    assert_eq!(text_field(&item, "kind"), "publication");
    assert_eq!(text_field(&item, "source"), "provider");
    assert!(bool_field(&item, "verified"));
    assert!(!text_field(&item, "record_id").is_empty());
    let terminal = terminal_coverage.expect("terminal coverage");
    assert!(bool_field(&terminal, "distributed"));
    assert!(unsigned_field(&terminal, "providers_queried") >= 1);
    assert!(unsigned_field(&terminal, "providers_succeeded") >= 1);
    assert!(matches!(
        text_field(&terminal, "label").as_str(),
        "complete" | "partial"
    ));
}
