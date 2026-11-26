-- Stage 18: local abuse controls. Blocking is a local policy decision and is
-- never published, so no central moderation authority is required. Reports are
-- local observations for the user; they are not uploaded automatically.
CREATE TABLE blocked_peers (
    peer_id TEXT PRIMARY KEY NOT NULL CHECK (length(peer_id) BETWEEN 1 AND 128),
    reason TEXT NOT NULL CHECK (reason IN ('manual', 'spam', 'malware', 'impersonation', 'harassment', 'scam', 'other')),
    note TEXT CHECK (note IS NULL OR length(CAST(note AS BLOB)) <= 512),
    created_at INTEGER NOT NULL CHECK (created_at >= 0),
    updated_at INTEGER NOT NULL CHECK (updated_at >= 0)
) STRICT;

CREATE INDEX blocked_peers_created_idx ON blocked_peers (created_at DESC);
CREATE INDEX blocked_peers_reason_idx ON blocked_peers (reason);

CREATE TABLE abuse_reports (
    report_id INTEGER PRIMARY KEY AUTOINCREMENT,
    peer_id TEXT NOT NULL CHECK (length(peer_id) BETWEEN 1 AND 128),
    category TEXT NOT NULL CHECK (category IN ('spam', 'malware', 'impersonation', 'harassment', 'scam', 'other')),
    note TEXT CHECK (note IS NULL OR length(CAST(note AS BLOB)) <= 512),
    created_at INTEGER NOT NULL CHECK (created_at >= 0)
) STRICT;

CREATE INDEX abuse_reports_peer_idx ON abuse_reports (peer_id, created_at DESC);
CREATE INDEX abuse_reports_created_idx ON abuse_reports (created_at DESC);
