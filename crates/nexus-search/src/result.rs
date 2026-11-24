//! Verified search result items, deduplication, conflict visibility, and
//! explainable ordering.

use std::cmp::Ordering;
use std::collections::HashMap;

use nexus_protocol::Category;

use crate::query::{NormalizedQuery, RecordKind, SearchQuery, SortOrder};
use crate::sanitize::sanitize_text;

/// Maximum bytes in a sanitized title.
pub const MAX_RESULT_TITLE_BYTES: usize = 160;
/// Maximum bytes in a sanitized summary.
pub const MAX_RESULT_SUMMARY_BYTES: usize = 280;
/// Maximum conflicts attached to one result.
pub const MAX_CONFLICTS_PER_RESULT: usize = 4;

/// Last known availability of a publication's content.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Availability {
    /// No availability observation is known.
    #[default]
    Unknown,
    /// Verified bytes are present on this device.
    Local,
    /// At least one provider responded during the last check.
    AvailableNow,
    /// A provider was observed recently but none responded now.
    SeenRecently,
    /// No source is known right now.
    NoKnownSource,
    /// Only the signed record is known.
    RecordOnly,
}

impl Availability {
    /// Stable storage and wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Local => "local",
            Self::AvailableNow => "available_now",
            Self::SeenRecently => "seen_recently",
            Self::NoKnownSource => "no_known_source",
            Self::RecordOnly => "record_only",
        }
    }

    /// Parses a stored availability value.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "unknown" => Some(Self::Unknown),
            "local" => Some(Self::Local),
            "available_now" => Some(Self::AvailableNow),
            "seen_recently" => Some(Self::SeenRecently),
            "no_known_source" => Some(Self::NoKnownSource),
            "record_only" => Some(Self::RecordOnly),
            _ => None,
        }
    }

    /// Ascending sort rank. Local verified bytes always come first.
    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            Self::Local => 0,
            Self::AvailableNow => 1,
            Self::SeenRecently => 2,
            Self::NoKnownSource => 3,
            Self::RecordOnly => 4,
            Self::Unknown => 5,
        }
    }
}

/// Where a result row was observed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResultSource {
    /// The local verified index.
    Local,
    /// A verified record returned by an optional index provider.
    Provider,
}

impl ResultSource {
    /// Stable diagnostics name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Provider => "provider",
        }
    }
}

/// One visible conflict attached to a result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Conflict {
    /// Stable machine code such as `revision_conflict`.
    pub code: String,
    /// Bounded plain-text explanation.
    pub detail: String,
}

impl Conflict {
    /// Builds a bounded conflict record.
    #[must_use]
    pub fn new(code: &str, detail: impl AsRef<str>) -> Self {
        Self {
            code: sanitize_text(code, 48),
            detail: sanitize_text(detail.as_ref(), MAX_RESULT_SUMMARY_BYTES),
        }
    }
}

/// One deduplicated result. Every row carries its verification source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchItem {
    /// Signed record kind.
    pub kind: RecordKind,
    /// Identity ID for profiles, publication ID for publications.
    pub record_id: String,
    /// Content ID for publications, enabling content grouping.
    pub content_id: Option<String>,
    /// Signing identity.
    pub identity_id: String,
    /// Sanitized display title.
    pub title: String,
    /// Sanitized short summary.
    pub summary: String,
    /// Closed publication category.
    pub category: Option<Category>,
    /// Ordered tags.
    pub tags: Vec<String>,
    /// Signed revision.
    pub revision: u64,
    /// Signed publication time in Unix milliseconds.
    pub published_at_ms: u64,
    /// Local update time in Unix milliseconds.
    pub updated_at_ms: u64,
    /// Last known availability.
    pub availability: Availability,
    /// Verification source.
    pub source: ResultSource,
    /// True when the row came from a remote cache rather than local work.
    pub cached: bool,
    /// FTS5 rank for local rows; `0` for provider-only rows. Lower is better.
    pub rank: i64,
    /// Visible conflicts. Never hidden or silently resolved.
    pub conflicts: Vec<Conflict>,
    /// Number of visible publications sharing this content ID.
    pub same_content_count: u32,
}

