-- Stage 12: remember where a resumable download came from so a restart can
-- resume without guessing a source or re-fetching an unauthenticated manifest.
ALTER TABLE downloads ADD COLUMN manifest_root TEXT;
ALTER TABLE downloads ADD COLUMN source_peer TEXT;
