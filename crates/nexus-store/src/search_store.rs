//! Verified local FTS5 index maintenance, search, Explore, and provider pages.
//!
//! The index is derived state. Every row is written from an already verified
//! profile or publication record; invalid or unsigned bytes cannot reach this
//! module. Queries are bounded, sorted deterministically, and paginated with a
//! cursor tied to the index generation.

use std::collections::HashMap;

use nexus_protocol::Category;
use nexus_search::{
    Availability, CategoryCount, Coverage, ExploreMode, ExplorePage, ExploreQuery, MAX_LOCAL_CANDIDATES, RecordKind,
    ResultAccumulator, SearchCursor, SearchItem, SearchQuery, SortOrder, sanitize_text, seeded_shuffle,
};
use rusqlite::{Connection, OptionalExtension, params};

use crate::error::StoreResult;
use crate::model::{ProfileRecord, ProfileState, PublicationRecord};
use crate::store::{PROFILE_COLUMNS, PUBLICATION_COLUMNS, profile_from_row, publication_from_row, publication_tags};
use crate::util::now_ms;

const GENERATION_KEY: &str = "generation";
const INDEXED_AT_KEY: &str = "indexed_at_ms";
const MAX_EXPLORE_CANDIDATES: usize = 1_000;
const CATEGORIES: [Category; 8] = [
    Category::Documents,
    Category::Images,
    Category::Audio,
    Category::Video,
    Category::Software,
    Category::Data,
    Category::Archives,
    Category::Other,
];

/// Current derived-index state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SearchIndexStats {
    /// Monotonic index generation.
    pub generation: u64,
    /// Last full or incremental refresh in Unix milliseconds.
    pub indexed_at_ms: u64,
    /// Indexed profile rows.
    pub profiles: u32,
    /// Indexed publication rows.
    pub publications: u32,
}

/// One local search page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalSearchPage {
    /// Deduplicated, sorted, sanitized results.
    pub items: Vec<SearchItem>,
    /// Cursor for the next page, tied to the query and index generation.
    pub next_cursor: Option<String>,
    /// Whether this page reached the end of local matches.
    pub complete: bool,
    /// Local coverage label and refresh time.
    pub coverage: Coverage,
    /// Index generation used for this page.
    pub generation: u64,
    /// Effective sort order.
    pub sort: SortOrder,
    /// Number of filtered matches before the page slice.
    pub total_matches: u32,
    /// True when the bounded candidate scan was exhausted.
    pub candidate_limit_reached: bool,
}

/// Signed records returned by this device as an index provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderPage {
    /// Verified signed record bytes, in result order.
    pub records: Vec<(RecordKind, Vec<u8>)>,
    /// Cursor for another provider page.
    pub next_cursor: Option<String>,
    /// Whether the provider reached the end of matching records.
    pub complete: bool,
}

impl crate::store::Store {
    /// Returns current index state without changing it.
    pub fn search_index_stats(&self) -> StoreResult<SearchIndexStats> {
        self.db.read(stats)
    }

    /// Rebuilds the entire derived index from verified stored records.
    ///
    /// The replacement is transactional. A failure leaves the previous index
    /// intact.
    pub fn rebuild_search_index(&self) -> StoreResult<SearchIndexStats> {
        self.db.write(|connection| {
            let transaction = connection.transaction()?;
            transaction.execute("DELETE FROM search_index", [])?;
            transaction.execute("DELETE FROM search_docs", [])?;
            let profiles = {
                let mut statement = transaction.prepare(&format!(
                    "SELECT {PROFILE_COLUMNS} FROM profiles ORDER BY identity_id ASC"
                ))?;
                let rows = statement.query_map([], profile_from_row)?;
                rows.collect::<Result<Vec<_>, _>>()?
            };
            for record in &profiles {
                write_profile_index(&transaction, record)?;
            }
            let publications = {
                let mut statement = transaction.prepare(&format!(
                    "SELECT {PUBLICATION_COLUMNS} FROM publications
                     WHERE tombstoned = 0 ORDER BY publication_id ASC"
                ))?;
                let rows = statement.query_map([], publication_from_row)?;
                let mut records = rows.collect::<Result<Vec<_>, _>>()?;
                for record in &mut records {
                    record.tags = publication_tags(&transaction, &record.publication_id)?;
                }
                records
            };
            for record in &publications {
                write_publication_index(&transaction, record)?;
            }
            set_meta(&transaction, INDEXED_AT_KEY, now_ms())?;
            bump_generation(&transaction)?;
            let stats = stats(&transaction)?;
            transaction.commit()?;
            Ok(stats)
        })
    }

