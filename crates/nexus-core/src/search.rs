//! Verified local search, Explore, and optional index-provider fan-out.
//!
//! Local results come from the derived FTS5 index in `nexus-store` and are
//! returned before any network work starts. Optional providers are discovered
//! through the DHT, contacted under a bounded deadline, and every returned
//! record is re-verified before it can reach an event or the UI. Provider
//! records are never written to local storage from a search response.
//!
//! The local index is authoritative for local results. Providers only add
//! signed records that the caller did not already have, and conflicts stay
//! visible instead of being silently resolved.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use nexus_crypto::{Value, to_hex};
use nexus_net::{DiscoveredPeer, IndexProvider, NetError, Network, PeerState};
use nexus_protocol::{
    IndexKind, IndexRecord, IndexRequest, IndexResponse, SignedProfile, SignedPublication, index_provider_key,
};
use nexus_search::{
    Availability, Conflict, Coverage, DEFAULT_PAGE_SIZE, ExploreMode, ExplorePage, ExploreQuery, MAX_PAGE_SIZE,
    MAX_PROVIDERS, MAX_RESULT_SUMMARY_BYTES, MAX_RESULT_TITLE_BYTES, MAX_TAG_FILTERS, NormalizedQuery,
    ProviderCoverage, RecordKind, ResultAccumulator, ResultSource, SEARCH_DEADLINE_MS, SearchError, SearchItem,
    SearchQuery, SortOrder, SourceState, item_matches, sanitize_text,
};
use nexus_store::{LocalSearchPage, Store, StoreError};

use crate::engine::{Engine, MethodOutcome};
use crate::error::{CoreError, DomainCode, DomainError};
use crate::event::{EventQueue, PendingEvent};
use crate::ops::{CancelOutcome, CancelToken, OperationRegistry};

const SEARCH_DEADLINE: Duration = Duration::from_millis(SEARCH_DEADLINE_MS);
const SEARCH_POLL: Duration = Duration::from_millis(50);

impl Engine {
    /// Runs one local Explore query. Explore is local-only in v1.
    pub(crate) fn discovery_explore(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let query = parse_explore_query(params)?;
        let page = match self.store.explore_local(&query) {
            Ok(page) => page,
            Err(error) => return Ok(MethodOutcome::Domain(DomainError::from_store(&error))),
        };
        Ok(MethodOutcome::Result(explore_value(&page)))
    }

    /// Runs one local search and, on the first page, starts a bounded provider
    /// fan-out. Local results are returned immediately. Every call emits one
    /// terminal `search.completed` event; provider additions stream as
    /// `search.result_page` events carrying the same operation ID.
    pub(crate) fn search_query(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let query = parse_search_query(params)?;
        let normalized = match query.validate() {
            Ok(normalized) => normalized,
            Err(error) => return Ok(MethodOutcome::Domain(search_domain_error(error))),
        };
        let local = match self.store.search_local(&query) {
            Ok(page) => page,
            Err(error) => return Ok(MethodOutcome::Domain(DomainError::from_store(&error))),
        };
        let (op_id, token) = self.ops.begin();
        let network = self.network_snapshot().filter(|_| query.cursor.is_none());
        let distributed = network
            .as_ref()
            .is_some_and(|network| network.status().state == nexus_net::NetworkState::Online);
        if let Some(network) = network.filter(|_| distributed) {
            let context = SearchContext {
                store: Arc::clone(&self.store),
                events: Arc::clone(&self.events),
                network,
                indexed_at_ms: local.coverage.indexed_at_ms,
            };
            let ops = Arc::clone(&self.ops);
            let worker_query = query.clone();
            let worker_normalized = normalized.clone();
            if let Err(error) = self.pool.submit(move || {
                run_search_fanout(&context, &ops, op_id, token, worker_query, worker_normalized);
            }) {
                self.ops.finish(op_id);
                return Ok(MethodOutcome::Domain(DomainError::from_core(&error)));
            }
            return Ok(MethodOutcome::Result(search_value(
                op_id,
                &query,
                &normalized,
                &local,
                true,
            )));
        }
        emit_search_completed(
            &self.events,
            op_id,
            &normalized.query_hash,
            &local.coverage,
            0,
            0,
            0,
            0,
            false,
            Instant::now(),
        );
        self.ops.finish(op_id);
        Ok(MethodOutcome::Result(search_value(
            op_id,
            &query,
            &normalized,
            &local,
            false,
        )))
    }

    /// Requests cancellation of one running search operation.
    pub(crate) fn search_cancel(&self, params: &Value) -> Result<MethodOutcome, CoreError> {
        self.require_running()?;
        let op_id = map_get(params, "op_id")
            .and_then(Value::as_unsigned)
            .ok_or(CoreError::InvalidArgument("op_id is required"))?;
        let outcome = self.ops.cancel(op_id);
        let (name, cancelled) = match outcome {
            CancelOutcome::Requested => ("requested", true),
            CancelOutcome::AlreadyTerminal => ("already_terminal", false),
            CancelOutcome::Unknown => ("unknown", false),
        };
        Ok(MethodOutcome::Result(Value::Map(vec![
            (Value::Text("op_id".to_owned()), Value::Unsigned(op_id)),
            (Value::Text("outcome".to_owned()), Value::Text(name.to_owned())),
            (Value::Text("cancelled".to_owned()), Value::Bool(cancelled)),
        ])))
    }
}

