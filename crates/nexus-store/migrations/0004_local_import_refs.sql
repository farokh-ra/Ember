-- Stage 08 streaming imports are retained by a local-import reference until a
-- publication, replica, or download takes ownership of the bytes. SQLite
-- cannot widen a CHECK constraint in place, so the small reference table is
-- rebuilt in the migration transaction.

CREATE TABLE content_refs_new (
    ref_id INTEGER PRIMARY KEY AUTOINCREMENT,
    content_id TEXT NOT NULL,
    ref_type TEXT NOT NULL CHECK (
        ref_type IN ('publication_original', 'active_replica', 'retained_download', 'local_import')
    ),
    owner_id TEXT NOT NULL CHECK (length(owner_id) BETWEEN 1 AND 128),
    created_at INTEGER NOT NULL CHECK (created_at >= 0),
    UNIQUE (content_id, ref_type, owner_id),
    FOREIGN KEY (content_id) REFERENCES contents (content_id) ON DELETE RESTRICT
) STRICT;

INSERT INTO content_refs_new (ref_id, content_id, ref_type, owner_id, created_at)
SELECT ref_id, content_id, ref_type, owner_id, created_at FROM content_refs;

DROP TABLE content_refs;

ALTER TABLE content_refs_new RENAME TO content_refs;

CREATE INDEX content_refs_content_idx ON content_refs (content_id);
CREATE INDEX content_refs_owner_idx ON content_refs (ref_type, owner_id);