    /// Returns one indexed result row by kind and record ID.
    ///
    /// Used to deduplicate provider rows against the whole local index, not
    /// just the page the caller is currently displaying.
    pub fn search_item(&self, kind: RecordKind, record_id: &str) -> StoreResult<Option<SearchItem>> {
        self.db.read(|connection| match kind {
            RecordKind::Profile => profile_item(connection, record_id, 0),
            RecordKind::Publication => publication_item(connection, record_id, 0),
        })
    }

    /// Runs one validated search against the local index.
    ///
    /// Local search never consults the network. Provider results are merged by
    /// the caller with [`nexus_search::ResultAccumulator`].
    pub fn search_local(&self, query: &SearchQuery) -> StoreResult<LocalSearchPage> {
        let normalized = query.validate()?;
        let limit = usize::from(query.limit);
        self.db.read(|connection| {
            let (generation, indexed_at_ms) = read_meta(connection)?;
            let offset = match &query.cursor {
                Some(cursor) => {
                    let cursor = SearchCursor::parse(cursor, &normalized.query_hash)?.check_generation(generation)?;
                    cursor.offset
                }
                None => 0,
            };
            let mut candidates = Vec::new();
            let mut statement = connection.prepare(
                "SELECT kind, owner_id, bm25(search_index, 0.0, 0.0, 4.0, 1.0)
                 FROM search_index
                 WHERE search_index MATCH ?1
                 ORDER BY bm25(search_index, 0.0, 0.0, 4.0, 1.0) ASC, owner_id ASC
                 LIMIT ?2",
            )?;
            let rows = statement.query_map(
                params![
                    normalized.expression,
                    i64::try_from(MAX_LOCAL_CANDIDATES).unwrap_or(i64::MAX)
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, f64>(2)?,
                    ))
                },
            )?;
            for row in rows {
                let (kind, owner_id, rank) = row?;
                let Some(kind) = RecordKind::parse(&kind) else {
                    continue;
                };
                candidates.push((kind, owner_id, rank_to_i64(rank)));
            }
            let candidate_limit_reached = candidates.len() >= MAX_LOCAL_CANDIDATES;
            let mut items = Vec::new();
            for (kind, owner_id, rank) in candidates {
                if query.kind.is_some_and(|wanted| wanted != kind) {
                    continue;
                }
                match kind {
                    RecordKind::Profile => {
                        if let Some(item) = profile_item(connection, &owner_id, rank)?
                            && matches_filters(query, &item, connection)
                        {
                            items.push(item);
                        }
                    }
                    RecordKind::Publication => {
                        if let Some(item) = publication_item(connection, &owner_id, rank)?
                            && matches_filters(query, &item, connection)
                        {
                            items.push(item);
                        }
                    }
                }
            }
            let mut accumulator = ResultAccumulator::new(query.sort, MAX_LOCAL_CANDIDATES);
            accumulator.extend(items);
            let items = accumulator.finish();
            let total_matches = u32::try_from(items.len()).unwrap_or(u32::MAX);
            let offset_usize = usize::try_from(offset).unwrap_or(usize::MAX);
            let page: Vec<SearchItem> = items.iter().skip(offset_usize).take(limit).cloned().collect();
            let next = offset_usize.saturating_add(page.len());
            let next_cursor = (next < items.len()).then(|| {
                SearchCursor::encode(
                    generation,
                    u32::try_from(next).unwrap_or(u32::MAX),
                    &normalized.query_hash,
                )
            });
            let complete = next_cursor.is_none();
            let mut coverage = Coverage::local_only(Some(indexed_at_ms));
            coverage.cached = page.iter().any(|item| item.cached);
            if candidate_limit_reached {
                coverage = coverage.note("local_candidate_limit_reached");
            }
            Ok(LocalSearchPage {
                items: page,
                next_cursor,
                complete,
                coverage: coverage.finish(),
                generation,
                sort: query.sort,
                total_matches,
                candidate_limit_reached,
            })
        })
    }

    /// Runs one Explore query against the local verified index.
    pub fn explore_local(&self, query: &ExploreQuery) -> StoreResult<ExplorePage> {
        query.validate()?;
        self.db.read(|connection| {
            let (generation, indexed_at_ms) = read_meta(connection)?;
            let seed = query
                .seed
                .unwrap_or_else(|| now_ms().wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(17) ^ generation);
            let hash = query.query_hash(seed);
            let offset = match &query.cursor {
                Some(cursor) => {
                    let cursor = SearchCursor::parse(cursor, &hash)?.check_generation(generation)?;
                    cursor.offset
                }
                None => 0,
            };
            let limit = usize::from(query.limit);
            let mut coverage = Coverage::local_only(Some(indexed_at_ms));
            let mut categories = Vec::new();
            let mut items = Vec::new();

            if query.mode == ExploreMode::Categories {
                let counts = category_counts(connection)?;
                categories = CATEGORIES
                    .iter()
                    .map(|category| CategoryCount {
                        category: *category,
                        count: counts.get(category.as_str()).copied().unwrap_or(0),
                    })
                    .collect();
            } else {
                let mut records = explore_publications(connection, query.mode, query.category)?;
                if query.mode == ExploreMode::Random {
                    seeded_shuffle(&mut records, seed);
                }
                if records.len() >= MAX_EXPLORE_CANDIDATES {
                    coverage = coverage.note("explore_candidate_limit_reached");
                }
                let mut candidates = Vec::new();
                for (index, record) in records.iter().enumerate() {
                    let rank = if query.mode == ExploreMode::Random {
                        i64::try_from(index).unwrap_or(i64::MAX)
                    } else {
                        0
                    };
                    candidates.push(publication_item_from_record(connection, record, rank)?);
                }
                let mut accumulator = ResultAccumulator::new(
                    if query.mode == ExploreMode::Random {
                        SortOrder::Relevance
                    } else {
                        SortOrder::Recency
                    },
                    MAX_EXPLORE_CANDIDATES,
                );
                accumulator.extend(candidates);
                let candidates = accumulator.finish();
                let offset_usize = usize::try_from(offset).unwrap_or(usize::MAX);
                items = candidates.iter().skip(offset_usize).take(limit).cloned().collect();
                let next = offset_usize.saturating_add(items.len());
                if next < candidates.len() {
                    coverage = coverage.note("more_explore_results_available");
                    // The next cursor is bound to the seed and mode.
                    let cursor = SearchCursor::encode(generation, u32::try_from(next).unwrap_or(u32::MAX), &hash);
                    coverage.cached = items.iter().any(|item| item.cached);
                    return Ok(ExplorePage {
                        mode: query.mode,
                        categories,
                        items,
                        seed,
                        next_cursor: Some(cursor),
                        complete: false,
                        coverage: coverage.finish(),
                    });
                }
            }
            coverage.cached = items.iter().any(|item| item.cached);
            Ok(ExplorePage {
                mode: query.mode,
                categories,
                items,
                seed,
                next_cursor: None,
                complete: true,
                coverage: coverage.finish(),
            })
        })
    }

    /// Builds a bounded provider page from verified signed records.
    ///
    /// Records are selected by the same local index that serves local search,
    /// then re-read as exact signed bytes. A provider never invents metadata and
    /// never returns draft or tombstoned records.
    pub fn provider_search(&self, query: &SearchQuery) -> StoreResult<ProviderPage> {
        let page = self.search_local(query)?;
        let mut records = Vec::new();
        self.db.read(|connection| {
            for item in &page.items {
                let record = match item.kind {
                    RecordKind::Profile => connection
                        .query_row(
                            "SELECT record_bytes FROM profiles
                             WHERE identity_id = ?1 AND state IN ('published', 'remote')",
                            [&item.record_id],
                            |row| row.get::<_, Vec<u8>>(0),
                        )
                        .optional()?,
                    RecordKind::Publication => connection
                        .query_row(
                            "SELECT record_bytes FROM publications
                             WHERE publication_id = ?1 AND tombstoned = 0",
                            [&item.record_id],
                            |row| row.get::<_, Vec<u8>>(0),
                        )
                        .optional()?,
                };
                if let Some(bytes) = record {
                    records.push((item.kind, bytes));
                }
            }
            Ok(())
        })?;
        Ok(ProviderPage {
            records,
            next_cursor: page.next_cursor,
            complete: page.complete,
        })
    }
}

