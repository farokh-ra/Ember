-- Stage 13: profiles become signed public metadata with local draft, pending,
-- published, and remote-cache states. A cached remote profile does not need a
-- local identity reference or a local avatar blob, so the old foreign keys are
-- removed. Conflicts keep the rejected signed bytes instead of overwriting the
-- accepted record.
CREATE TABLE profiles_new (
    identity_id TEXT PRIMARY KEY NOT NULL CHECK (length(identity_id) BETWEEN 1 AND 128),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    record_bytes BLOB NOT NULL CHECK (length(record_bytes) <= 65536),
    display_name TEXT NOT NULL,
    username TEXT,
    bio TEXT,
    avatar_content_id TEXT,
    updated_at INTEGER NOT NULL CHECK (updated_at >= 0),
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= 0),
    state TEXT NOT NULL CHECK (state IN ('draft', 'pending', 'published', 'remote')),
    published_at INTEGER,
    last_checked_at INTEGER,
    freshness TEXT NOT NULL CHECK (freshness IN ('current', 'stale', 'unknown'))
) STRICT;

INSERT INTO profiles_new (
    identity_id, revision, record_bytes, display_name, username, bio,
    avatar_content_id, updated_at, updated_at_ms, state, published_at,
    last_checked_at, freshness
)
SELECT
    identity_id, revision, record_bytes, display_name, username, bio,
    avatar_content_id, updated_at, updated_at * 1000, 'published',
    updated_at * 1000, NULL, 'current'
FROM profiles;

DROP TABLE profiles;
ALTER TABLE profiles_new RENAME TO profiles;

CREATE TABLE profile_conflicts (
    conflict_id INTEGER PRIMARY KEY AUTOINCREMENT,
    identity_id TEXT NOT NULL CHECK (length(identity_id) BETWEEN 1 AND 128),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    record_bytes BLOB NOT NULL CHECK (length(record_bytes) <= 65536),
    observed_at INTEGER NOT NULL CHECK (observed_at >= 0),
    UNIQUE (identity_id, revision, record_bytes)
) STRICT;

CREATE INDEX profile_conflicts_identity ON profile_conflicts (identity_id, revision);
CREATE INDEX profiles_username ON profiles (username COLLATE NOCASE);
