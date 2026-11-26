# Ember Project Handoff

**Current stage:** 18 - security, abuse resistance, and privacy hardening
**Stage status:** Complete on Windows loopback; review and merge pending
**Last updated:** 2026-11-26
**Branch:** `feat/18-security`, based on `feat/16-search` at `8abaa50`;
stacked pull request #25 targets `feat/16-search`
**Next stage:** 19 - performance, compatibility, and release engineering
(requires explicit project-owner approval)

## What was implemented

Stage 18 hardens the existing product without adding a new user-facing feature:

- A 34-entry threat model covering identity and key theft, fake records, Sybil
  behaviour, DHT poisoning, malicious peers, spam, resource exhaustion, unsafe
  files, privacy, local device access, and supply chain.
- A privacy review with a data inventory, public-versus-local data, telemetry
  status, logging rules, retention, platform permissions, and findings.
- A security release checklist with an explicit high-severity release block.
- Schema version 11 with `blocked_peers` and `abuse_reports`. Both tables have
  no foreign key to observed peers, so moderation survives observation pruning.
- Store caps from `StoreConfig`: 4096 blocks and 4096 reports by default,
  configurable from 1 to 65,536 and enforced inside the writer transaction.
- Bounded notes (512 bytes, truncated on a UTF-8 boundary), validated peer IDs,
  reason and category constraints, and truncated peer observations.
- `nexus-net` block-list state and enforcement on outbound dial, inbound
  connection, inbound test/file/index requests, discovery results, and
  providers. A blocked peer fails with `peer_blocked`.
- Core methods `0x0C01`-`0x0C06`: block list, block add, block remove, report
  add, report list, and notices. `system.info` advertises all six.
- Blocked-source filtering in `download.start` and `download.resume`, with
  `permission_denied` when every source is blocked.
- Advisory safe-file classification, visible warnings, `auto_execute: false`,
  and `preview: "delegated"` on download events and listings.
- Standing notices for no automatic execution, delegated previews, key-loss
  risk, public visibility, and local-only blocking.
- Truncation and single-byte mutation tests for file protocol messages, plus
  malformed and oversized request tests in the core.
- 19 new Rust tests, for 372 total. Existing Stage 17 behavior remains passing.

## Files changed

Store crate:

- `crates/nexus-store/migrations/0011_security_hardening.sql` (new)
- `crates/nexus-store/src/migrations.rs`
- `crates/nexus-store/src/model.rs`
- `crates/nexus-store/src/store.rs`
- `crates/nexus-store/src/lib.rs`
- `crates/nexus-store/tests/stage18.rs` (new)
- `crates/nexus-store/tests/storage.rs`, `profiles.rs`, `local_import.rs`
  (schema assertion only)

Network crate:

- `crates/nexus-net/src/shared.rs`
- `crates/nexus-net/src/driver.rs`
- `crates/nexus-net/src/error.rs`
- `crates/nexus-net/src/lib.rs`

Core crate:

- `crates/nexus-core/src/security.rs` (new)
- `crates/nexus-core/src/safety.rs` (new)
- `crates/nexus-core/src/engine.rs`
- `crates/nexus-core/src/downloads.rs`
- `crates/nexus-core/src/network.rs`
- `crates/nexus-core/src/lib.rs` (modules and protocol version)
- `crates/nexus-core/tests/stage18.rs` (new)

Protocol crate:

- `crates/nexus-protocol/src/file.rs` (malformed-input tests only)

Documentation:

- `docs/security/THREAT_MODEL.md` (new)
- `docs/security/PRIVACY_REVIEW.md` (new)
- `docs/security/SECURITY_RELEASE_CHECKLIST.md` (new)
- `docs/progress/STAGE_18.md` (new)
- `docs/architecture/FFI_INTERFACE.md` (version 1.11.0)
- `docs/architecture/FILE_TRANSFER.md`
- `docs/ARCHITECTURE.md`
- `SECURITY.md`
- `docs/HANDOFF.md`
- `README.md`, `CHANGELOG.md`

No Kotlin, FFI adapter, or new network protocol changed in Stage 18.

## Commands and results

### Rust tests

```text
cargo test --workspace
```

Result: PASS, 372 tests, 0 failures.

Stage 18-specific suites:

```text
cargo test -p nexus-store --test stage18
cargo test -p nexus-core --test stage18
```

Result: PASS, 5 and 7 tests respectively.

### Test breakdown