/// Replaces one profile row in the derived index. Called inside the same
/// transaction as the record write.
pub(crate) fn index_profile(connection: &Connection, record: &ProfileRecord) -> StoreResult<()> {
    write_profile_index(connection, record)?;
    set_meta(connection, INDEXED_AT_KEY, now_ms())?;
    bump_generation(connection)?;
    Ok(())
}

/// Re-reads one publication and refreshes its index row. This keeps the derived
/// index aligned with tombstone and tag updates without trusting caller state.
pub(crate) fn refresh_publication_index(connection: &Connection, publication_id: &str) -> StoreResult<()> {
    let record = connection
        .query_row(
            &format!("SELECT {PUBLICATION_COLUMNS} FROM publications WHERE publication_id = ?1"),
            [publication_id],
            publication_from_row,
        )
        .optional()?;
    match record {
        Some(mut record) if !record.tombstoned => {
            record.tags = publication_tags(connection, publication_id)?;
            write_publication_index(connection, &record)?;
        }
        _ => remove_publication_index(connection, publication_id)?,
    }
    set_meta(connection, INDEXED_AT_KEY, now_ms())?;
    bump_generation(connection)?;
    Ok(())
}

/// Removes one publication from the index and advances the generation.
pub(crate) fn remove_publication(connection: &Connection, publication_id: &str) -> StoreResult<()> {
    remove_publication_index(connection, publication_id)?;
    bump_generation(connection)?;
    Ok(())
}

