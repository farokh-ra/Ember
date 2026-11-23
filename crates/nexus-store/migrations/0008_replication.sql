-- Stage 15: explicit replica hosting, verification history, and availability.
--
-- A replica row is created only by an explicit host action. The row keeps the
-- original publication ID as attribution; it never rewrites or replaces the
-- signed publication record. Removed rows are retained so the same content can
-- be re-hosted later without losing its audit trail.

ALTER TABLE replicas ADD COLUMN source_peer TEXT
    CHECK (source_peer IS NULL OR length(source_peer) BETWEEN 1 AND 128);
ALTER TABLE replicas ADD COLUMN verified_size_bytes INTEGER
    CHECK (verified_size_bytes IS NULL OR verified_size_bytes >= 0);
ALTER TABLE replicas ADD COLUMN last_checked_at INTEGER
    CHECK (last_checked_at IS NULL OR last_checked_at >= 0);
ALTER TABLE replicas ADD COLUMN last_seen_at INTEGER
    CHECK (last_seen_at IS NULL OR last_seen_at >= 0);
ALTER TABLE replicas ADD COLUMN failure_count INTEGER NOT NULL DEFAULT 0
    CHECK (failure_count >= 0);
ALTER TABLE replicas ADD COLUMN last_error TEXT
    CHECK (last_error IS NULL OR length(last_error) <= 512);
ALTER TABLE replicas ADD COLUMN announced INTEGER NOT NULL DEFAULT 0
    CHECK (announced IN (0, 1));

CREATE INDEX replicas_state_idx ON replicas (state);
