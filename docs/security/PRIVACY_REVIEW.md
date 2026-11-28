# Ember Privacy Review

**Status:** Reviewed for Stage 18; no production release yet
**Last reviewed:** 2026-11-26
**Scope:** Ember `0.1.x` development builds for Android and Windows
**Related requirements:** FR-SEC-03, FR-SEC-07, FR-SEC-08, FR-SEC-11,
FR-DATA-06, FR-ID-05

## 1. Summary

Ember stores user data locally, publishes only what the user explicitly signs,
and has no telemetry, analytics, advertising, account system, or automatic crash
reporting. The main privacy exposure is inherent to a peer-to-peer network:
published records are public, peers learn network addresses and peer IDs needed
to communicate, and a network observer can see that a device participates.

This review covers what Ember collects, where it goes, how long it is kept, and
which statements the product must not make.

## 2. Data inventory

| Data | Classification | Stored locally | Shared with peers | Public | Notes |
|---|---|---|---|---|---|
| Identity signing key | Secret | Encrypted vault, outside SQLite | No | No | Export only through an explicit encrypted backup |
| Device peer key | Secret | Encrypted file | No | No | Public key is presented during transport authentication |
| Identity backup | Secret | Only the file the user chooses to save | No | No | Passphrase-derived encryption; bytes returned once |
| Profile record | Public by user action | Yes | Yes, when requested or announced | Yes | Contains display name, username, bio, avatar reference |
| Publication record and manifest | Public by user action | Yes | Yes | Yes | Title, description, tags, size, content ID |
| Published file bytes | Public by user action | Yes | Yes, while serving | Yes | Only while the user hosts or seeds it |
| Local import metadata | Sensitive personal | Yes | No | No | Local file names and staged paths stay local |
| Downloads and partial files | Sensitive personal | Yes | No | No | Peer sources are recorded locally |
| Peer observations | Sensitive personal | Yes | No | No | Peer ID, address, last seen, bounded error text |
| Block list | Sensitive personal | Yes | No | No | Peer ID, reason, bounded note, timestamps |
| Abuse reports | Sensitive personal | Yes | No | No | Peer ID, category, bounded note, timestamp |
| Settings | Operational | Yes | No | No | Theme, locale, quota, relay and bootstrap configuration |
| Logs | Operational or sensitive | Local only | No | No | Debug identifiers are opt-in and redacted |
| Network addresses | Sensitive personal | In memory; observed peers also stored | As required to connect | No | Visible to peers and to a local network observer |

## 3. What is public by design

Signing and publishing are explicit user actions. Once published:

- The signed profile and publication records can be requested by any peer and
  may be announced through the DHT.
- The manifest and file bytes can be downloaded by any peer while the user is
  serving them.
- A publication may remain available after the user stops serving it if another
  peer already holds and serves the bytes. Ember cannot retract copies held by
  others.
- The UI must say this before the first publish and repeat it in the privacy
  section. `security.notices` exposes the same public-visibility warning.

Ember does **not** claim anonymity. Peer IDs, network addresses, request timing,
and content interests may be observable to other participants and to a local
network observer.

## 4. What stays local

- Identity keys, the device peer key, and backup passphrases never leave the
  device except in the explicit encrypted backup the user creates.
- Block list and abuse reports are local policy. They are never uploaded,
  announced, gossiped, or written into signed records, and no central moderation
  authority receives them.
- Observed peer addresses and error text are stored only to operate the network
  and diagnose local failures.
- Downloads, partial files, and local import metadata stay in app-managed
  storage.
- Logs are written locally. There is no log upload path.

A metadata export is a consistent snapshot of the local SQLite database, taken
only when a user explicitly requests it. Because it is a full snapshot, it
includes local security rows such as the block list, abuse reports, peer
observations, and peer sources. It excludes keys, passphrases, backup contents,
and file bytes. The export is a private file and must be treated as sensitive.
Diagnostics export, when implemented, must redact the same data and must not
include security rows by default.

## 5. Telemetry and third parties