/// Advances the generation without changing indexed content. Used when a
/// visible conflict row changes.
pub(crate) fn note_search_change(connection: &Connection) -> StoreResult<()> {
    bump_generation(connection)?;
    Ok(())
}

fn write_profile_index(connection: &Connection, record: &ProfileRecord) -> StoreResult<()> {
    remove_doc(connection, "profile", &record.identity_id)?;
    if matches!(record.state, ProfileState::Draft | ProfileState::Pending) {
        return Ok(());
    }
    let mut body = String::new();
    if let Some(username) = &record.username {
        body.push_str(username);
        body.push(' ');
    }
    if let Some(bio) = &record.bio {
        body.push_str(bio);
        body.push(' ');
    }
    body.push_str(&record.identity_id);
    insert_doc(connection, "profile", &record.identity_id, &record.display_name, &body)
}

fn write_publication_index(connection: &Connection, record: &PublicationRecord) -> StoreResult<()> {
    remove_doc(connection, "publication", &record.publication_id)?;
    if record.tombstoned {
        return Ok(());
    }
    let mut body = String::new();
    body.push_str(&record.filename);
    body.push(' ');
    if let Some(description) = &record.description {
        body.push_str(description);
        body.push(' ');
    }
    body.push_str(record.category.as_str());
    body.push(' ');
    for tag in &record.tags {
        body.push_str(tag);
        body.push(' ');
    }
    body.push_str(&record.identity_id);
    body.push(' ');
    body.push_str(&record.publication_id);
    insert_doc(connection, "publication", &record.publication_id, &record.title, &body)
}

fn insert_doc(connection: &Connection, kind: &str, owner_id: &str, title: &str, body: &str) -> StoreResult<()> {
    connection.execute(
        "INSERT INTO search_index (kind, owner_id, title, body) VALUES (?1, ?2, ?3, ?4)",
        params![kind, owner_id, title, body],
    )?;
    let rowid = connection.last_insert_rowid();
    connection.execute(
        "INSERT INTO search_docs (rowid, kind, owner_id) VALUES (?1, ?2, ?3)",
        params![rowid, kind, owner_id],
    )?;
    Ok(())
}

fn remove_doc(connection: &Connection, kind: &str, owner_id: &str) -> StoreResult<()> {
    connection.execute(
        "DELETE FROM search_index WHERE rowid IN
         (SELECT rowid FROM search_docs WHERE kind = ?1 AND owner_id = ?2)",
        params![kind, owner_id],
    )?;
    connection.execute(
        "DELETE FROM search_docs WHERE kind = ?1 AND owner_id = ?2",
        params![kind, owner_id],
    )?;
    Ok(())
}

