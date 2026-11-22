-- Stage 14: publications become signed public records with explicit local
-- serving state. A cached remote publication does not need a local identity
-- reference or a local content blob, so the old foreign keys are removed.
--
-- Replicas are rebuilt first because they referenced publications. The new
-- replica table keeps its content foreign key but treats publication_id as an
-- optional attribution value, not a row that must exist locally.

CREATE TABLE replicas_new (
    replica_id TEXT PRIMARY KEY NOT NULL CHECK (length(replica_id) BETWEEN 1 AND 128),
    content_id TEXT NOT NULL CHECK (length(content_id) = 64),
    publication_id TEXT CHECK (publication_id IS NULL OR length(publication_id) BETWEEN 1 AND 128),
    state TEXT NOT NULL CHECK (state IN ('active', 'paused', 'verification_failed', 'removed')),
    last_verified_at INTEGER,
    created_at INTEGER NOT NULL CHECK (created_at >= 0),
    updated_at INTEGER NOT NULL CHECK (updated_at >= 0),
    FOREIGN KEY (content_id) REFERENCES contents (content_id) ON DELETE RESTRICT
) STRICT;

INSERT INTO replicas_new (
    replica_id, content_id, publication_id, state, last_verified_at, created_at, updated_at
)
SELECT replica_id, content_id, publication_id, state, last_verified_at, created_at, updated_at
FROM replicas;

DROP TABLE replicas;
ALTER TABLE replicas_new RENAME TO replicas;

CREATE INDEX replicas_content_idx ON replicas (content_id);
CREATE INDEX replicas_publication_idx ON replicas (publication_id);

CREATE TABLE publications_new (
    publication_id TEXT PRIMARY KEY NOT NULL CHECK (length(publication_id) BETWEEN 1 AND 128),
    identity_id TEXT NOT NULL CHECK (length(identity_id) BETWEEN 1 AND 128),
    content_id TEXT NOT NULL CHECK (length(content_id) = 64),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    record_bytes BLOB NOT NULL CHECK (length(record_bytes) <= 65536),
    title TEXT NOT NULL,
    description TEXT,
    filename TEXT NOT NULL,
    published_at INTEGER NOT NULL CHECK (published_at >= 0),
    updated_at INTEGER NOT NULL CHECK (updated_at >= 0),
    tombstoned INTEGER NOT NULL DEFAULT 0 CHECK (tombstoned IN (0, 1)),
    category TEXT NOT NULL CHECK (category IN (
        'documents', 'images', 'audio', 'video', 'software', 'data', 'archives', 'other'
    )),
    size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
    local INTEGER NOT NULL DEFAULT 0 CHECK (local IN (0, 1)),
    serving INTEGER NOT NULL DEFAULT 0 CHECK (serving IN (0, 1)),
    announced INTEGER NOT NULL DEFAULT 0 CHECK (announced IN (0, 1)),
    availability TEXT NOT NULL CHECK (availability IN (
        'local', 'available_now', 'seen_recently', 'no_known_source', 'record_only'
    )),
    last_checked_at INTEGER CHECK (last_checked_at IS NULL OR last_checked_at >= 0)
) STRICT;

INSERT INTO publications_new (
    publication_id, identity_id, content_id, revision, record_bytes, title,
    description, filename, published_at, updated_at, tombstoned, category,
    size_bytes, local, serving, announced, availability, last_checked_at
)
SELECT
    p.publication_id, p.identity_id, p.content_id, p.revision, p.record_bytes,
    p.title, p.description, p.filename, p.published_at, p.updated_at,
    p.tombstoned, 'other', COALESCE(c.size_bytes, 0), 1,
    CASE WHEN p.tombstoned = 0 THEN 1 ELSE 0 END,
    0,
    CASE WHEN p.tombstoned = 0 THEN 'local' ELSE 'record_only' END,
    NULL
FROM publications p
LEFT JOIN contents c ON c.content_id = p.content_id;

DROP TABLE publications;
ALTER TABLE publications_new RENAME TO publications;

CREATE INDEX publications_identity_idx ON publications (identity_id, published_at DESC);
CREATE INDEX publications_content_idx ON publications (content_id);
CREATE INDEX publications_local_idx ON publications (local, publication_id);
CREATE INDEX publications_serving_idx ON publications (content_id, serving, tombstoned);

CREATE TABLE publication_tags (
    publication_id TEXT NOT NULL CHECK (length(publication_id) BETWEEN 1 AND 128),
    tag TEXT NOT NULL CHECK (length(tag) BETWEEN 1 AND 32),
    position INTEGER NOT NULL CHECK (position >= 0),
    PRIMARY KEY (publication_id, tag)
) STRICT;

CREATE INDEX publication_tags_tag_idx ON publication_tags (tag);
