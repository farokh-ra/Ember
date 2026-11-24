-- Stage 16: the FTS5 table created in migration 2 becomes live derived state.
-- `search_meta` stores the index generation and last refresh time. The
-- generation changes whenever indexed content changes so a pagination cursor
-- can reject a snapshot it no longer matches. Existing verified records are
-- indexed once during the upgrade; later writes maintain the index.

CREATE TABLE search_meta (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
) STRICT;

-- FTS5 cannot use a normal index for UNINDEXED columns, so deleting one
-- document by `kind`/`owner_id` would scan the whole virtual table. This
-- ordinary table maps stable record IDs to FTS rowids and makes incremental
-- replacement logarithmic.
CREATE TABLE search_docs (
    rowid INTEGER PRIMARY KEY,
    kind TEXT NOT NULL,
    owner_id TEXT NOT NULL,
    UNIQUE (kind, owner_id)
) STRICT;

INSERT INTO search_meta (key, value) VALUES
    ('generation', '1'),
    ('indexed_at_ms', CAST(strftime('%s', 'now') AS INTEGER) * 1000);

INSERT INTO search_index (kind, owner_id, title, body)
SELECT
    'profile',
    identity_id,
    display_name,
    TRIM(
        COALESCE(username, '') || ' ' ||
        COALESCE(bio, '') || ' ' ||
        identity_id
    )
FROM profiles
WHERE state IN ('published', 'remote');

INSERT INTO search_index (kind, owner_id, title, body)
SELECT
    'publication',
    publication_id,
    title,
    TRIM(
        COALESCE(filename, '') || ' ' ||
        COALESCE(description, '') || ' ' ||
        category || ' ' ||
        COALESCE((
            SELECT group_concat(tag, ' ')
            FROM publication_tags
            WHERE publication_tags.publication_id = publications.publication_id
        ), '') || ' ' ||
        identity_id || ' ' ||
        publication_id
    )
FROM publications
WHERE tombstoned = 0;

INSERT INTO search_docs (rowid, kind, owner_id)
SELECT rowid, kind, owner_id FROM search_index;