fn remove_publication_index(connection: &Connection, publication_id: &str) -> StoreResult<()> {
    remove_doc(connection, "publication", publication_id)
}

fn read_meta(connection: &Connection) -> StoreResult<(u64, u64)> {
    let generation = meta_value(connection, GENERATION_KEY)?
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let indexed_at_ms = meta_value(connection, INDEXED_AT_KEY)?
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    Ok((generation, indexed_at_ms))
}

fn stats(connection: &Connection) -> StoreResult<SearchIndexStats> {
    let (generation, indexed_at_ms) = read_meta(connection)?;
    let profiles: i64 =
        connection.query_row("SELECT COUNT(*) FROM search_index WHERE kind = 'profile'", [], |row| {
            row.get(0)
        })?;
    let publications: i64 = connection.query_row(
        "SELECT COUNT(*) FROM search_index WHERE kind = 'publication'",
        [],
        |row| row.get(0),
    )?;
    Ok(SearchIndexStats {
        generation,
        indexed_at_ms,
        profiles: u32::try_from(profiles).unwrap_or(u32::MAX),
        publications: u32::try_from(publications).unwrap_or(u32::MAX),
    })
}

fn meta_value(connection: &Connection, key: &str) -> StoreResult<Option<String>> {
    Ok(connection
        .query_row("SELECT value FROM search_meta WHERE key = ?1", [key], |row| row.get(0))
        .optional()?)
}

fn set_meta(connection: &Connection, key: &str, value: u64) -> StoreResult<()> {
    connection.execute(
        "INSERT INTO search_meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value.to_string()],
    )?;
    Ok(())
}

fn bump_generation(connection: &Connection) -> StoreResult<u64> {
    let current = meta_value(connection, GENERATION_KEY)?
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let next = current.saturating_add(1);
    set_meta(connection, GENERATION_KEY, next)?;
    Ok(next)
}

fn profile_item(connection: &Connection, identity_id: &str, rank: i64) -> StoreResult<Option<SearchItem>> {
    let record = connection
        .query_row(
            &format!("SELECT {PROFILE_COLUMNS} FROM profiles WHERE identity_id = ?1"),
            [identity_id],
            profile_from_row,
        )
        .optional()?;
    let Some(record) = record else {
        return Ok(None);
    };
    if matches!(record.state, ProfileState::Draft | ProfileState::Pending) {
        return Ok(None);
    }
    let mut summary = String::new();
    if let Some(username) = &record.username {
        summary.push('@');
        summary.push_str(username);
        summary.push(' ');
    }
    if let Some(bio) = &record.bio {
        summary.push_str(bio);
    }
    if summary.trim().is_empty() {
        summary = format!("Profile {}", short_id(&record.identity_id));
    }
    let mut item = SearchItem {
        kind: RecordKind::Profile,
        record_id: record.identity_id.clone(),
        content_id: None,
        identity_id: record.identity_id.clone(),
        title: sanitize_text(&record.display_name, nexus_search::MAX_RESULT_TITLE_BYTES),
        summary: sanitize_text(&summary, nexus_search::MAX_RESULT_SUMMARY_BYTES),
        category: None,
        tags: Vec::new(),
        revision: record.revision,
        published_at_ms: record.updated_at_ms,
        updated_at_ms: record.updated_at.saturating_mul(1_000),
        availability: Availability::Unknown,
        source: nexus_search::ResultSource::Local,
        cached: record.state == ProfileState::Remote,
        rank,
        conflicts: Vec::new(),
        same_content_count: 0,
    };
    for revision in conflict_revisions(connection, &record.identity_id)? {
        item.push_conflict(nexus_search::Conflict::new(
            "profile_conflict",
            format!("revision {revision} also has a different signed record"),
        ));
    }
    Ok(Some(item))
}

fn conflict_revisions(connection: &Connection, identity_id: &str) -> StoreResult<Vec<u64>> {
    let mut statement = connection.prepare(
        "SELECT revision FROM profile_conflicts WHERE identity_id = ?1
         ORDER BY observed_at DESC, conflict_id DESC LIMIT 4",
    )?;
    let rows = statement.query_map([identity_id], |row| row.get::<_, i64>(0))?;
    Ok(rows
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|value| u64::try_from(value).unwrap_or(0))
        .collect())
}