impl SearchItem {
    /// Adds a conflict, deduplicating and bounding the list.
    pub fn push_conflict(&mut self, conflict: Conflict) {
        if self
            .conflicts
            .iter()
            .any(|existing| existing.code == conflict.code && existing.detail == conflict.detail)
        {
            return;
        }
        if self.conflicts.len() < MAX_CONFLICTS_PER_RESULT {
            self.conflicts.push(conflict);
        }
    }
}

/// Returns true when a provider result satisfies every filter and every query
/// token prefix-matches a searchable word.
///
/// Providers are untrusted. The core re-checks returned records against the
/// same filters the local index applies before a provider row can be shown.
#[must_use]
pub fn item_matches(query: &SearchQuery, normalized: &NormalizedQuery, item: &SearchItem) -> bool {
    if query.kind.is_some_and(|kind| kind != item.kind) {
        return false;
    }
    if let Some(category) = query.category
        && item.category != Some(category)
    {
        return false;
    }
    if !query.tags.is_empty() {
        if item.kind != RecordKind::Publication {
            return false;
        }
        if !query
            .tags
            .iter()
            .all(|wanted| item.tags.iter().any(|tag| tag == wanted))
        {
            return false;
        }
    }
    let mut words = Vec::new();
    collect_words(&item.title, &mut words);
    collect_words(&item.summary, &mut words);
    for tag in &item.tags {
        collect_words(tag, &mut words);
    }
    if let Some(category) = item.category {
        collect_words(category.as_str(), &mut words);
    }
    collect_words(&item.identity_id, &mut words);
    collect_words(&item.record_id, &mut words);
    normalized
        .tokens
        .iter()
        .all(|token| words.iter().any(|word| word.starts_with(token)))
}

fn collect_words(text: &str, words: &mut Vec<String>) {
    for raw in text.split(|character: char| !character.is_alphanumeric()) {
        if !raw.is_empty() {
            words.push(raw.to_lowercase());
        }
    }
}

/// Accumulates local and provider pages, deduplicating by record identity.
///
/// No popularity or engagement signal participates in ordering.
pub struct ResultAccumulator {
    sort: SortOrder,
    max_items: usize,
    items: Vec<SearchItem>,
    positions: HashMap<(RecordKind, String), usize>,
}

impl ResultAccumulator {
    /// Creates an accumulator for one query's sort order.
    #[must_use]
    pub fn new(sort: SortOrder, max_items: usize) -> Self {
        Self {
            sort,
            max_items,
            items: Vec::new(),
            positions: HashMap::new(),
        }
    }

    /// Inserts or merges one result.
    pub fn insert(&mut self, item: SearchItem) {
        let key = (item.kind, item.record_id.clone());
        if let Some(index) = self.positions.get(&key).copied() {
            merge_duplicate(&mut self.items[index], item);
            return;
        }
        self.positions.insert(key, self.items.len());
        self.items.push(item);
    }

    /// Inserts every result from a page in order.
    pub fn extend(&mut self, items: impl IntoIterator<Item = SearchItem>) {
        for item in items {
            self.insert(item);
        }
    }

    /// Number of visible rows before final truncation.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// True when nothing has been inserted.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Sorts, groups shared content IDs, and truncates to the bounded page.
    #[must_use]
    pub fn finish(mut self) -> Vec<SearchItem> {
        let mut content_counts: HashMap<String, u32> = HashMap::new();
        for item in &self.items {
            if item.kind == RecordKind::Publication
                && let Some(content_id) = &item.content_id
                && !content_id.is_empty()
            {
                *content_counts.entry(content_id.clone()).or_insert(0) += 1;
            }
        }
        for item in &mut self.items {
            item.same_content_count = item
                .content_id
                .as_ref()
                .and_then(|content_id| content_counts.get(content_id))
                .copied()
                .unwrap_or(0);
        }
        self.items.sort_by(|left, right| compare(left, right, self.sort));
        self.items.truncate(self.max_items);
        self.items
    }
}

