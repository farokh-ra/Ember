# ADR-0010 — Search and Explore use a verified local index, never a central service

- **Status:** Accepted
- **Date:** 2026-11-12
- **Deciders:** NEXUS engineering
- **Supersedes:** none
- **Related:** ADR-0002, ADR-0004, ADR-0005, ADR-0007, ADR-0008

## Context

NEXUS users need to find profiles and files by username/display name, filename, category, and tags. Distributed hash tables support exact key lookup, not arbitrary full-text search. A central search service would be simple but violates the product's no-central-backend rule and creates a moderation and availability authority. A global distributed full-text index is expensive, privacy-sensitive, and cannot promise complete results.

The product requires honest partial results within a bounded time (target 10 seconds), coverage/freshness labels, cached offline results, deduplication, and conflict visibility. Explore must show categories, recent records, and random discovery without likes, follows, or popularity ranking.

## Decision

Use a **derived local index** as the primary search source, plus optional peer index providers for broader coverage. There is no global index and no company-operated search service.

### Local index

- The core maintains a SQLite FTS5 index over **verified** profile and publication records.
- Only records that passed schema and signature validation are indexed. Quarantined or invalid records never appear.
- Indexed fields: display name, filename, description, category, tags, and the short identity ID. Description text is bounded before indexing.
- FTS5 uses the `unicode61` tokenizer with diacritic removal. Query input is bounded to 128 characters, tokenized, and rate-limited.
- Search tables are derived state and can be rebuilt from stored signed records at any time.
- Local results are available offline and are labelled with their last refresh time.

### Explore

- Explore queries the local verified index for categories, recent records, and random records.
- Random discovery uses a bounded randomized query, not popularity or engagement.
- Results show publication existence separately from observed availability. A record being indexed does not mean a source is online.
- No likes, comments, follows, follower counts, or popularity ranking exist in the schema or UI.

### Distributed search

- v1 may query **optional peer index providers** discovered through the DHT. An index provider is a peer that chooses to answer bounded index queries; it is not an authority.
- The query fans out to a bounded number of providers (initial target: at most 8) with a 10-second total deadline. Local results appear immediately; remote results stream in as `search.result_page` events; a terminal `search.completed` event carries coverage and freshness.
- Provider responses contain signed publication/profile records or references to them. The core verifies every record before display, regardless of the provider's reputation.
- Duplicate records are deduplicated by publication ID and grouped by content ID. Conflicting revisions or metadata remain visible as conflicts.
- Coverage metadata records which sources succeeded, which timed out or failed, and whether results include cached data. The UI labels reduced coverage honestly.
- If no index provider is available, search is local-only and says so. It does not fabricate network coverage.
- Index provider protocol IDs carry a major version. Unknown versions are ignored with a coverage note.
- Index providers cannot mutate local records, suppress local results, or grant verification.

### Boundaries

- Search never parses raw network messages. It receives verified records through `nexus-protocol` and reads the local index through `nexus-store`.
- Search never returns a record to the UI without a verification state.
- DHT exact lookup is not search. It resolves a known publication, profile, or provider key; it does not answer text queries.
- Search pagination uses a stable cursor tied to the query and index snapshot; duplicates are suppressed across pages.
- Search does not require a network. Offline startup and local search work without waiting for timeouts.

### Abuse and resource limits

- Query length, token count, result page size, provider fan-out, response size, and per-peer request rate are bounded.
- Malformed, oversized, or unsigned provider responses are discarded without affecting local or other-provider results.
- Search never executes downloaded content or renders unsanitized HTML. Metadata is rendered as plain text with safe truncation.

## Consequences

**Positive**

- Search works offline and without any central service.
- Local results are fast and verifiable; the 500 ms local-result target is achievable.
- Optional index providers improve coverage without becoming authorities.
- Derived indexes can be rebuilt and do not risk canonical record data.
- Coverage and conflict labels match the product's honesty rules.

**Negative / trade-offs**

- Distributed coverage is inherently partial; the UI must educate users.
- Index providers add protocol, abuse, and privacy surface.
- FTS5 increases database size and migration complexity.
- Provider discovery and ranking require Stage 16 design work.
- No global ranking means search relevance is local and provider-dependent.

**Constraints**

- No central search database, account, or hidden index service.
- No search result is displayed as verified without a signature check.
- No popularity, engagement, or social signal may influence ranking.
- No provider may be treated as an identity or ownership authority.
- Local search must remain available when the network is offline.

## Alternatives considered

1. **Central search service.** Best relevance and completeness, but violates the product rules and creates a single point of control. Rejected.
2. **Full-text data in the DHT.** Extremely expensive, privacy-leaking, and unsuitable for mutable indexes. Rejected.
3. **Gossip-based global index.** More decentralized, but unbounded storage, spam, and consistency problems for v1. Deferred.
4. **Local-only search with no distributed providers.** Simplest and honest, but weakens discovery and the P0 search journey. Kept as the offline fallback, not the only mode.
5. **IPFS/IPNI-style external index.** Adds a large external dependency and a different record model. Rejected.
6. **Rank by provider popularity or peer reputation.** Invites Sybil manipulation and violates the no-popularity rule. Rejected.

## Verification

- Stage 16 tests local search latency with 10,000 records, FTS rebuild, offline search, partial provider results, provider loss, malformed responses, duplicate suppression, conflict display, pagination, and the 10-second deadline.
- Stage 16 proves that invalid or unsigned records never enter the index or results.
- Stage 19 measures search performance on the baseline devices and reviews coverage labels with accessibility testing.