| Suite | Tests | Failures |
|---|---:|---:|
| `nexus-core` unit tests | 69 | 0 |
| `nexus-core` Stage 09 tests | 4 | 0 |
| `nexus-core` Stage 10 tests | 2 | 0 |
| `nexus-core` Stage 11 tests | 4 | 0 |
| `nexus-core` Stage 12 tests | 1 | 0 |
| `nexus-core` Stage 13 tests | 1 | 0 |
| `nexus-core` Stage 14 tests | 1 | 0 |
| `nexus-core` Stage 15 tests | 3 | 0 |
| `nexus-core` Stage 16 tests | 4 | 0 |
| `nexus-core` Stage 17 tests | 2 | 0 |
| `nexus-core` Stage 18 tests | 7 | 0 |
| `nexus-crypto` unit tests | 24 | 0 |
| `nexus-ffi` unit tests | 4 | 0 |
| `nexus-ffi` C ABI tests | 6 | 0 |
| `nexus-identity` unit tests | 9 | 0 |
| `nexus-identity` integration tests | 12 | 0 |
| `nexus-net` unit tests | 23 | 0 |
| `nexus-net` Stage 09 tests | 8 | 0 |
| `nexus-net` Stage 10 tests | 4 | 0 |
| `nexus-net` Stage 11 tests | 5 | 0 |
| `nexus-net` Stage 12 tests | 2 | 0 |
| `nexus-observability` unit tests | 3 | 0 |
| `nexus-protocol` unit tests | 39 | 0 |
| `nexus-protocol` signed-record tests | 9 | 0 |
| `nexus-protocol` Stage 07 tests | 8 | 0 |
| `nexus-search` unit tests | 27 | 0 |
| `nexus-store` unit tests | 24 | 0 |
| `nexus-store` local-import tests | 4 | 0 |
| `nexus-store` profile tests | 5 | 0 |
| `nexus-store` search tests | 4 | 0 |
| `nexus-store` Stage 07 tests | 4 | 0 |
| `nexus-store` Stage 14 tests | 5 | 0 |
| `nexus-store` Stage 15 tests | 2 | 0 |
| `nexus-store` Stage 18 tests | 5 | 0 |
| `nexus-store` storage tests | 14 | 0 |
| `nexus-transfer` unit tests | 16 | 0 |
| `nexus-transfer` integration tests | 7 | 0 |
| `nexus-types` unit tests | 1 | 0 |
| **Rust total** | **372** | **0** |

Stage 17 had 353 tests. Stage 18 adds 7 core integration tests, 5 store
integration tests, 2 core unit tests for safety, 2 core unit tests for security
value shaping, 1 network unit test, 1 protocol unit test, and 1 migration test.

### Kotlin and native builds

No Kotlin, FFI adapter, or native symbol changed in Stage 18, so the Stage 16
Kotlin and native build gates were not re-run. The Stage 16 results still
describe the current Kotlin and native code. The core library and all Rust
targets build and test with the pinned toolchain.

### Formatting, linting, and dependency policy

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
python scripts/check_repo_hygiene.py
python scripts/validate_stage01_docs.py
python scripts/validate_stage02_docs.py
cargo deny check
cargo audit
```

- Formatting, Clippy with warnings denied, repository hygiene, and both
  documentation validators: PASS.
- `cargo deny check`: PASS (advisories, bans, licenses, and sources).
- `cargo audit`: PASS with the documented allowed `paste` unmaintained warning.
- No dependency added; `Cargo.lock` remains at 411 packages.
- The FFI interface document is at version 1.11.0 with fifty-one callable
  methods.
- The ABI remains 1; the core protocol version is `0.1.0-stage-18`. The
  identify protocol remains `/nexus/0.1.0-stage-12`; Stage 18 adds no new
  network protocol or frame type.

## Errors and failed attempts

1. The first core Stage 18 test used a 64 MiB quota, below the core's 1 GiB
   minimum, so engine creation failed. The test config now uses 1 GiB.
2. Category and note validation initially surfaced as a raw `CoreError` instead
   of a domain response. The test now asserts the real contract: parameter
   errors return `InvalidArgument`, and store ID errors return `Store`.
3. The first workspace build could not find the MinGW linker because the PATH
   pointed at the toolchain root instead of `mingw64\bin`. The corrected path
   builds and links every test.
4. Clippy rejected a useless `vec!` in the new protocol test and a needless
   borrow in the new store test. Both were fixed without relaxing the gate.
5. `cargo fmt --check` initially failed on the new download safety fields, the
   network error mapping, and the safety extension table. They were formatted.
6. A previous attempt to read and rewrite UTF-8 test files with PowerShell text
   cmdlets mangled non-ASCII data and added a BOM. This stage uses the file edit
   tools for UTF-8 content.

## Decisions and unresolved questions

- Blocking and reporting are local by design. There is no central moderation
  authority, no automatic upload, and no gossip of moderation data.
- Security tables do not reference observed peers, so a block survives
  observation pruning.
- New tables are not required by the archive validator, preserving restore of
  pre-Stage-18 archives; migration 11 creates them empty.
- The FFI namespace is `0x0C`, leaving `0x0A` reserved for settings and
  diagnostics.
- File safety classification is advisory and has no execution path.
- Enforcement lives in the network layer and in download source selection, not
  only in the UI.
- A blocked peer is refused at request time and on new connections; libp2p's
  native allow/block list is not used.
- A metadata export is a full local database snapshot and includes security
  rows. It is documented as private; diagnostics export must exclude them.
- Per-report deletion, Kotlin UI binding for safety and notices, Android runtime
  evidence, independent security review, and fuzzing remain open.
- Sybil resistance remains partial and is documented as accepted residual risk.

## Exact next action

1. Review the stacked Stage 18 pull request #25 into `feat/16-search`.
2. Review PR #24 from `feat/16-search` into `main`; it carries Stages 16 and 17.
3. Merge only after review, in stack order, then delete merged branches.
4. Do not start Stage 19 without explicit project-owner approval.
5. Stage 19 covers performance, compatibility, fuzzing, installers, signed
   releases, and final release evidence.