/// Inbound `/nexus/index/1` source backed by the verified local index.
pub(crate) struct CoreIndexProvider {
    store: Arc<Store>,
}

impl CoreIndexProvider {
    pub(crate) fn new(store: Arc<Store>) -> Self {
        Self { store }
    }
}

impl IndexProvider for CoreIndexProvider {
    fn respond(&self, request: &IndexRequest) -> IndexResponse {
        let query = SearchQuery {
            text: request.query.clone(),
            kind: request.kind.map(record_kind),
            category: request.category,
            tags: request.tags.clone(),
            sort: SortOrder::Relevance,
            limit: request.limit,
            cursor: request.cursor.clone(),
        };
        match self.store.provider_search(&query) {
            Ok(page) => IndexResponse {
                records: page
                    .records
                    .into_iter()
                    .map(|(kind, signed)| IndexRecord {
                        kind: index_kind(kind),
                        signed,
                    })
                    .collect(),
                next_cursor: page.next_cursor,
                complete: page.complete,
                error: None,
                indexed_at_ms: self.store.search_index_stats().ok().map(|stats| stats.indexed_at_ms),
            },
            Err(StoreError::InvalidSearch(_)) => IndexResponse {
                error: Some("invalid_request".to_owned()),
                ..IndexResponse::default()
            },
            Err(_) => IndexResponse {
                error: Some("unavailable".to_owned()),
                ..IndexResponse::default()
            },
        }
    }
}

struct SearchContext {
    store: Arc<Store>,
    events: Arc<EventQueue>,
    network: Arc<Network>,
    indexed_at_ms: Option<u64>,
}

