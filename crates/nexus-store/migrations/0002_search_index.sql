-- Search tables are rebuildable derived state. FTS5 is compiled into the
-- bundled SQLite build; Stage 16 populates and queries this index.

CREATE VIRTUAL TABLE search_index USING fts5 (
    kind UNINDEXED,
    owner_id UNINDEXED,
    title,
    body,
    tokenize = 'unicode61 remove_diacritics 2'
);
