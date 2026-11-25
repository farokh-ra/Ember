-- Stage 17: record per-chunk completion so one download can combine verified
-- chunks from several peers, arrive out of order, and resume across peers.
--
-- `verified_prefix` stays the longest contiguous verified prefix for legacy
-- callers. `verified_bytes` is the quota charge and may exceed the prefix when
-- later chunks are already verified. `chunk_map` is a little-endian bit per
-- 1 MiB chunk; NULL means a pre-Stage-17 record that only has a prefix.
ALTER TABLE downloads ADD COLUMN verified_bytes INTEGER NOT NULL DEFAULT 0 CHECK (verified_bytes >= 0);
ALTER TABLE downloads ADD COLUMN chunk_map BLOB;

UPDATE downloads SET verified_bytes = verified_prefix;