fn merge_duplicate(existing: &mut SearchItem, incoming: SearchItem) {
    let revision_changed = existing.revision != incoming.revision;
    let metadata_changed = existing.title != incoming.title || existing.summary != incoming.summary;
    let existing_revision = existing.revision;
    let incoming_revision = incoming.revision;
    let incoming_source = incoming.source;
    let incoming_conflicts = incoming.conflicts.clone();

    if incoming.source == ResultSource::Local && existing.source == ResultSource::Provider {
        let local_conflicts = std::mem::take(&mut existing.conflicts);
        let local_cached = existing.cached;
        *existing = incoming;
        existing.cached = local_cached;
        for conflict in local_conflicts {
            existing.push_conflict(conflict);
        }
    } else {
        merge_tags(&mut existing.tags, &incoming.tags);
    }
    for conflict in incoming_conflicts {
        existing.push_conflict(conflict);
    }
    if revision_changed {
        existing.push_conflict(Conflict::new(
            "revision_conflict",
            format!("kept revision {existing_revision}; another source returned revision {incoming_revision}"),
        ));
    }
    if metadata_changed {
        existing.push_conflict(Conflict::new(
            "metadata_conflict",
            format!(
                "another {} source described this record differently",
                incoming_source.as_str()
            ),
        ));
    }
}

fn merge_tags(existing: &mut Vec<String>, incoming: &[String]) {
    for tag in incoming {
        if !existing.iter().any(|candidate| candidate == tag) {
            existing.push(tag.clone());
        }
    }
}

fn compare(left: &SearchItem, right: &SearchItem, sort: SortOrder) -> Ordering {
    let primary = match sort {
        SortOrder::Relevance => left
            .rank
            .cmp(&right.rank)
            .then_with(|| right.published_at_ms.cmp(&left.published_at_ms)),
        SortOrder::Recency => right
            .published_at_ms
            .cmp(&left.published_at_ms)
            .then_with(|| left.rank.cmp(&right.rank)),
        SortOrder::Availability => left
            .availability
            .rank()
            .cmp(&right.availability.rank())
            .then_with(|| right.published_at_ms.cmp(&left.published_at_ms))
            .then_with(|| left.rank.cmp(&right.rank)),
    };
    primary
        .then_with(|| left.kind.as_str().cmp(right.kind.as_str()))
        .then_with(|| left.record_id.cmp(&right.record_id))
}

#[cfg(test)]
mod tests {
    use super::{Availability, Conflict, ResultAccumulator, ResultSource, SearchItem};
    use crate::query::{RecordKind, SortOrder};

    fn item(kind: RecordKind, id: &str, source: ResultSource, rank: i64) -> SearchItem {
        SearchItem {
            kind,
            record_id: id.to_owned(),
            content_id: (kind == RecordKind::Publication).then(|| format!("content-{id}")),
            identity_id: "nx1owner".to_owned(),
            title: format!("Title {id}"),
            summary: "Summary".to_owned(),
            category: None,
            tags: vec!["tag".to_owned()],
            revision: 1,
            published_at_ms: 1_000,
            updated_at_ms: 1_000,
            availability: Availability::RecordOnly,
            source,
            cached: source == ResultSource::Provider,
            rank,
            conflicts: Vec::new(),
            same_content_count: 0,
        }
    }