fn publication_item(connection: &Connection, publication_id: &str, rank: i64) -> StoreResult<Option<SearchItem>> {
    let record = connection
        .query_row(
            &format!(
                "SELECT {PUBLICATION_COLUMNS} FROM publications
                 WHERE publication_id = ?1 AND tombstoned = 0"
            ),
            [publication_id],
            publication_from_row,
        )
        .optional()?;
    let Some(mut record) = record else {
        return Ok(None);
    };
    record.tags = publication_tags(connection, &record.publication_id)?;
    publication_item_from_record(connection, &record, rank).map(Some)
}

fn publication_item_from_record(
    _connection: &Connection,
    record: &PublicationRecord,
    rank: i64,
) -> StoreResult<SearchItem> {
    let summary = record
        .description
        .as_deref()
        .filter(|description| !description.trim().is_empty())
        .unwrap_or(&record.filename);
    Ok(SearchItem {
        kind: RecordKind::Publication,
        record_id: record.publication_id.clone(),
        content_id: Some(record.content_id.clone()),
        identity_id: record.identity_id.clone(),
        title: sanitize_text(&record.title, nexus_search::MAX_RESULT_TITLE_BYTES),
        summary: sanitize_text(summary, nexus_search::MAX_RESULT_SUMMARY_BYTES),
        category: Some(record.category),
        tags: record.tags.iter().take(16).cloned().collect(),
        revision: record.revision,
        published_at_ms: record.published_at.saturating_mul(1_000),
        updated_at_ms: record.updated_at.saturating_mul(1_000),
        availability: Availability::parse(record.availability.as_str()).unwrap_or(Availability::Unknown),
        source: nexus_search::ResultSource::Local,
        cached: !record.local,
        rank,
        conflicts: Vec::new(),
        same_content_count: 0,
    })
}

fn matches_filters(query: &SearchQuery, item: &SearchItem, connection: &Connection) -> bool {
    if let Some(category) = query.category
        && item.category != Some(category)
    {
        return false;
    }
    if !query.tags.is_empty() {
        if item.kind != RecordKind::Publication {
            return false;
        }
        let Ok(tags) = publication_tags(connection, &item.record_id) else {
            return false;
        };
        if !query.tags.iter().all(|wanted| tags.iter().any(|tag| tag == wanted)) {
            return false;
        }
    }
    true
}

fn category_counts(connection: &Connection) -> StoreResult<HashMap<String, u32>> {
    let mut statement =
        connection.prepare("SELECT category, COUNT(*) FROM publications WHERE tombstoned = 0 GROUP BY category")?;
    let rows = statement.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?;
    let mut counts = HashMap::new();
    for row in rows {
        let (category, count) = row?;
        counts.insert(category, u32::try_from(count).unwrap_or(u32::MAX));
    }
    Ok(counts)
}

fn explore_publications(
    connection: &Connection,
    mode: ExploreMode,
    category: Option<Category>,
) -> StoreResult<Vec<PublicationRecord>> {
    let order = if mode == ExploreMode::Recent {
        "published_at DESC, publication_id ASC"
    } else {
        "publication_id ASC"
    };
    let mut records = Vec::new();
    if let Some(category) = category {
        let mut statement = connection.prepare(&format!(
            "SELECT {PUBLICATION_COLUMNS} FROM publications
             WHERE tombstoned = 0 AND category = ?1 ORDER BY {order} LIMIT {MAX_EXPLORE_CANDIDATES}"
        ))?;
        let rows = statement.query_map([category.as_str()], publication_from_row)?;
        for row in rows {
            records.push(row?);
        }
    } else {
        let mut statement = connection.prepare(&format!(
            "SELECT {PUBLICATION_COLUMNS} FROM publications
             WHERE tombstoned = 0 ORDER BY {order} LIMIT {MAX_EXPLORE_CANDIDATES}"
        ))?;
        let rows = statement.query_map([], publication_from_row)?;
        for row in rows {
            records.push(row?);
        }
    }
    for record in &mut records {
        record.tags = publication_tags(connection, &record.publication_id)?;
    }
    Ok(records)
}

fn rank_to_i64(rank: f64) -> i64 {
    if rank.is_finite() {
        (rank * 1_000.0).round() as i64
    } else {
        0
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
