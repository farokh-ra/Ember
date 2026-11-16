# Ember Project Handoff

**Current stage:** 09 - Peer identity, transports, and direct connections
**Stage status:** Complete
**Last updated:** 2026-11-16
**Next stage:** 10 - DHT publish and lookup (waiting for user permission)

## What was implemented

Stage 09 adds the authenticated transport and connection layer required by
ADR-0005, ADR-0007, and ADR-0008:

- A device peer key at `network/peer_key.enc`, encrypted under the local
  encryption key with HKDF-SHA256 and `DomainTag::PeerKey`.
- Device peer identity separated from the user signing identity. Identity backup
  and restore never read or replace the peer key.
- Pinned libp2p `0.57.1` with QUIC v1 and TCP + Noise + Yamux, DNS resolution,
  identify, ping, connection limits, and `/nexus/test/1` request/response.
- Authenticated direct connections and wrong-peer-identity rejection before any
  application protocol runs.
- A bounded canonical-CBOR transport-check schema with a 4 KiB payload cap and
  an 8 KiB frame cap.
- Network and peer state with separate listen, observed, and confirmed address
  classes. The core never claims public reachability without evidence.
- Persisted bootstrap and manual peers in `network/bootstrap.list`, bounded to
  64 entries and requiring `/p2p/<peer-id>`.
- Exponential reconnect backoff from 1 s to 60 s, network-change reset, and a
  bounded synchronous driver API.
- Connection, stream, request-rate, message, and idle-timeout bounds.
- Eight `network.*` core methods, network events, and domain-error mapping.
- A network start failure never fails core start; the core stays usable offline.
- 35 new Rust tests, for 201 total.

The Kotlin product screens still use visibly labelled mock data. DHT routing,
hosting, downloads, search, relay, hole punching, and mDNS remain later stages.

## Files changed

Network crate:

- `crates/nexus-net/Cargo.toml`
- `crates/nexus-net/src/lib.rs`
- `crates/nexus-net/src/behaviour.rs` (new)
- `crates/nexus-net/src/bootstrap.rs` (new)
- `crates/nexus-net/src/codec.rs` (new)
- `crates/nexus-net/src/config.rs` (new)
- `crates/nexus-net/src/driver.rs` (new)
- `crates/nexus-net/src/error.rs` (new)
- `crates/nexus-net/src/peer_key.rs` (new)
- `crates/nexus-net/src/shared.rs` (new)
- `crates/nexus-net/src/state.rs` (new)
- `crates/nexus-net/tests/stage09.rs` (new)

Core crate:

- `crates/nexus-core/Cargo.toml`
- `crates/nexus-core/src/engine.rs`
- `crates/nexus-core/src/error.rs`
- `crates/nexus-core/src/lib.rs`
- `crates/nexus-core/src/network.rs` (new)
- `crates/nexus-core/tests/stage09.rs` (new)

Protocol and crypto:

- `crates/nexus-protocol/src/error.rs`
- `crates/nexus-protocol/src/lib.rs`
- `crates/nexus-protocol/src/wire.rs` (new)
- `crates/nexus-crypto/src/domain.rs`

FFI and workspace:

- `crates/nexus-ffi/src/lib.rs`
- `crates/nexus-ffi/tests/c_abi.rs`
- `Cargo.toml`, `Cargo.lock`
- `deny.toml`

Documentation:

- `docs/architecture/FFI_INTERFACE.md`
- `docs/architecture/DEPENDENCY_LICENSE_INVENTORY.md`
- `docs/ARCHITECTURE.md`
- `docs/progress/STAGE_09.md` (new)
- `docs/HANDOFF.md`
- `README.md`, `CHANGELOG.md`

The untracked `LICENSE.md` is unrelated user work and is not part of this stage.

## Commands and results

### Native build

```text
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force
. .\scripts\dev-env.ps1 -Quiet
.\scripts\build-native.ps1 -Target All -Configuration Release
```

Result: PASS.

- Windows `build/native/windows-x64/nexus_core.dll` built and packaged.
- `libnexus_core.dll.a` regenerated and used by the C harness.
- Required Windows exports present; the C ABI harness passed.
- Android arm64-v8a and x86_64 `libnexus_core.so` built with libp2p/QUIC and
  export-checked.

### Rust tests

```text
cargo test --workspace
```

Result: PASS, 201 tests, 0 failures.

Stage 09-specific suites:

```text
cargo test -p nexus-net --test stage09 -- --test-threads=1
cargo test -p nexus-core --test stage09 -- --test-threads=1
```

Result: PASS, 8 and 4 tests respectively.

### Test breakdown

