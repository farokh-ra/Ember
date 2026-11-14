-- NEXUS metadata schema, version 3.
--
-- Tombstones become independent signed records. A tombstone may arrive before
-- the publication row, and it must preserve the original uploader and content
-- attribution rather than inheriting them from a mutable local publication.

CREATE TABLE tombstones_v3 (
    publication_id TEXT PRIMARY KEY NOT NULL CHECK (length(publication_id) BETWEEN 1 AND 128),
    identity_id TEXT NOT NULL CHECK (length(identity_id) BETWEEN 1 AND 128),
    content_id TEXT NOT NULL CHECK (length(content_id) = 64),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    record_bytes BLOB NOT NULL CHECK (length(record_bytes) <= 65536),
    created_at INTEGER NOT NULL CHECK (created_at >= 0)
) STRICT;

INSERT INTO tombstones_v3 (publication_id, identity_id, content_id, revision, record_bytes, created_at)
SELECT t.publication_id, p.identity_id, p.content_id, t.revision, t.record_bytes, t.created_at
FROM tombstones t
JOIN publications p ON p.publication_id = t.publication_id;

DROP TABLE tombstones;
ALTER TABLE tombstones_v3 RENAME TO tombstones;

CREATE INDEX tombstones_identity_idx ON tombstones (identity_id, publication_id);