    #[test]
    fn duplicate_record_identities_are_collapsed_and_conflicts_surface() {
        let mut accumulator = ResultAccumulator::new(SortOrder::Relevance, 20);
        accumulator.insert(item(RecordKind::Publication, "pub-a", ResultSource::Provider, 0));
        let mut conflicting = item(RecordKind::Publication, "pub-a", ResultSource::Provider, 0);
        conflicting.revision = 2;
        conflicting.title = "Different title".to_owned();
        accumulator.insert(conflicting);
        let results = accumulator.finish();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].revision, 1);
        assert!(
            results[0]
                .conflicts
                .iter()
                .any(|conflict| conflict.code == "revision_conflict")
        );
        assert!(
            results[0]
                .conflicts
                .iter()
                .any(|conflict| conflict.code == "metadata_conflict")
        );
    }

    #[test]
    fn local_results_replace_provider_duplicates() {
        let mut accumulator = ResultAccumulator::new(SortOrder::Relevance, 20);
        accumulator.insert(item(RecordKind::Profile, "nx1a", ResultSource::Provider, 0));
        accumulator.insert(item(RecordKind::Profile, "nx1a", ResultSource::Local, -3));
        let results = accumulator.finish();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, ResultSource::Local);
        assert_eq!(results[0].rank, -3);
    }

    #[test]
    fn availability_sort_puts_local_first_then_recency() {
        let mut accumulator = ResultAccumulator::new(SortOrder::Availability, 20);
        let mut remote = item(RecordKind::Publication, "pub-a", ResultSource::Provider, 0);
        remote.availability = Availability::RecordOnly;
        remote.published_at_ms = 9_000;
        let mut local = item(RecordKind::Publication, "pub-b", ResultSource::Local, 0);
        local.availability = Availability::Local;
        local.published_at_ms = 1_000;
        let mut available = item(RecordKind::Publication, "pub-c", ResultSource::Provider, 0);
        available.availability = Availability::AvailableNow;
        available.published_at_ms = 5_000;
        accumulator.extend([remote, local, available]);
        let results = accumulator.finish();
        assert_eq!(results[0].availability, Availability::Local);
        assert_eq!(results[1].availability, Availability::AvailableNow);
        assert_eq!(results[2].availability, Availability::RecordOnly);
    }

    #[test]
    fn shared_content_ids_are_grouped_but_not_hidden() {
        let mut accumulator = ResultAccumulator::new(SortOrder::Recency, 20);
        let mut left = item(RecordKind::Publication, "pub-a", ResultSource::Local, 0);
        left.content_id = Some("shared".to_owned());
        let mut right = item(RecordKind::Publication, "pub-b", ResultSource::Provider, 0);
        right.content_id = Some("shared".to_owned());
        accumulator.extend([left, right]);
        let results = accumulator.finish();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|item| item.same_content_count == 2));
    }

    #[test]
    fn conflicts_are_bounded_and_deduplicated() {
        let mut result = item(RecordKind::Profile, "nx1a", ResultSource::Local, 0);
        for index in 0..10 {
            result.push_conflict(Conflict::new("metadata_conflict", format!("detail {index}")));
        }
        result.push_conflict(Conflict::new("metadata_conflict", "detail 0"));
        assert_eq!(result.conflicts.len(), super::MAX_CONFLICTS_PER_RESULT);
    }

    #[test]
    fn provider_items_must_match_filters_and_every_token() {
        let mut query = crate::query::SearchQuery::new("field notes");
        let normalized = query.validate().expect("query");
        let mut provider_item = item(RecordKind::Publication, "pub-a", ResultSource::Provider, 0);
        provider_item.title = "Field notes from the coast".to_owned();
        assert!(super::item_matches(&query, &normalized, &provider_item));

        provider_item.title = "Unrelated record".to_owned();
        provider_item.summary = "nothing to see".to_owned();
        provider_item.tags.clear();
        assert!(!super::item_matches(&query, &normalized, &provider_item));

        let mut filtered_item = item(RecordKind::Publication, "pub-b", ResultSource::Provider, 0);
        filtered_item.title = "Field notes".to_owned();
        filtered_item.category = Some(nexus_protocol::Category::Documents);
        filtered_item.tags = vec!["stage16".to_owned()];
        query.category = Some(nexus_protocol::Category::Images);
        assert!(!super::item_matches(&query, &normalized, &filtered_item));

        query.category = None;
        query.tags = vec!["stage16".to_owned()];
        assert!(super::item_matches(&query, &normalized, &filtered_item));
        query.tags = vec!["missing".to_owned()];
        assert!(!super::item_matches(&query, &normalized, &filtered_item));

        query.tags.clear();
        query.kind = Some(RecordKind::Profile);
        assert!(!super::item_matches(&query, &normalized, &filtered_item));
    }
}
