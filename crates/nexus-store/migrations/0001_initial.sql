-- NEXUS metadata schema, version 1.
--
-- File bytes never live in this database. See ADR-0004 and ARCHITECTURE.md
-- section 7 for the managed-store rules. Timestamps are Unix seconds.

CREATE TABLE schema_migrations (
    version INTEGER PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    applied_at INTEGER NOT NULL CHECK (applied_at >= 0),
    checksum TEXT NOT NULL CHECK (length(checksum) = 64)
) STRICT;

CREATE TABLE settings (
    key TEXT PRIMARY KEY NOT NULL CHECK (length(key) BETWEEN 1 AND 128),
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL CHECK (updated_at >= 0)
) STRICT;

INSERT INTO settings (key, value, updated_at) VALUES
    ('managed_quota_bytes', '10737418240', 0),
    ('download_retention_days', '30', 0);

CREATE TABLE contents (
    content_id TEXT PRIMARY KEY NOT NULL CHECK (length(content_id) = 64),
    size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
    state TEXT NOT NULL CHECK (state IN ('pending', 'verified', 'missing', 'quarantined')),
    blob_path TEXT NOT NULL CHECK (length(blob_path) BETWEEN 1 AND 512),
    verified_at INTEGER,
    created_at INTEGER NOT NULL CHECK (created_at >= 0),
    updated_at INTEGER NOT NULL CHECK (updated_at >= 0)
) STRICT;

CREATE TABLE identity_refs (
    identity_id TEXT PRIMARY KEY NOT NULL CHECK (length(identity_id) BETWEEN 1 AND 128),
    public_key BLOB NOT NULL CHECK (length(public_key) = 32),
    key_file TEXT NOT NULL CHECK (length(key_file) BETWEEN 1 AND 512),
    label TEXT,
    created_at INTEGER NOT NULL CHECK (created_at >= 0)
) STRICT;

CREATE TABLE profiles (
    identity_id TEXT PRIMARY KEY NOT NULL,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    record_bytes BLOB NOT NULL CHECK (length(record_bytes) <= 65536),
    display_name TEXT NOT NULL,
    username TEXT,
    bio TEXT,
    avatar_content_id TEXT,
    updated_at INTEGER NOT NULL CHECK (updated_at >= 0),
    FOREIGN KEY (identity_id) REFERENCES identity_refs (identity_id) ON DELETE RESTRICT,
    FOREIGN KEY (avatar_content_id) REFERENCES contents (content_id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE publications (
    publication_id TEXT PRIMARY KEY NOT NULL CHECK (length(publication_id) BETWEEN 1 AND 128),
    identity_id TEXT NOT NULL,
    content_id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    record_bytes BLOB NOT NULL CHECK (length(record_bytes) <= 65536),
    title TEXT NOT NULL,
    description TEXT,
    filename TEXT NOT NULL,
    published_at INTEGER NOT NULL CHECK (published_at >= 0),
    updated_at INTEGER NOT NULL CHECK (updated_at >= 0),
    tombstoned INTEGER NOT NULL DEFAULT 0 CHECK (tombstoned IN (0, 1)),
    FOREIGN KEY (identity_id) REFERENCES identity_refs (identity_id) ON DELETE RESTRICT,
    FOREIGN KEY (content_id) REFERENCES contents (content_id) ON DELETE RESTRICT
) STRICT;

CREATE INDEX publications_identity_idx ON publications (identity_id, published_at DESC);
CREATE INDEX publications_content_idx ON publications (content_id);

CREATE TABLE tombstones (
    publication_id TEXT PRIMARY KEY NOT NULL,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    record_bytes BLOB NOT NULL CHECK (length(record_bytes) <= 65536),
    created_at INTEGER NOT NULL CHECK (created_at >= 0),
    FOREIGN KEY (publication_id) REFERENCES publications (publication_id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE content_refs (
    ref_id INTEGER PRIMARY KEY AUTOINCREMENT,
    content_id TEXT NOT NULL,
    ref_type TEXT NOT NULL CHECK (ref_type IN ('publication_original', 'active_replica', 'retained_download')),
    owner_id TEXT NOT NULL CHECK (length(owner_id) BETWEEN 1 AND 128),
    created_at INTEGER NOT NULL CHECK (created_at >= 0),
    UNIQUE (content_id, ref_type, owner_id),
    FOREIGN KEY (content_id) REFERENCES contents (content_id) ON DELETE RESTRICT
) STRICT;

CREATE INDEX content_refs_content_idx ON content_refs (content_id);
CREATE INDEX content_refs_owner_idx ON content_refs (ref_type, owner_id);

CREATE TABLE downloads (
    download_id TEXT PRIMARY KEY NOT NULL CHECK (length(download_id) BETWEEN 1 AND 128),
    content_id TEXT NOT NULL CHECK (length(content_id) = 64),
    display_name TEXT NOT NULL,
    expected_size INTEGER NOT NULL CHECK (expected_size >= 0),
    verified_prefix INTEGER NOT NULL DEFAULT 0 CHECK (verified_prefix >= 0 AND verified_prefix <= expected_size),
    state TEXT NOT NULL CHECK (state IN ('pending', 'downloading', 'paused', 'completed', 'failed', 'cancelled')),
    partial_file TEXT NOT NULL CHECK (length(partial_file) BETWEEN 1 AND 512),
    created_at INTEGER NOT NULL CHECK (created_at >= 0),
    updated_at INTEGER NOT NULL CHECK (updated_at >= 0),
    last_error TEXT
) STRICT;

CREATE INDEX downloads_state_idx ON downloads (state, updated_at);

CREATE TABLE replicas (
    replica_id TEXT PRIMARY KEY NOT NULL CHECK (length(replica_id) BETWEEN 1 AND 128),
    content_id TEXT NOT NULL,
    publication_id TEXT,
    state TEXT NOT NULL CHECK (state IN ('active', 'paused', 'verification_failed', 'removed')),
    last_verified_at INTEGER,
    created_at INTEGER NOT NULL CHECK (created_at >= 0),
    updated_at INTEGER NOT NULL CHECK (updated_at >= 0),
    FOREIGN KEY (content_id) REFERENCES contents (content_id) ON DELETE RESTRICT,
    FOREIGN KEY (publication_id) REFERENCES publications (publication_id) ON DELETE SET NULL
) STRICT;

CREATE INDEX replicas_content_idx ON replicas (content_id);

CREATE TABLE peers (
    peer_id TEXT PRIMARY KEY NOT NULL CHECK (length(peer_id) BETWEEN 1 AND 128),
    address TEXT,
    last_seen_at INTEGER NOT NULL CHECK (last_seen_at >= 0),
    last_error TEXT,
    observation_count INTEGER NOT NULL DEFAULT 1 CHECK (observation_count >= 0)
) STRICT;

CREATE INDEX peers_last_seen_idx ON peers (last_seen_at DESC);