| Channel | Status |
|---|---|
| Analytics or advertising SDK | None |
| Automatic crash reporting | None |
| Remote logging | None |
| Push notification service | None |
| Central account or identity service | None |
| Update check | None in the core |
| Bootstrap or relay operators | Receive ordinary peer connections; no account or profile data is sent deliberately |
| Platform app store | Receives install and crash data only under the platform's own policy, outside Ember's control |

Any future telemetry or crash reporting must be opt-in, minimised, documented,
separable from core functionality, and must exclude keys, passphrases, backup
contents, local paths, file names, and unnecessary peer identifiers. There is no
mechanism today, and adding one requires a new privacy review. The unimplemented
opt-in network-health design is in
[NETWORK_HEALTH.md](../operations/NETWORK_HEALTH.md).

## 6. Logging and diagnostics

The logging policy (`docs/LOGGING_POLICY.md`) classifies secrets, sensitive
personal data, operational data, and public build data. Ember follows it:

- Secrets are never logged at any level.
- Peer IDs, addresses, file names, and local paths are debug-only and redacted
  in user-shared output.
- Errors carry stable codes and safe messages.
- Event payloads contain identifiers and counts, not file bytes, keys, or
  passphrases.
- Block and report notes are user-authored and may contain personal data; they
  are returned only to the local UI and are never logged.

Stage 18 re-checked the Rust error and event paths for paths, keys, and peer
identifiers. No new log statement or error message introduced in Stage 18
includes a secret, a local path, or an unbounded note.

## 7. Retention and deletion

| Data | Retention | Deletion |
|---|---|---|
| Profile and publication records | Until the user deletes them or the store is reset | User action in the owning feature |
| Published file bytes | Until the user stops serving or deletes the file | User action |
| Downloads and partials | Until completed, cancelled, or cleaned up | User action and bounded cleanup |
| Peer observations | Bounded by table policy and observation pruning | Automatic pruning |
| Block list | Until the user unblocks the peer | `security.block_remove` |
| Abuse reports | Until the local store is reset; reports are capped but not individually deletable yet | No UI deletion yet; deferred |
| Logs | Local, user-clearable; no automatic upload | Local file removal |
| Identity backup | Only where the user saved it | User-managed |

The missing per-report deletion control is recorded as a residual privacy item.
It does not create a network disclosure, because reports never leave the device,
but a user cannot currently remove one report without clearing the store.

## 8. Permissions and platform exposure

- Network access is required for peer-to-peer operation and is user-controlled
  through the network switch.
- File access uses the platform picker or app-managed storage; Ember does not
  request broad filesystem access.
- Notifications, background execution, and battery optimisation are platform
  concerns documented with the lifecycle design. They do not carry content.
- Android backup of app data is a platform feature. Ember does not add its own
  cloud backup, and the identity vault and peer key are encrypted at rest.
  Platform backup behaviour must be reviewed on a real device before release.
- Windows stores data under the app's local data directory. No registry or
  cloud sync is used by the core.

## 9. Findings and actions

| Finding | Severity | Action |
|---|---|---|
| No per-report deletion control | Low | Deferred; document until a delete method exists |
| Metadata export includes local security rows | Low | Documented as a private full snapshot; diagnostics must exclude them |
| Peer addresses are inherently visible to peers | Medium | Accepted; no anonymity claim; redacted in diagnostics |
| Android platform backup behaviour unverified | Medium | Add to the release checklist; verify on a device |
| No independent privacy or security review | Medium | Required before broad public release |

## 10. Product statements that must remain true

- “No telemetry” means no analytics, advertising, or automatic crash upload.
- “Local blocking” means the block list is enforced locally and never uploaded;
  it does not mean the blocked peer is removed from the network.
- “Public profile” and “published file” mean any peer may fetch and keep a copy.
- “Encrypted backup” means the backup is encrypted with the user's passphrase;
  losing the passphrase loses the identity.
- “No central recovery” means Ember cannot restore an identity for the user.