enum Discovery {
    Found(Vec<DiscoveredPeer>),
    Empty,
    Failed(&'static str),
    TimedOut,
    Cancelled,
}

fn run_search_fanout(
    context: &SearchContext,
    ops: &OperationRegistry,
    op_id: u64,
    token: Arc<CancelToken>,
    query: SearchQuery,
    normalized: NormalizedQuery,
) {
    let started = Instant::now();
    let deadline = started + SEARCH_DEADLINE;
    let indexed_at = context.indexed_at_ms;
    let mut coverage = Coverage::local_only(indexed_at)
        .with_deadline(SEARCH_DEADLINE_MS)
        .pending();

    let providers = match discover_providers(&context.network, &token, deadline) {
        Discovery::Found(providers) => providers,
        Discovery::Empty => {
            let coverage = Coverage::local_only(indexed_at)
                .with_deadline(SEARCH_DEADLINE_MS)
                .note("no_index_providers")
                .finish();
            emit_search_completed(
                &context.events,
                op_id,
                &normalized.query_hash,
                &coverage,
                0,
                0,
                0,
                0,
                false,
                started,
            );
            ops.finish(op_id);
            return;
        }
        Discovery::Failed(reason) => {
            let coverage = Coverage::local_only(indexed_at)
                .with_deadline(SEARCH_DEADLINE_MS)
                .note(reason)
                .finish();
            emit_search_completed(
                &context.events,
                op_id,
                &normalized.query_hash,
                &coverage,
                0,
                0,
                0,
                0,
                false,
                started,
            );
            ops.finish(op_id);
            return;
        }
        Discovery::TimedOut => {
            let coverage = Coverage::local_only(indexed_at)
                .with_deadline(SEARCH_DEADLINE_MS)
                .note("provider_discovery_timeout")
                .finish();
            emit_search_completed(
                &context.events,
                op_id,
                &normalized.query_hash,
                &coverage,
                0,
                0,
                0,
                0,
                false,
                started,
            );
            ops.finish(op_id);
            return;
        }
        Discovery::Cancelled => {
            let coverage = Coverage::local_only(indexed_at)
                .with_deadline(SEARCH_DEADLINE_MS)
                .note("search_cancelled")
                .finish();
            emit_search_completed(
                &context.events,
                op_id,
                &normalized.query_hash,
                &coverage,
                0,
                0,
                0,
                0,
                true,
                started,
            );
            ops.finish(op_id);
            return;
        }
    };

    let request = provider_request(&query);
    let (sender, receiver) = mpsc::channel();
    let mut outstanding: HashSet<String> = HashSet::new();
    for provider in providers {
        let peer_id = provider.peer_id.clone();
        if spawn_provider_fetch(Arc::clone(&context.network), provider, request.clone(), sender.clone()) {
            outstanding.insert(peer_id);
        } else {
            coverage.record_provider(
                ProviderCoverage::new(&peer_id, SourceState::Unavailable).with_detail("thread_unavailable"),
            );
        }
    }
    drop(sender);

    let mut processor = ProviderProcessor::new(&context.store, &query, &normalized);
    let mut total_new = 0_u32;
    let mut total_rejected = 0_u32;
    let mut total_ignored = 0_u32;
    let mut total_conflicts = 0_u32;
    let mut cancelled = false;

    while !outstanding.is_empty() {
        if token.is_cancelled() {
            cancelled = true;
            break;
        }
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        let wait = SEARCH_POLL.min(deadline.saturating_duration_since(now));
        match receiver.recv_timeout(wait) {
            Ok(fetch) => {
                outstanding.remove(&fetch.peer_id);
                let processed = processor.process(&fetch.peer_id, fetch.result);
                coverage.record_provider(processed.coverage);
                total_new = total_new.saturating_add(u32::try_from(processed.items.len()).unwrap_or(u32::MAX));
                total_rejected = total_rejected.saturating_add(processed.rejected);
                total_ignored = total_ignored.saturating_add(processed.ignored);
                total_conflicts = total_conflicts.saturating_add(processed.conflicts);
                if !processed.items.is_empty() || !processed.conflict_updates.is_empty() {
                    emit_result_page(
                        &context.events,
                        op_id,
                        &normalized.query_hash,
                        &fetch.peer_id,
                        &processed.items,
                        &processed.conflict_updates,
                        &coverage,
                    );
                }
            }
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    for peer_id in outstanding.drain() {
        let state = if cancelled {
            SourceState::Unavailable
        } else {
            SourceState::Timeout
        };
        let detail = if cancelled { "cancelled" } else { "deadline" };
        coverage.record_provider(ProviderCoverage::new(&peer_id, state).with_detail(detail));
    }
    if cancelled {
        coverage = coverage.note("search_cancelled");
    }
    let coverage = coverage.finish();
    emit_search_completed(
        &context.events,
        op_id,
        &normalized.query_hash,
        &coverage,
        total_new,
        total_rejected,
        total_ignored,
        total_conflicts,
        cancelled,
        started,
    );
    ops.finish(op_id);
}

fn discover_providers(network: &Arc<Network>, token: &CancelToken, deadline: Instant) -> Discovery {
    let self_id = network.peer_id().to_owned();
    let (sender, receiver) = mpsc::channel();
    let worker = Arc::clone(network);
    let spawned = thread::Builder::new()
        .name("nexus-search-discovery".to_owned())
        .spawn(move || {
            let _ = sender.send(worker.find_providers(&index_provider_key()));
        })
        .is_ok();
    if !spawned {
        return Discovery::Failed("provider_discovery_failed");
    }
    loop {
        if token.is_cancelled() {
            return Discovery::Cancelled;
        }
        let now = Instant::now();
        if now >= deadline {
            return Discovery::TimedOut;
        }
        let wait = SEARCH_POLL.min(deadline.saturating_duration_since(now));
        match receiver.recv_timeout(wait) {
            Ok(Ok(mut peers)) => {
                peers.retain(|peer| peer.peer_id != self_id);
                peers.sort_by(|left, right| left.peer_id.cmp(&right.peer_id));
                peers.dedup_by(|left, right| left.peer_id == right.peer_id);
                peers.truncate(MAX_PROVIDERS);
                if peers.is_empty() {
                    return Discovery::Empty;
                }
                return Discovery::Found(peers);
            }
            Ok(Err(error)) => return Discovery::Failed(error.code()),
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return Discovery::Failed("provider_discovery_failed"),
        }
    }
}

struct ProviderFetch {
    peer_id: String,
    result: Result<IndexResponse, NetError>,
}

fn spawn_provider_fetch(
    network: Arc<Network>,
    provider: DiscoveredPeer,
    request: IndexRequest,
    sender: Sender<ProviderFetch>,
) -> bool {
    thread::Builder::new()
        .name("nexus-search-provider".to_owned())
        .spawn(move || {
            let peer_id = provider.peer_id.clone();
            let result = fetch_provider(&network, &provider, &request);
            let _ = sender.send(ProviderFetch { peer_id, result });
        })
        .is_ok()
}

fn fetch_provider(
    network: &Network,
    provider: &DiscoveredPeer,
    request: &IndexRequest,
) -> Result<IndexResponse, NetError> {
    let connected = network
        .peers()
        .iter()
        .any(|peer| peer.peer_id == provider.peer_id && peer.state == PeerState::Connected);
    if !connected {
        let Some(address) = provider.addresses.first() else {
            return Err(NetError::NotConnected);
        };
        let address = if address.contains("/p2p/") {
            address.clone()
        } else {
            format!("{address}/p2p/{}", provider.peer_id)
        };
        network.connect(&address)?;
    }
    network.fetch_index_page(&provider.peer_id, request)
}

struct ProcessedProvider {
    coverage: ProviderCoverage,
    items: Vec<SearchItem>,
    conflict_updates: Vec<Value>,
    rejected: u32,
    ignored: u32,
    conflicts: u32,
}

struct ProviderProcessor<'a> {
    store: &'a Store,
    query: &'a SearchQuery,
    normalized: &'a NormalizedQuery,
    seen: HashMap<(RecordKind, String), SearchItem>,
}

impl<'a> ProviderProcessor<'a> {
    fn new(store: &'a Store, query: &'a SearchQuery, normalized: &'a NormalizedQuery) -> Self {
        Self {
            store,
            query,
            normalized,
            seen: HashMap::new(),
        }
    }

    fn process(&mut self, peer_id: &str, result: Result<IndexResponse, NetError>) -> ProcessedProvider {
        let response = match result {
            Ok(response) => response,
            Err(error) => {
                return ProcessedProvider {
                    coverage: ProviderCoverage::new(peer_id, provider_state(&error)).with_detail(error.code()),
                    items: Vec::new(),
                    conflict_updates: Vec::new(),
                    rejected: 0,
                    ignored: 0,
                    conflicts: 0,
                };
            }
        };
        if let Some(error) = response.error.as_deref() {
            return ProcessedProvider {
                coverage: ProviderCoverage::new(peer_id, SourceState::Failed).with_detail(error),
                items: Vec::new(),
                conflict_updates: Vec::new(),
                rejected: 0,
                ignored: 0,
                conflicts: 0,
            };
        }
        let mut accepted = Vec::new();
        let mut rejected = 0_u32;
        let mut ignored = 0_u32;
        for record in response.records {
            match verified_item(record) {
                Ok(item) if item_matches(self.query, self.normalized, &item) => accepted.push(item),
                Ok(_) => ignored = ignored.saturating_add(1),
                Err(()) => rejected = rejected.saturating_add(1),
            }
        }
        let records = u32::try_from(accepted.len()).unwrap_or(u32::MAX);
        let mut accumulator = ResultAccumulator::new(self.query.sort, accepted.len().max(1));
        for (index, mut item) in accepted.into_iter().enumerate() {
            item.rank = i64::try_from(index).unwrap_or(i64::MAX);
            accumulator.insert(item);
        }
        let mut items = Vec::new();
        let mut conflict_updates = Vec::new();
        let mut conflicts = 0_u32;
        for item in accumulator.finish() {
            let key = (item.kind, item.record_id.clone());
            if let Ok(Some(local)) = self.store.search_item(item.kind, &item.record_id) {
                let (merged, added) = merge_items(local, item, self.query.sort);
                if !added.is_empty() {
                    conflicts = conflicts.saturating_add(u32::try_from(added.len()).unwrap_or(u32::MAX));
                    conflict_updates.push(conflict_value(&merged, &added));
                }
            } else if let Some(existing) = self.seen.remove(&key) {
                let (merged, added) = merge_items(existing, item, self.query.sort);
                if !added.is_empty() {
                    conflicts = conflicts.saturating_add(u32::try_from(added.len()).unwrap_or(u32::MAX));
                    conflict_updates.push(conflict_value(&merged, &added));
                }
                self.seen.insert(key, merged);
            } else {
                self.seen.insert(key, item.clone());
                items.push(item);
            }
        }
        let state = if records == 0 && rejected > 0 {
            SourceState::Failed
        } else if !response.complete {
            SourceState::Truncated
        } else {
            SourceState::Ok
        };
        let mut coverage = ProviderCoverage::new(peer_id, state).with_records(records);
        if rejected > 0 {
            coverage = coverage.with_detail("invalid_records");
        }
        ProcessedProvider {
            coverage,
            items,
            conflict_updates,
            rejected,
            ignored,
            conflicts,
        }
    }
}

fn merge_items(existing: SearchItem, incoming: SearchItem, sort: SortOrder) -> (SearchItem, Vec<Conflict>) {
    let before = existing.conflicts.clone();
    let mut accumulator = ResultAccumulator::new(sort, 2);
    accumulator.insert(existing);
    accumulator.insert(incoming);
    let merged = accumulator.finish().into_iter().next().unwrap_or_else(|| {
        // `finish` always returns the single deduplicated row; this branch is
        // unreachable and only keeps the helper total.
        SearchItem {
            kind: RecordKind::Profile,
            record_id: String::new(),
            content_id: None,
            identity_id: String::new(),
            title: String::new(),
            summary: String::new(),
            category: None,
            tags: Vec::new(),
            revision: 0,
            published_at_ms: 0,
            updated_at_ms: 0,
            availability: Availability::Unknown,
            source: ResultSource::Provider,
            cached: false,
            rank: 0,
            conflicts: Vec::new(),
            same_content_count: 0,
        }
    });
    let added = merged
        .conflicts
        .iter()
        .filter(|conflict| !before.contains(conflict))
        .cloned()
        .collect();
    (merged, added)
}

fn verified_item(record: IndexRecord) -> Result<SearchItem, ()> {
    match record.kind {
        IndexKind::Profile => {
            let signed = SignedProfile::parse(&record.signed).map_err(|_| ())?;
            Ok(profile_item(&signed))
        }
        IndexKind::Publication => {
            let signed = SignedPublication::parse(&record.signed).map_err(|_| ())?;
            Ok(publication_item(&signed))
        }
    }
}

fn profile_item(signed: &SignedProfile) -> SearchItem {
    let profile = signed.profile();
    let mut summary = String::new();
    if let Some(username) = &profile.username {
        summary.push('@');
        summary.push_str(username);
        summary.push(' ');
    }
    if !profile.bio.is_empty() {
        summary.push_str(&profile.bio);
    }
    if summary.trim().is_empty() {
        summary = format!("Profile {}", short_id(profile.identity_id.as_str()));
    }
    SearchItem {
        kind: RecordKind::Profile,
        record_id: profile.identity_id.as_str().to_owned(),
        content_id: None,
        identity_id: profile.identity_id.as_str().to_owned(),
        title: sanitize_text(&profile.display_name, MAX_RESULT_TITLE_BYTES),
        summary: sanitize_text(&summary, MAX_RESULT_SUMMARY_BYTES),
        category: None,
        tags: Vec::new(),
        revision: profile.revision,
        published_at_ms: profile.updated_at_ms,
        updated_at_ms: profile.updated_at_ms,
        availability: Availability::Unknown,
        source: ResultSource::Provider,
        cached: false,
        rank: 0,
        conflicts: Vec::new(),
        same_content_count: 0,
    }
}

fn publication_item(signed: &SignedPublication) -> SearchItem {
    let body = signed.publication();
    let summary = if body.description.trim().is_empty() {
        body.display_filename.clone()
    } else {
        body.description.clone()
    };
    SearchItem {
        kind: RecordKind::Publication,
        record_id: to_hex(&signed.publication_id()),
        content_id: Some(to_hex(&body.content_id)),
        identity_id: body.identity_id.as_str().to_owned(),
        title: sanitize_text(&body.display_filename, MAX_RESULT_TITLE_BYTES),
        summary: sanitize_text(&summary, MAX_RESULT_SUMMARY_BYTES),
        category: Some(body.category),
        tags: body.tags.iter().take(16).cloned().collect(),
        revision: body.revision,
        published_at_ms: body.published_at_ms,
        updated_at_ms: body.published_at_ms,
        availability: Availability::RecordOnly,
        source: ResultSource::Provider,
        cached: false,
        rank: 0,
        conflicts: Vec::new(),
        same_content_count: 0,
    }
}

fn provider_state(error: &NetError) -> SourceState {
    match error {
        NetError::Timeout => SourceState::Timeout,
        NetError::UnsupportedProtocol => SourceState::Unsupported,
        NetError::NotConnected
        | NetError::Unavailable
        | NetError::NoViablePath
        | NetError::RelayUnavailable
        | NetError::PeerNotFound => SourceState::Unavailable,
        _ => SourceState::Failed,
    }
}

fn provider_request(query: &SearchQuery) -> IndexRequest {
    IndexRequest {
        query: query.text.clone(),
        kind: query.kind.map(index_kind),
        category: query.category,
        tags: query.tags.clone(),
        cursor: None,
        limit: query
            .limit
            .min(u16::try_from(MAX_PAGE_SIZE).unwrap_or(DEFAULT_PAGE_SIZE)),
    }
}

fn index_kind(kind: RecordKind) -> IndexKind {
    match kind {
        RecordKind::Profile => IndexKind::Profile,
        RecordKind::Publication => IndexKind::Publication,
    }
}

fn record_kind(kind: IndexKind) -> RecordKind {
    match kind {
        IndexKind::Profile => RecordKind::Profile,
        IndexKind::Publication => RecordKind::Publication,
    }
}

fn emit_result_page(
    events: &EventQueue,
    op_id: u64,
    query_hash: &str,
    provider_id: &str,
    items: &[SearchItem],
    conflicts: &[Value],
    coverage: &Coverage,
) {
    let _ = events.push(
        PendingEvent::progress(
            "search.result_page",
            Value::Map(vec![
                (Value::Text("query_hash".to_owned()), Value::Text(query_hash.to_owned())),
                (
                    Value::Text("provider_id".to_owned()),
                    Value::Text(provider_id.to_owned()),
                ),
                (
                    Value::Text("items".to_owned()),
                    Value::Array(items.iter().map(search_item_value).collect()),
                ),
                (Value::Text("conflicts".to_owned()), Value::Array(conflicts.to_vec())),
                (
                    Value::Text("coverage".to_owned()),
                    coverage_value(&coverage.clone().finish()),
                ),
            ]),
        )
        .with_op(op_id),
    );
}

#[allow(clippy::too_many_arguments)]
fn emit_search_completed(
    events: &EventQueue,
    op_id: u64,
    query_hash: &str,
    coverage: &Coverage,
    new_results: u32,
    rejected: u32,
    ignored: u32,
    conflicts: u32,
    cancelled: bool,
    started: Instant,
) {
    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let _ = events.push(
        PendingEvent::durable(
            "search.completed",
            Value::Map(vec![
                (Value::Text("query_hash".to_owned()), Value::Text(query_hash.to_owned())),
                (Value::Text("coverage".to_owned()), coverage_value(coverage)),
                (
                    Value::Text("new_results".to_owned()),
                    Value::Unsigned(u64::from(new_results)),
                ),
                (
                    Value::Text("rejected_records".to_owned()),
                    Value::Unsigned(u64::from(rejected)),
                ),
                (
                    Value::Text("ignored_records".to_owned()),
                    Value::Unsigned(u64::from(ignored)),
                ),
                (
                    Value::Text("conflict_count".to_owned()),
                    Value::Unsigned(u64::from(conflicts)),
                ),
                (Value::Text("cancelled".to_owned()), Value::Bool(cancelled)),
                (Value::Text("elapsed_ms".to_owned()), Value::Unsigned(elapsed_ms)),
            ]),
        )
        .with_op(op_id),
    );
}

fn search_value(
    op_id: u64,
    query: &SearchQuery,
    normalized: &NormalizedQuery,
    local: &LocalSearchPage,
    providers_pending: bool,
) -> Value {
    let coverage = if providers_pending {
        local.coverage.clone().pending()
    } else {
        local.coverage.clone()
    };
    Value::Map(vec![
        (Value::Text("op_id".to_owned()), Value::Unsigned(op_id)),
        (
            Value::Text("query_hash".to_owned()),
            Value::Text(normalized.query_hash.clone()),
        ),
        (Value::Text("generation".to_owned()), Value::Unsigned(local.generation)),
        (
            Value::Text("sort".to_owned()),
            Value::Text(query.sort.as_str().to_owned()),
        ),
        (
            Value::Text("indexed_at_ms".to_owned()),
            coverage.indexed_at_ms.map_or(Value::Null, Value::Unsigned),
        ),
        (
            Value::Text("deadline_ms".to_owned()),
            Value::Unsigned(coverage.deadline_ms),
        ),
        (
            Value::Text("results".to_owned()),
            Value::Array(local.items.iter().map(search_item_value).collect()),
        ),
        (
            Value::Text("total_matches".to_owned()),
            Value::Unsigned(u64::from(local.total_matches)),
        ),
        (
            Value::Text("next_cursor".to_owned()),
            local
                .next_cursor
                .as_ref()
                .map_or(Value::Null, |cursor| Value::Text(cursor.clone())),
        ),
        (Value::Text("page_complete".to_owned()), Value::Bool(local.complete)),
        (
            Value::Text("providers_pending".to_owned()),
            Value::Bool(providers_pending),
        ),
        (
            Value::Text("candidate_limit_reached".to_owned()),
            Value::Bool(local.candidate_limit_reached),
        ),
        (Value::Text("coverage".to_owned()), coverage_value(&coverage)),
    ])
}

fn explore_value(page: &ExplorePage) -> Value {
    Value::Map(vec![
        (
            Value::Text("mode".to_owned()),
            Value::Text(page.mode.as_str().to_owned()),
        ),
        (
            Value::Text("categories".to_owned()),
            Value::Array(
                page.categories
                    .iter()
                    .map(|category| {
                        Value::Map(vec![
                            (
                                Value::Text("category".to_owned()),
                                Value::Text(category.category.as_str().to_owned()),
                            ),
                            (
                                Value::Text("count".to_owned()),
                                Value::Unsigned(u64::from(category.count)),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            Value::Text("results".to_owned()),
            Value::Array(page.items.iter().map(search_item_value).collect()),
        ),
        (Value::Text("seed".to_owned()), Value::Unsigned(page.seed)),
        (
            Value::Text("next_cursor".to_owned()),
            page.next_cursor
                .as_ref()
                .map_or(Value::Null, |cursor| Value::Text(cursor.clone())),
        ),
        (Value::Text("page_complete".to_owned()), Value::Bool(page.complete)),
        (Value::Text("coverage".to_owned()), coverage_value(&page.coverage)),
    ])
}

fn search_item_value(item: &SearchItem) -> Value {
    Value::Map(vec![
        (
            Value::Text("kind".to_owned()),
            Value::Text(item.kind.as_str().to_owned()),
        ),
        (Value::Text("record_id".to_owned()), Value::Text(item.record_id.clone())),
        (
            Value::Text("content_id".to_owned()),
            item.content_id
                .as_ref()
                .map_or(Value::Null, |content_id| Value::Text(content_id.clone())),
        ),
        (
            Value::Text("identity_id".to_owned()),
            Value::Text(item.identity_id.clone()),
        ),
        (Value::Text("title".to_owned()), Value::Text(item.title.clone())),
        (Value::Text("summary".to_owned()), Value::Text(item.summary.clone())),
        (
            Value::Text("category".to_owned()),
            item.category
                .map_or(Value::Null, |category| Value::Text(category.as_str().to_owned())),
        ),
        (
            Value::Text("tags".to_owned()),
            Value::Array(item.tags.iter().cloned().map(Value::Text).collect()),
        ),
        (Value::Text("revision".to_owned()), Value::Unsigned(item.revision)),
        (
            Value::Text("published_at_ms".to_owned()),
            Value::Unsigned(item.published_at_ms),
        ),
        (
            Value::Text("updated_at_ms".to_owned()),
            Value::Unsigned(item.updated_at_ms),
        ),
        (
            Value::Text("availability".to_owned()),
            Value::Text(item.availability.as_str().to_owned()),
        ),
        (
            Value::Text("source".to_owned()),
            Value::Text(item.source.as_str().to_owned()),
        ),
        (Value::Text("cached".to_owned()), Value::Bool(item.cached)),
        (Value::Text("rank".to_owned()), signed_value(item.rank)),
        (
            Value::Text("conflicts".to_owned()),
            Value::Array(item.conflicts.iter().map(conflict_entry).collect()),
        ),
        (
            Value::Text("same_content_count".to_owned()),
            Value::Unsigned(u64::from(item.same_content_count)),
        ),
        (Value::Text("verified".to_owned()), Value::Bool(true)),
    ])
}

fn conflict_value(item: &SearchItem, conflicts: &[Conflict]) -> Value {
    Value::Map(vec![
        (
            Value::Text("kind".to_owned()),
            Value::Text(item.kind.as_str().to_owned()),
        ),
        (Value::Text("record_id".to_owned()), Value::Text(item.record_id.clone())),
        (
            Value::Text("conflicts".to_owned()),
            Value::Array(conflicts.iter().map(conflict_entry).collect()),
        ),
    ])
}

fn conflict_entry(conflict: &Conflict) -> Value {
    Value::Map(vec![
        (Value::Text("code".to_owned()), Value::Text(conflict.code.clone())),
        (Value::Text("detail".to_owned()), Value::Text(conflict.detail.clone())),
    ])
}

fn coverage_value(coverage: &Coverage) -> Value {
    Value::Map(vec![
        (
            Value::Text("label".to_owned()),
            Value::Text(coverage.label().to_owned()),
        ),
        (Value::Text("local".to_owned()), Value::Bool(coverage.local)),
        (Value::Text("cached".to_owned()), Value::Bool(coverage.cached)),
        (Value::Text("distributed".to_owned()), Value::Bool(coverage.distributed)),
        (Value::Text("complete".to_owned()), Value::Bool(coverage.complete)),
        (
            Value::Text("may_be_incomplete".to_owned()),
            Value::Bool(coverage.may_be_incomplete()),
        ),
        (
            Value::Text("providers_queried".to_owned()),
            Value::Unsigned(u64::from(coverage.providers_queried)),
        ),
        (
            Value::Text("providers_succeeded".to_owned()),
            Value::Unsigned(u64::from(coverage.providers_succeeded)),
        ),
        (
            Value::Text("providers_failed".to_owned()),
            Value::Unsigned(u64::from(coverage.providers_failed)),
        ),
        (
            Value::Text("providers_timed_out".to_owned()),
            Value::Unsigned(u64::from(coverage.providers_timed_out)),
        ),
        (
            Value::Text("providers_unsupported".to_owned()),
            Value::Unsigned(u64::from(coverage.providers_unsupported)),
        ),
        (
            Value::Text("indexed_at_ms".to_owned()),
            coverage.indexed_at_ms.map_or(Value::Null, Value::Unsigned),
        ),
        (
            Value::Text("deadline_ms".to_owned()),
            Value::Unsigned(coverage.deadline_ms),
        ),
        (
            Value::Text("notes".to_owned()),
            Value::Array(coverage.notes.iter().cloned().map(Value::Text).collect()),
        ),
        (
            Value::Text("providers".to_owned()),
            Value::Array(
                coverage
                    .providers
                    .iter()
                    .map(|provider| {
                        Value::Map(vec![
                            (
                                Value::Text("provider_id".to_owned()),
                                Value::Text(provider.provider_id.clone()),
                            ),
                            (
                                Value::Text("state".to_owned()),
                                Value::Text(provider.state.as_str().to_owned()),
                            ),
                            (
                                Value::Text("records".to_owned()),
                                Value::Unsigned(u64::from(provider.records)),
                            ),
                            (
                                Value::Text("detail".to_owned()),
                                provider
                                    .detail
                                    .as_ref()
                                    .map_or(Value::Null, |detail| Value::Text(detail.clone())),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
    ])
}

fn parse_search_query(params: &Value) -> Result<SearchQuery, CoreError> {
    if let Some(filters) = map_get(params, "filters")
        && !matches!(filters, Value::Null | Value::Map(_))
    {
        return Err(CoreError::InvalidArgument("filters must be a map or null"));
    }
    let text = optional_text(params, "query")?.ok_or(CoreError::InvalidArgument("query is required"))?;
    let kind = optional_text_param(params, "kind")?
        .map(|text| RecordKind::parse(&text).ok_or(CoreError::InvalidArgument("kind is not profile or publication")))
        .transpose()?;
    let category = optional_text_param(params, "category")?
        .map(|text| {
            nexus_protocol::Category::parse(&text).ok_or(CoreError::InvalidArgument(
                "category is not a known publication category",
            ))
        })
        .transpose()?;
    let tags = parse_tags(params)?;
    let sort = optional_text_param(params, "sort")?
        .map(|text| SortOrder::parse(&text).ok_or(CoreError::InvalidArgument("sort is not a known order")))
        .transpose()?
        .unwrap_or_default();
    let limit = optional_unsigned(params, "limit")?.unwrap_or(u64::from(DEFAULT_PAGE_SIZE));
    let limit = u16::try_from(limit).map_err(|_| CoreError::InvalidArgument("limit is too large"))?;
    let cursor = optional_text(params, "cursor")?;
    Ok(SearchQuery {
        text,
        kind,
        category,
        tags,
        sort,
        limit,
        cursor,
    })
}

fn parse_explore_query(params: &Value) -> Result<ExploreQuery, CoreError> {
    let mode = optional_text(params, "mode")?
        .map(|text| {
            ExploreMode::parse(&text).ok_or(CoreError::InvalidArgument("mode is not categories, recent, or random"))
        })
        .transpose()?
        .unwrap_or_default();
    let category = optional_text(params, "category")?
        .map(|text| {
            nexus_protocol::Category::parse(&text).ok_or(CoreError::InvalidArgument(
                "category is not a known publication category",
            ))
        })
        .transpose()?;
    let limit = optional_unsigned(params, "limit")?.unwrap_or(u64::from(nexus_search::DEFAULT_EXPLORE_LIMIT));
    let limit = u16::try_from(limit).map_err(|_| CoreError::InvalidArgument("limit is too large"))?;
    Ok(ExploreQuery {
        mode,
        category,
        limit,
        seed: optional_unsigned(params, "seed")?,
        cursor: optional_text(params, "cursor")?,
    })
}

fn parse_tags(params: &Value) -> Result<Vec<String>, CoreError> {
    match param_or_filter(params, "tags") {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => {
            if items.len() > MAX_TAG_FILTERS {
                return Err(CoreError::InvalidArgument("too many tag filters"));
            }
            let mut tags = Vec::with_capacity(items.len());
            for item in items {
                let tag = item
                    .as_text()
                    .ok_or(CoreError::InvalidArgument("tags must contain text"))?;
                if !tags.iter().any(|existing| existing == tag) {
                    tags.push(tag.to_owned());
                }
            }
            Ok(tags)
        }
        Some(_) => Err(CoreError::InvalidArgument("tags must be an array or null")),
    }
}

/// Reads a top-level field, falling back to the `filters` map.
fn param_or_filter<'a>(params: &'a Value, key: &str) -> Option<&'a Value> {
    match map_get(params, key) {
        Some(Value::Null) | None => map_get(params, "filters").and_then(|filters| map_get(filters, key)),
        Some(value) => Some(value),
    }
}

fn optional_text_param(params: &Value, key: &str) -> Result<Option<String>, CoreError> {
    match param_or_filter(params, key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Text(text)) => Ok(Some(text.clone())),
        Some(_) => Err(CoreError::InvalidArgument("a search filter has the wrong type")),
    }
}

fn search_domain_error(error: SearchError) -> DomainError {
    let code = if error.is_conflict() {
        DomainCode::Conflict
    } else {
        DomainCode::InvalidArgument
    };
    DomainError::new(code, error.to_string())
}

fn map_get<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    let Value::Map(entries) = value else {
        return None;
    };
    entries
        .iter()
        .find_map(|(candidate, value)| (candidate.as_text() == Some(key)).then_some(value))
}

fn optional_text(value: &Value, key: &str) -> Result<Option<String>, CoreError> {
    match map_get(value, key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Text(text)) => Ok(Some(text.clone())),
        Some(_) => Err(CoreError::InvalidArgument("a search field has the wrong type")),
    }
}

fn optional_unsigned(value: &Value, key: &str) -> Result<Option<u64>, CoreError> {
    match map_get(value, key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_unsigned()
            .map(Some)
            .ok_or(CoreError::InvalidArgument("a search field must be an unsigned integer")),
    }
}

fn signed_value(value: i64) -> Value {
    if value < 0 {
        Value::Negative(value)
    } else {
        Value::Unsigned(u64::try_from(value).unwrap_or(u64::MAX))
    }
}

fn short_id(identity_id: &str) -> String {
    identity_id
        .strip_prefix("nx1")
        .unwrap_or(identity_id)
        .chars()
        .take(8)
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use nexus_crypto::{IdentityId, KeyPair};
    use nexus_net::NetError;
    use nexus_protocol::{
        Category, IndexKind, IndexRecord, ProfileV1, PublicationV1, SignedProfile, SignedPublication,
    };
    use nexus_search::{RecordKind, SearchQuery, SortOrder, SourceState};

    use super::{provider_request, provider_state, verified_item};

    fn identity(seed: u8) -> (KeyPair, IdentityId) {
        let keypair = KeyPair::from_seed([seed; 32]);
        let identity = IdentityId::from_public_key(&keypair.public_key());
        (keypair, identity)
    }

    #[test]
    fn unsigned_and_tampered_provider_records_are_rejected() {
        let (keypair, identity) = identity(3);
        let profile = ProfileV1::new(identity.clone(), "Zephyr", "field notes", 1, 1_000).with_username("zephyr");
        let signed_profile = SignedProfile::sign(&profile, &keypair).expect("sign profile");
        let mut tampered = signed_profile.raw().to_vec();
        let last = tampered.len() - 1;
        tampered[last] ^= 0xFF;

        assert!(
            verified_item(IndexRecord {
                kind: IndexKind::Profile,
                signed: tampered,
            })
            .is_err()
        );
        assert!(
            verified_item(IndexRecord {
                kind: IndexKind::Profile,
                signed: Vec::new(),
            })
            .is_err()
        );
        assert!(
            verified_item(IndexRecord {
                kind: IndexKind::Publication,
                signed: signed_profile.raw().to_vec(),
            })
            .is_err()
        );

        let item = verified_item(IndexRecord {
            kind: IndexKind::Profile,
            signed: signed_profile.raw().to_vec(),
        })
        .expect("valid profile");
        assert_eq!(item.kind, RecordKind::Profile);
        assert_eq!(item.title, "Zephyr");
        assert!(item.summary.contains("@zephyr"));

        let publication = PublicationV1::new(
            identity,
            [7_u8; 32],
            42,
            "zephyr-report.txt",
            "A field report",
            Category::Documents,
            [8_u8; 32],
            1,
            2_000,
            "0.1.0-test",
        )
        .with_tags(vec!["zephyr".to_owned()]);
        let signed_publication = SignedPublication::sign(&publication, &keypair).expect("sign publication");
        let item = verified_item(IndexRecord {
            kind: IndexKind::Publication,
            signed: signed_publication.raw().to_vec(),
        })
        .expect("valid publication");
        assert_eq!(item.kind, RecordKind::Publication);
        assert_eq!(item.title, "zephyr-report.txt");
        assert_eq!(item.category, Some(Category::Documents));
        assert_eq!(item.tags, ["zephyr"]);
    }

    #[test]
    fn provider_requests_are_bounded_and_never_paginate() {
        let mut query = SearchQuery::new("field notes");
        query.limit = 20;
        query.sort = SortOrder::Recency;
        query.cursor = Some("v1:1:20:0123456789abcdef".to_owned());
        let request = provider_request(&query);
        assert_eq!(request.query, "field notes");
        assert_eq!(request.limit, 20);
        assert!(request.cursor.is_none());
        assert!(request.kind.is_none());
        assert!(request.tags.is_empty());
    }

    #[test]
    fn provider_transport_failures_map_to_honest_coverage_states() {
        assert_eq!(provider_state(&NetError::Timeout), SourceState::Timeout);
        assert_eq!(provider_state(&NetError::UnsupportedProtocol), SourceState::Unsupported);
        assert_eq!(provider_state(&NetError::NotConnected), SourceState::Unavailable);
        assert_eq!(provider_state(&NetError::ConnectionFailed), SourceState::Failed);
    }
}