| Suite | Tests | Failures |
|---|---:|---:|
| `nexus-core` unit tests | 27 | 0 |
| `nexus-core` Stage 09 tests | 4 | 0 |
| `nexus-crypto` unit tests | 24 | 0 |
| `nexus-ffi` unit tests | 4 | 0 |
| `nexus-ffi` C ABI tests | 6 | 0 |
| `nexus-identity` unit tests | 9 | 0 |
| `nexus-identity` integration tests | 12 | 0 |
| `nexus-net` unit tests | 14 | 0 |
| `nexus-net` Stage 09 tests | 8 | 0 |
| `nexus-observability` unit tests | 3 | 0 |
| `nexus-protocol` unit tests | 21 | 0 |
| `nexus-protocol` signed-record tests | 9 | 0 |
| `nexus-protocol` Stage 07 tests | 8 | 0 |
| `nexus-store` unit tests | 17 | 0 |
| `nexus-store` local-import tests | 4 | 0 |
| `nexus-store` Stage 07 tests | 4 | 0 |
| `nexus-store` storage tests | 12 | 0 |
| `nexus-transfer` unit tests | 7 | 0 |
| `nexus-transfer` integration tests | 7 | 0 |
| `nexus-types` unit tests | 1 | 0 |
| **Rust total** | **201** | **0** |

### Formatting, linting, and dependency policy

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check
cargo audit
```

- Formatting and Clippy with warnings denied: PASS.
- `cargo deny check`: `advisories ok, bans ok, licenses ok, sources ok`. One
  unmaintained, non-vulnerable Linux-only `paste` advisory
  (`RUSTSEC-2024-0436`) is documented in `deny.toml` with removal guidance.
- `cargo audit`: 406 dependencies scanned against 1,509 advisories; no
  vulnerabilities and one allowed warning.
- The `Cargo.lock` resolves 406 packages and the dependency inventory is at
  version 1.6.0.

## Errors and failed attempts

1. A second transport connection to an already-connected peer was skipped.
   `DialOpts` now uses `PeerCondition::Always`.
2. `request_response::Behaviour::add_address` did not give the swarm a dial
   target. Bootstrap dials now call `Swarm::add_peer_address` before `dial`.
3. `multiaddr` 0.19 stores a parsed `PeerId` in `Protocol::P2p`; the code uses
   the typed value instead of rebuilding a multihash.
4. Network operational failures were mapped to `NEXUS_STATUS_INTERNAL`. They are
   now domain errors with `net_code`, while malformed field values stay
   ABI-level `INVALID_ARGUMENT`.
5. A network start failure initially failed `Engine::start`. It is now recorded
   and swallowed so the core stays offline-usable.
6. `notify_network_change` was fire-and-forget and untested. It now waits for the
   driver to apply the reset, and a loopback test proves prompt reconnection
   after accumulated backoff.
7. The new libp2p graph triggered the `paste` unmaintained advisory. The
   advisory is accepted with a documented, Linux-only, non-vulnerable
   justification in `deny.toml`.

## Decisions and unresolved questions

- Stage 09 was explicitly approved by the project owner.
- ADR-0005, ADR-0007, and ADR-0008 are implemented without architecture changes.
- libp2p is pinned exactly at `0.57.1`; `tokio` is `1.54.1` and `futures` is
  `0.3.34`.
- QUIC is preferred; TCP + Noise + Yamux is the fallback; DNS is enabled; mDNS
  is disabled in v1.
- The device peer key is separate from the user signing identity and is stored
  encrypted with `DomainTag::PeerKey`.
- `network_enabled` starts transports, but a networking failure never fails core
  start.
- Bootstrap and manual peers persist in `network/bootstrap.list`; every entry
  must include `/p2p`.
- Reconnect backoff is 1 s to 60 s by default and resets on a reported network
  change.
- Operational `network.*` failures are domain errors; malformed arguments are
  ABI-level.
- The ABI remains 1; the protocol version is `0.1.0-stage-09`.
- Android runtime networking evidence is deferred because no emulator or device
  is available. The Android libraries cross-build and export-check.
- The Android lifecycle hook for Wi-Fi switches and sleep/wake is not bound yet.
- Remote GitHub Actions execution has not run for Stage 09.

## Exact next action

1. Review `docs/progress/STAGE_09.md`, the updated
   `docs/architecture/FFI_INTERFACE.md`, and the `nexus-net`/`nexus-core`
   Stage 09 tests.
2. Give explicit approval to start Stage 10.
3. Stage 10 adds Kademlia peer routing, bounded record publication and lookup,
   multi-bootstrap failover, and lookup timeouts on top of these transports. Do
   not begin it before that approval.
