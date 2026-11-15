# Ember --- Complete Development Roadmap

## From 0% to a production-ready, global, decentralized file-discovery and sharing network

**Document status:** Master development plan\
**Target platforms:** Android (`.apk` / `.aab`) and Windows (`.exe`)\
**Primary languages:** Kotlin, Rust, SQL\
**UI framework:** Compose Multiplatform\
**P2P foundation:** Rust + libp2p\
**Local database:** SQLite\
**Architecture goal:** One shared product, with native Android and
Windows applications and a shared Rust networking core.

------------------------------------------------------------------------

# 1. Project vision

Ember is a decentralized application for discovering people and publicly
shared files, publishing files from a user's own device, and downloading
files directly from other participating devices.

There is no company-owned central server, central cloud storage, or
mandatory cloud account. Devices participate as peers. Users can
voluntarily keep copies of files to make them available to others.

The intended product is a global network, not merely a two-device
transfer utility.

## 1.1 Core product experience

A user should be able to:

-   Create and control an Ember identity.
-   Set up one public profile containing their published files.
-   Browse other users' profiles.
-   Publish public files with descriptions, categories and tags.
-   Explore categories and discover random or recently published files.
-   Search for users and files.
-   Open a file page and see its details, verification status and known
    availability.
-   Download a file directly from one or more peers.
-   Resume interrupted downloads.
-   Voluntarily replicate selected public files.
-   See which files they are currently hosting for the network.
-   Preserve original-uploader attribution when a file is replicated.
-   Publish a modified file as a distinct file or version linked to its
    source.
-   Use the same Ember network from Android and Windows.

## 1.2 Product rules already decided

These are product requirements, not optional implementation suggestions:

1.  **All published files are public.** There is no private-file sharing
    in the planned product.
2.  **No company-owned central server.** The network must not depend on
    a company-operated central database, file host or mandatory cloud
    service.
3.  **No mandatory replication.** Users decide whether to store and
    serve copies.
4.  **Replication does not transfer authorship.** A replica remains
    attributed to the original uploader.
5.  **Attribution must be verifiable.** The original publication record
    is signed by the publishing identity; it is not merely editable
    text.
6.  **Modified files are distinct.** A modified file receives its own
    content identity and can reference its source.
7.  **File integrity is verified.** A downloaded file must be checked
    against its published content identity.
8.  **No likes, comments or follow system.** Discovery is based on
    profiles, files, categories and search.
9.  **Android and Windows are first-class platforms.** Neither is a
    reduced web wrapper.
10. **The architecture is designed for a global network.** Early
    releases may have few peers, but they must not be designed as a
    two-user-only product.

## 1.3 Important reality checks

-   Decentralized does not mean that peers are always online. A file
    with no online source cannot be downloaded at that moment.
-   A network with no reachable bootstrap peers, relays or other known
    peers can be difficult for a new installation to join. Ember should
    support multiple independently operated community bootstrap options,
    not rely on one company endpoint.
-   NAT traversal is not guaranteed on every network. Some connections
    will require a relay or will fail.
-   Distributed search cannot promise perfectly complete, instantly
    consistent global results. The UI must communicate freshness and
    availability honestly.
-   "No company server" does not mean "no infrastructure exists
    anywhere." Community-operated bootstrap nodes and optional relays
    may be used without becoming central authorities or file custodians.
-   The application can choose not to implement copyright enforcement,
    but software design cannot eliminate legal obligations that may
    apply to users or operators.
-   No architecture can guarantee millions of users merely by being
    decentralized. Scalability must be measured through load tests, real
    network tests and staged adoption.

------------------------------------------------------------------------

# 2. Technical direction

## 2.1 Recommended stack

  -----------------------------------------------------------------------
  Area                    Technology              Responsibility
  ----------------------- ----------------------- -----------------------
  Android app             Kotlin + Compose        Native Android UI and
                          Multiplatform           platform integration

  Windows app             Kotlin + Compose        Desktop UI and Windows
                          Multiplatform           integration

  Shared application      Kotlin Multiplatform    Shared screens, state,
  logic                                           workflows and domain
                                                  logic

  P2P engine              Rust                    Peer networking,
                                                  discovery, transfer
                                                  engine and protocol
                                                  implementation

  P2P framework           rust-libp2p             Peer identity,
                                                  transports, DHT and
                                                  peer-to-peer protocols

  Android ↔ Rust bridge   JNI / Android Native    Call Rust core from
                          Interface, wrapped      Android
                          behind Kotlin APIs

  Windows ↔ Rust bridge   Stable C ABI with a     Call the same Rust core
                          Kotlin/JVM native       from Windows
                          binding layer

  Local database          SQLite                  Profiles, metadata,
                                                  search cache, downloads
                                                  and replication state

  File integrity          SHA-256 initially;      Verify content and
                          chunk hashes / Merkle   chunks
                          structure for large
                          files

  Identity signatures     Ed25519                 Sign profile and
                                                  publication records

  Build automation        Gradle + Cargo          Build Kotlin and Rust
                                                  components

  Source control          Git                     Version history and
                                                  collaboration

  Automated testing       Kotlin test tools +     Unit, integration and
                          Rust test tools         system tests
  -----------------------------------------------------------------------

**Important integration decision:** Kotlin and Rust are separate
language runtimes. The project must define a small, versioned interface
between them. Do not let UI code call arbitrary low-level networking
functions. Expose stable operations such as `startNode`, `connect`,
`publishRecord`, `findPeers`, `downloadFile`, `pauseDownload` and
`getNetworkStatus`.

## 2.2 Application architecture

``` text
                    Ember APPLICATION
             ┌─────────────────────────────┐
             │       Compose UI            │
             │  Android / Windows screens  │
             └──────────────┬──────────────┘
                            │
             ┌──────────────▼──────────────┐
             │    Kotlin shared layer      │
             │ state, workflows, domain    │
             └──────────────┬──────────────┘
                            │ stable FFI API
             ┌──────────────▼──────────────┐
             │       Rust Ember Core       │
             │ identity · DHT · P2P        │
             │ discovery · transfer        │
             │ verification · replication  │
             └───────┬─────────────┬───────┘
                     │             │
             ┌───────▼─────┐ ┌─────▼────────┐
             │ SQLite      │ │ Local files  │
             │ metadata    │ │ cache/store  │
             └─────────────┘ └──────────────┘
                     │
             Internet / other Ember peers
```

The diagram represents logical components. SQLite and the file store
remain local to each installation. The Rust core communicates with other
peers over supported network transports.

## 2.3 Suggested repository layout

``` text
nexus/
├── apps/
│   ├── androidApp/
│   └── desktopApp/
├── shared/
│   ├── commonMain/
│   ├── androidMain/
│   └── desktopMain/
├── core-rust/
│   ├── crates/
│   │   ├── nexus-core/
│   │   ├── nexus-identity/
│   │   ├── nexus-protocol/
│   │   ├── nexus-discovery/
│   │   ├── nexus-transfer/
│   │   ├── nexus-storage/
│   │   └── nexus-ffi/
│   └── Cargo.toml
├── database/
│   ├── migrations/
│   └── schema/
├── docs/
│   ├── architecture/
│   ├── protocol/
│   ├── security/
│   └── decisions/
├── tests/
│   ├── integration/
│   ├── network/
│   └── performance/
├── scripts/
├── .github/
│   └── workflows/
├── README.md
├── CONTRIBUTING.md
└── LICENSE
```

The exact layout can change after the initial build-system
investigation, but the boundaries between UI, shared logic, Rust core,
local storage and tests should remain clear.

------------------------------------------------------------------------

# 3. Non-negotiable engineering principles

1.  **Local-first:** The app remains useful when disconnected. Local
    profile data, published-file records, downloads and replication
    choices are stored locally.
2.  **No central dependency:** A company-operated server must not be
    required for identity, profiles, discovery, file storage or
    downloads.
3.  **Explicit user control:** Replication, storage limits, upload
    bandwidth and background activity are user-controlled.
4.  **Verify before trust:** Never treat a filename, peer claim or
    metadata field as proof that file contents are authentic.
5.  **Signed authorship records:** A valid signature proves that a
    particular identity signed a publication record. It does not prove
    the real-world identity of the person behind that key or the
    legality/originality of their content.
6.  **Content-addressed files:** File identity is based on content, not
    just filename or uploader.
7.  **Resilient transfers:** Downloads must support chunk verification,
    retry, pause and resume.
8.  **Honest availability:** A listed file is not necessarily online.
    Display last-seen and current availability separately.
9.  **Modular protocols:** Network messages and record formats must be
    versioned.
10. **Observable performance:** Measure battery, memory, CPU, bandwidth,
    startup time and transfer reliability on real devices.
11. **Privacy-aware metadata:** Public files are public, but the app
    should avoid unnecessarily exposing private device details, local
    paths or precise user location.
12. **Secure defaults:** Do not expose arbitrary local folders or serve
    files the user has not explicitly published or chosen to replicate.

------------------------------------------------------------------------

# 4. Master roadmap: 20 stages from 0% to 100%

The stages are ordered by dependency. A stage is complete only when its
exit criteria are met and its deliverables are committed and documented.
Percentages are project-progress guideposts, not estimates of calendar
time.

## Stage 01 --- Product definition and requirements

**Progress guide: 0--4%**

### Work

-   Turn the vision and product rules into a formal product requirements
    document.
-   Define the user journeys: first launch, identity creation, profile
    setup, publishing, browsing, search, downloading and replication.
-   Define what is explicitly out of scope: private sharing, likes,
    comments, follows, centralized accounts and mandatory replication.
-   Define the meaning of "available," "online," "replicated,"
    "verified," "original uploader" and "modified version."
-   Define Android and Windows minimum supported operating-system
    versions.
-   Decide initial file-size limits and supported file categories for
    the first release.
-   Define accessibility, language and localization requirements.
-   Create a feature priority list: required for first public release,
    later release, or optional.

### Deliverables

-   `docs/PRODUCT_REQUIREMENTS.md`
-   User-flow diagrams
-   Feature priority matrix
-   Glossary and acceptance criteria

### Exit criteria

-   No contradictory product rules remain.
-   Every first-release feature has a testable acceptance criterion.
-   Platform targets are written down.

## Stage 02 --- Architecture and technical decision records

**Progress guide: 4--9%**

### Work

-   Confirm Kotlin Multiplatform and Compose Multiplatform for shared
    UI/application logic.
-   Confirm Rust as the shared P2P engine.
-   Select the Kotlin-to-Rust FFI approach for Android and Windows.
-   Decide how the Rust core is started, stopped, updated and monitored.
-   Define process/thread ownership and lifecycle rules.
-   Define local storage boundaries: SQLite metadata versus actual file
    bytes.
-   Define protocol boundaries between identity, profiles, discovery,
    transfer and replication.
-   Write architecture decision records (ADRs) for major choices.
-   Identify fallback choices if a library does not work on both target
    platforms.

### Deliverables

-   `docs/ARCHITECTURE.md`
-   `docs/decisions/ADR-*.md`
-   Component diagram
-   FFI interface draft
-   Dependency and license inventory

### Exit criteria

-   Every major component has one responsibility.
-   Android and Windows can use the same Rust core API.
-   No core feature requires a centralized company backend.

## Stage 03 --- Repository, build system and developer workflow

**Progress guide: 9--13%**

### Work

-   Create the Git repository and clean module structure.
-   Configure Gradle for Kotlin Multiplatform and Compose Multiplatform.
-   Configure Cargo workspace for Rust crates.
-   Add formatting, linting and static analysis.
-   Add automated build checks for Android, Windows and Rust.
-   Define branch, commit, issue and release conventions.
-   Add debug logging that does not leak secrets.
-   Add dependency update and vulnerability scanning.
-   Create a repeatable developer setup guide.

### Deliverables

-   Buildable empty Android app.
-   Buildable empty Windows desktop app.
-   Buildable Rust library.
-   CI workflow.
-   `CONTRIBUTING.md` and developer setup documentation.

### Exit criteria

-   A clean checkout can be built using documented steps.
-   CI detects compilation or test failures.
-   No API keys or private signing keys are committed.

## Stage 04 --- UI foundation and design system

**Progress guide: 13--18%**

### Work

-   Establish Ember branding, typography, spacing, colors and icon
    rules.
-   Build reusable Compose components.
-   Create navigation and application state patterns.
-   Design responsive layouts for phone and desktop.
-   Add dark and light themes if both are in the product scope.
-   Build loading, empty, offline, error and retry states.
-   Define accessibility and keyboard-navigation behavior.
-   Create clickable mock screens using local sample data.

### Screens

-   First launch / welcome
-   Identity setup
-   Explore
-   Search
-   User profile
-   File details
-   Publish
-   Downloads
-   Replications / hosted files
-   Settings and network status

### Deliverables

-   Design system
-   Screen prototypes
-   Navigation shell
-   UI component library

### Exit criteria

-   All main journeys can be navigated with mock data.
-   Phone and desktop layouts are usable.
-   No screen falsely implies that network features already work.

## Stage 05 --- Local database and file-store foundation

**Progress guide: 18--23%**

### Work

-   Define SQLite schema and migration strategy.
-   Store local identity references, profile data, publication records,
    file metadata, downloads, replication choices and peer observations.
-   Store file bytes in a managed local file store, not as huge database
    blobs.
-   Define safe file paths and filename handling.
-   Track storage usage and available disk space.
-   Implement cleanup rules that never delete user-published originals
    or chosen replicas without clear user control.
-   Add database backup/export strategy for recoverable local metadata.

### Deliverables

-   Database schema and migrations
-   Local file-store module
-   Repository/data-access layer
-   Storage management screen or settings model

### Exit criteria

-   Data survives app restarts.
-   Migrations work from a clean database and previous schema versions.
-   Unsafe paths and filename collisions are handled.

## Stage 06 --- Identity and cryptographic foundations

**Progress guide: 23--28%**

### Work

-   Generate an Ed25519 identity keypair locally.
-   Derive a stable public identifier from the public key using a
    documented format.
-   Sign and verify test messages.
-   Define identity export and user-controlled backup.
-   Keep private keys out of logs, analytics and ordinary database
    queries.
-   Design key rotation and recovery limitations.
-   Define profile-record signatures and publication-record signatures.
-   Clearly distinguish cryptographic identity from verified real-world
    identity.

### Deliverables

-   Identity module
-   Key backup/export format
-   Signed-record specification
-   Security tests

### Exit criteria

-   A user can create an identity without a central account server.
-   Signatures verify across Android and Windows.
-   A changed signed record fails verification.
-   Key loss limitations are clearly explained to the user.

## Stage 07 --- File identity, hashing and publication records

**Progress guide: 28--33%**

### Work

-   Define a canonical file identifier based on cryptographic content
    hash.
-   Implement SHA-256 hashing for files.
-   Define chunk size and per-chunk hash format for large files.
-   Add a Merkle-tree or equivalent authenticated chunk structure where
    appropriate.
-   Define a signed publication record containing file hash, size,
    filename, description, category, tags, uploader public identity,
    timestamp and protocol version.
-   Ensure the signed fields have canonical serialization.
-   Define duplicate-content behavior.
-   Define modified-file and parent/source references.
-   Ensure a replica preserves the original publication record and does
    not rewrite the uploader.

### Deliverables

-   File identity specification
-   Publication-record schema
-   Hashing and verification module
-   Cross-platform test vectors

### Exit criteria

-   Same file bytes produce the same content identifier.
-   One-byte changes produce a different content identifier.
-   Invalid signatures and corrupted chunks are rejected.
-   A replica cannot change the signed original-uploader attribution.

## Stage 08 --- Rust core and Kotlin FFI integration

**Progress guide: 33--38%**

### Work

-   Implement the Rust core library boundary.
-   Expose a small stable API to Kotlin.
-   Implement initialization, shutdown, status, error reporting and
    cancellation.
-   Integrate Rust with Android through the selected native interface.
-   Integrate Rust with Windows through the selected native binding.
-   Define memory ownership and safe conversion of strings, byte arrays
    and callbacks.
-   Ensure long-running network operations do not block the UI thread.
-   Add compatibility/version checks between Kotlin and Rust components.

### Deliverables

-   Working Rust library on both platforms
-   Kotlin wrapper API
-   FFI error model
-   Integration tests

### Exit criteria

-   Android and Windows can initialize and stop the same core.
-   Kotlin can call a Rust test operation and receive a result.
-   No UI freeze or unmanaged-memory crash occurs in repeated tests.

## Stage 09 --- Peer identity, transports and direct connections

**Progress guide: 38--43%**

### Work

-   Implement libp2p peer identity and node startup.
-   Select and configure supported transports.
-   Add encrypted authenticated peer connections.
-   Implement connection lifecycle, timeouts and reconnection.
-   Add connection diagnostics.
-   Test local-network discovery separately from internet discovery.
-   Test Wi-Fi changes, sleep/wake and network switching.
-   Do not assume every peer is directly reachable.

### Deliverables

-   Running peer node on Android and Windows
-   Direct peer connection API
-   Connection state model
-   Network diagnostic logs

### Exit criteria

-   Two devices can connect and exchange a small test message.
-   Connection failures are handled without crashing.
-   Peer identity is authenticated.

## Stage 10 --- Bootstrap, DHT and decentralized peer discovery

**Progress guide: 43--49%**

### Work

-   Choose and document the DHT record and routing design.
-   Integrate Kademlia DHT capabilities supported by the chosen libp2p
    version.
-   Support multiple bootstrap peers from independent operators.
-   Define how bootstrap addresses are distributed and updated.
-   Add peer routing, lookup timeouts and retry behavior.
-   Design protections against malformed records, resource exhaustion
    and basic DHT poisoning.
-   Add peer diversity where possible.
-   Document that a bootstrap peer helps new nodes join but is not the
    authority for identities or file ownership.
-   Test churn: peers joining, leaving and returning.

### Deliverables

-   DHT integration
-   Bootstrap configuration mechanism
-   Peer-discovery tests
-   Operations guide for community bootstrap nodes

### Exit criteria

-   Fresh installations can join using at least one configured bootstrap
    path.
-   Nodes can discover peers without a central account service.
-   Loss of one bootstrap peer does not automatically disable the whole
    test network.

## Stage 11 --- NAT traversal, relays and difficult networks

**Progress guide: 49--54%**

### Work

-   Implement supported NAT traversal methods, such as hole punching
    where available.
-   Integrate STUN/TURN or libp2p relay mechanisms where appropriate.
-   Define optional community-operated relay nodes.
-   Handle symmetric NAT, restrictive firewalls and carrier-grade NAT.
-   Provide connection fallback order and clear status messages.
-   Test home Wi-Fi, mobile data, hotspot, VPN and restrictive networks.
-   Measure relay bandwidth and cost implications for volunteer
    operators.
-   Make relay usage visible and configurable where practical.

### Deliverables

-   NAT traversal implementation
-   Relay fallback
-   Connectivity test matrix
-   Network troubleshooting UI

### Exit criteria

-   Direct connections work in supported network scenarios.
-   Relay fallback works when direct connectivity fails and a relay is
    reachable.
-   The app clearly reports when a connection cannot be established.

## Stage 12 --- Direct file transfer

**Progress guide: 54--60%**

### Work

-   Implement a request/response protocol for file metadata and chunks.
-   Transfer files directly between peers.
-   Verify chunk hashes as data arrives.
-   Support pause, resume, retry and cancellation.
-   Prevent path traversal and unsafe overwrite behavior.
-   Check free disk space before and during transfer.
-   Support large files without loading the whole file into RAM.
-   Show transfer speed, progress, source peer and errors.
-   Define safe handling for a peer that disconnects mid-transfer.

### Deliverables

-   P2P transfer engine
-   Download manager
-   Transfer progress UI
-   Large-file and interruption tests

### Exit criteria

-   Devices can exchange real files.
-   Corrupt chunks are rejected and retried.
-   Interrupted downloads can resume without restarting from zero.
-   Memory usage remains bounded for large files.

## Stage 13 --- Profiles and signed public metadata

**Progress guide: 60--65%**

### Work

-   Define the public profile schema.
-   Include username/display name, public identity ID, avatar reference,
    bio and published-file references.
-   Define username uniqueness and collision behavior without a central
    registry.
-   Use signed profile records and version/timestamp rules.
-   Publish and retrieve profiles through the decentralized record
    system.
-   Define profile update, stale record and conflict handling.
-   Avoid exposing private device details or precise location by
    default.
-   Build profile pages and local profile editing.

### Deliverables

-   Profile protocol
-   Signed profile publishing/retrieval
-   Profile UI
-   Conflict and stale-record behavior

### Exit criteria

-   A user can publish a profile and another peer can retrieve and
    verify it.
-   Profile changes can be distinguished from stale records.
-   Username collisions are handled transparently.

## Stage 14 --- Publishing and file pages

**Progress guide: 65--70%**

### Work

-   Implement explicit file selection and publishing.
-   Let users add descriptions, categories and tags.
-   Hash the file and create its signed publication record.
-   Make the file available from the publishing device while it is
    online.
-   Display file details, content identifier, original uploader and
    signature status.
-   Display last-seen/online availability separately from publication
    existence.
-   Add unpublish behavior and explain its limits: copies already held
    by other peers cannot be remotely erased.
-   Add safe previews only for supported formats; never execute
    downloaded files automatically.

### Deliverables

-   Publish workflow
-   Public file page
-   Signed publication record publishing
-   Unpublish behavior and documentation

### Exit criteria

-   Users can publish a file from Android and Windows.
-   Other peers can discover the publication record.
-   The source device can serve the file while online.
-   The UI never promises global deletion or permanent availability.

## Stage 15 --- Voluntary replication and availability

**Progress guide: 70--75%**

### Work

-   Add an explicit "replicate/host this file" user action.
-   Let users configure storage quotas and bandwidth limits.
-   Let users pause or remove hosted replicas.
-   Verify a complete file before advertising it as a valid replica.
-   Preserve the original signed publication record.
-   Advertise replica availability without implying that replication
    transfers authorship.
-   Track last-seen and successful verification timestamps.
-   Prevent silent replication of files the user did not select.
-   Add battery-aware and metered-network controls on Android.

### Deliverables

-   Replication manager
-   Hosted-files screen
-   Storage/bandwidth controls
-   Replica verification and advertisement

### Exit criteria

-   A user can voluntarily replicate a file and later serve it to
    another peer.
-   Removing a replica does not affect the original uploader's record.
-   The app does not replicate files without explicit user choice.

## Stage 16 --- Search, Explore and distributed indexing

**Progress guide: 75--81%**

### Work

-   Define the difference between DHT lookup and full-text search.
-   Design a distributed indexing approach for user profiles and file
    metadata.
-   Decide how index shards or index providers are discovered and
    queried.
-   Support search by username, filename, category and tags.
-   Add Explore categories and random discovery.
-   Add sorting/filtering by relevance, recency and currently observed
    availability.
-   Define result freshness, pagination and timeout behavior.
-   Handle duplicate results and conflicting metadata.
-   Avoid designing a hidden central search service.
-   Test index loss, stale indexes, malicious index responses and
    partial network partitions.
-   Make clear that results may be incomplete when the network is
    partitioned or peers are offline.

### Deliverables

-   Distributed search/index design
-   Search and Explore APIs
-   Search UI
-   Index freshness and partial-result behavior

### Exit criteria

-   Users can discover profiles and files without a central search
    database.
-   Results are verified against signed records where possible.
-   Search reports partial or stale results honestly.
-   Search can continue when some index providers disappear.

## Stage 17 --- Multi-peer downloads and advanced resilience

**Progress guide: 81--85%**

### Work

-   Allow a file's chunks to be requested from multiple peers.
-   Verify every chunk against the file's authenticated chunk structure.
-   Schedule chunks across peers based on responsiveness and
    availability.
-   Retry failed chunks from another source.
-   Avoid trusting a peer's claim that it possesses a file until it
    proves possession by serving valid chunks.
-   Add source scoring based on observed transfer reliability, not
    identity popularity.
-   Prevent one malicious peer from corrupting the completed file.
-   Support cross-peer resume.
-   Measure whether multi-peer transfer actually improves speed on
    realistic networks.

### Deliverables

-   Multi-source transfer scheduler
-   Peer reliability metrics
-   Cross-peer resume
-   Adversarial transfer tests

### Exit criteria

-   A download can combine valid chunks from multiple peers.
-   A failed or malicious source cannot silently corrupt the final file.
-   Download can continue when one source disappears.

## Stage 18 --- Security, abuse resistance and privacy hardening

**Progress guide: 85--90%**

### Work

-   Threat-model identity theft, key theft, fake records, Sybil attacks,
    DHT poisoning, malicious peers, spam and resource exhaustion.
-   Validate all incoming protocol messages and metadata.
-   Add connection, request, storage and bandwidth limits.
-   Add local blocking and community moderation/reporting mechanisms
    that do not require a central authority.
-   Add safe file warnings and platform security integration where
    available.
-   Never auto-execute downloaded files.
-   Ensure file previews are sandboxed or delegated to safe platform
    handlers.
-   Add key backup warnings and recovery guidance.
-   Review logs for accidental disclosure of keys, local paths or
    private device data.
-   Conduct dependency and static security analysis.
-   Commission an independent security review before broad public
    release if feasible.

### Deliverables

-   Threat model
-   Security test suite
-   Abuse-control mechanisms
-   Privacy review
-   Security release checklist

### Exit criteria

-   Known high-severity issues are fixed or explicitly blocked from
    release.
-   Malformed peer messages cannot crash the app in tested cases.
-   Downloaded content is never executed automatically.
-   Users understand public visibility and key-loss risks.

## Stage 19 --- Performance, compatibility and release engineering

**Progress guide: 90--95%**

### Work

-   Test on low-memory Android devices as well as modern phones.
-   Test Windows x64 packaging and installation/uninstallation.
-   Measure startup time, RAM, CPU, battery drain, disk usage and
    network overhead.
-   Test long-running sessions and repeated sleep/wake cycles.
-   Test poor Wi-Fi, mobile networks, IPv4/IPv6 and network changes.
-   Optimize chunk sizes, hashing, database indexes and background work.
-   Implement crash reporting only with explicit privacy-conscious
    design and consent where required.
-   Create signed release builds and reproducible build documentation.
-   Add update and rollback strategy.
-   Prepare translations and accessibility review.
-   Run closed alpha and beta programs with real users and different
    network environments.

### Deliverables

-   Performance report
-   Compatibility matrix
-   Android APK/AAB release pipeline
-   Windows EXE installer/portable package
-   Release notes and upgrade guide

### Exit criteria

-   No known release-blocking crashes or data-loss bugs.
-   Performance targets are met on agreed baseline devices.
-   Install, update and uninstall work on both platforms.
-   Release artifacts are signed and checksums are published.

## Stage 20 --- Global launch and ongoing network governance

**Progress guide: 95--100%**

### Work

-   Publish the protocol specification and supported client versions.
-   Publish bootstrap and optional relay operating instructions.
-   Make it possible for independent community members to operate
    infrastructure.
-   Provide a public list or discovery mechanism for independently
    operated bootstrap options without making one operator
    authoritative.
-   Define protocol upgrade and deprecation rules.
-   Establish a security disclosure process.
-   Publish moderation and abuse-handling principles compatible with
    decentralized operation.
-   Monitor network health using privacy-preserving, opt-in
    measurements.
-   Run interoperability tests with independently built clients.
-   Expand capacity through community-run nodes and better peer
    algorithms, not mandatory central hosting.
-   Maintain backward compatibility and a migration policy.
-   Publish regular release notes, known limitations and security
    advisories.

### Deliverables

-   Public Android and Windows releases
-   Protocol documentation
-   Community operations guide
-   Governance and security policy
-   Interoperability test suite
-   Post-launch maintenance plan

### Exit criteria

-   Independent users can install, join, publish, discover, download and
    voluntarily replicate.
-   The network can operate without a company-owned central database or
    file host.
-   Community operators can run optional bootstrap/relay infrastructure.
-   Known limitations and operational responsibilities are public.

------------------------------------------------------------------------

# 5. Cross-stage test strategy

Testing is continuous, not a final-stage activity.

## 5.1 Test layers

-   **Unit tests:** hashing, signatures, parsing, state transitions,
    database operations and scheduling.
-   **Cross-platform tests:** same identity, signature and content
    identifiers on Android and Windows.
-   **Integration tests:** Kotlin-to-Rust calls, database operations and
    file-store behavior.
-   **Network tests:** peer discovery, connection, transfer,
    reconnection, NAT and relay fallback.
-   **Adversarial tests:** corrupt chunks, invalid signatures, malformed
    messages, malicious peers and stale records.
-   **End-to-end tests:** complete user journeys from identity creation
    to publishing, discovery and download.
-   **Performance tests:** memory, CPU, battery, bandwidth, startup and
    large-file handling.
-   **Compatibility tests:** supported Android versions, Windows
    versions, architecture and protocol versions.

## 5.2 Required network test scenarios

1.  Two devices on the same Wi-Fi.
2.  Android on Wi-Fi and Windows on a different network.
3.  Android on mobile data and Windows on Wi-Fi.
4.  Both peers behind different NATs.
5.  Direct connection blocked, relay available.
6.  One peer sleeps or loses connectivity during a download.
7.  Source peer disappears and a voluntary replica remains online.
8.  All known sources go offline.
9.  DHT bootstrap peer disappears.
10. DHT returns stale or malformed metadata.
11. A peer sends corrupted chunks.
12. A file is downloaded from multiple peers.
13. A network changes from Wi-Fi to mobile data.
14. A peer has insufficient disk space.
15. Android background restrictions pause networking.

------------------------------------------------------------------------

# 6. Product behavior and data rules

## 6.1 File record versus file availability

These must be separate concepts:

-   **Publication record:** Signed statement that an identity published
    a particular content hash and metadata.
-   **Content:** The actual bytes identified by the hash.
-   **Source/replica:** A peer currently claiming to serve those bytes.
-   **Availability observation:** A time-limited observation that a peer
    was reachable and could provide valid data.

A publication record can remain discoverable even when no peer currently
has the file online. The interface must say so.

## 6.2 Attribution and versions

-   Original publication records are signed by the publishing identity.
-   Replicas serve the same content and preserve the original
    publication record.
-   A peer that replicates a file is identified as a current source, not
    as the original uploader.
-   A modified file has a different content hash and its own publication
    record.
-   A modified publication can reference a parent/source file, but that
    reference does not automatically prove that the modification is
    authorized by the original uploader.
-   A signature proves control of a cryptographic key at signing time;
    it does not establish a real-world person's legal authorship.

## 6.3 Deletion and unpublishing

A user can stop serving their local original and publish an
unpublish/tombstone statement, but cannot force every independent peer
to delete copies it already holds. The app must not promise remote
deletion from a decentralized network.

## 6.4 Identity and usernames

A cryptographic identity is the stable identifier. A human-readable
username is a discoverability label and may collide unless a separate
decentralized reservation mechanism is designed. Do not treat a username
alone as proof of identity.

------------------------------------------------------------------------

# 7. Key architecture risks and planned mitigations

  -----------------------------------------------------------------------
  Risk                                Planned response
  ----------------------------------- -----------------------------------
  No peers online for a file          Show unavailable; rely on voluntary
                                      replicas

  New peer cannot find network        Multiple independent bootstrap
                                      options and documented manual peer
                                      entry

  NAT prevents direct connection      Hole punching where possible;
                                      optional community relay

  DHT poisoning or Sybil attacks      Record validation, peer diversity,
                                      rate limits and cautious trust

  Distributed search is incomplete    Partial-result indicators, multiple
                                      index providers and eventual
                                      consistency

  False replica claim                 Verify possession by requesting and
                                      validating chunks

  Fake uploader claim                 Signed publication records tied to
                                      public-key identity

  Modified file presented as original Content hashes and distinct
                                      publication records

  Malware or unsafe files             Warnings, scanning integration
                                      where available, safe previews,
                                      never auto-execute

  Key loss                            User-controlled backup/export;
                                      explain that no central password
                                      reset exists

  Android kills background work       Lifecycle-aware transfer queue and
                                      user-visible pause/resume

  High bandwidth/storage use          Explicit replication, quotas and
                                      bandwidth controls

  Protocol fragmentation              Versioned protocol, compatibility
                                      tests and migration policy

  Community relay/bootstrap outage    Multiple independent operators and
                                      optional peer-to-peer fallback
  -----------------------------------------------------------------------

------------------------------------------------------------------------

# 8. First-release scope versus long-term scope

The roadmap targets the complete product, but not every advanced
capability must be enabled in the first public release.

## First functional release

Must include:

-   Android and Windows builds.
-   Local cryptographic identity.
-   Signed public profile and publication records.
-   Local SQLite metadata and managed file storage.
-   Peer connection and basic discovery.
-   Bootstrap configuration.
-   Direct file publishing and downloading.
-   SHA-256 verification.
-   Pause/resume downloads.
-   Explicit voluntary replication.
-   Profile pages and basic file browsing.
-   Basic search over discovered/indexed metadata.
-   Clear online/offline and availability status.
-   Safe error handling and diagnostics.

## Later global-network capabilities

Can be introduced after the first functional release:

-   More resilient multi-bootstrap and DHT operations.
-   Community-operated relays and improved NAT traversal.
-   Distributed indexing at larger scale.
-   Multi-peer chunk downloads.
-   Advanced Explore ranking and random discovery.
-   Better spam and Sybil resistance.
-   Independent client interoperability.
-   Large-network performance optimization.
-   Governance and protocol evolution tooling.

These are not discarded features. They are later milestones in the same
architecture.

------------------------------------------------------------------------

# 9. Definition of 100% complete

Ember is not 100% merely because the screens look finished or because
two devices can transfer a file.

The project reaches its planned 100% milestone when:

-   Android and Windows applications are installable and maintainable.
-   Both use the same documented protocol and Rust networking core.
-   Users create and control cryptographic identities without a company
    account server.
-   Users can publish and retrieve signed public profiles.
-   Users can publish public files from their own devices.
-   Other users can discover files and profiles through decentralized
    mechanisms.
-   Users can download, verify, pause and resume files.
-   Users can voluntarily replicate and serve files.
-   Replicas preserve original attribution.
-   Modified files have distinct content identities and source
    references.
-   Search and Explore work across distributed metadata providers with
    honest freshness/coverage indicators.
-   The network handles peer churn, offline devices, NAT limitations and
    optional relays gracefully.
-   Security, abuse resistance, privacy, performance and compatibility
    have been tested.
-   Independent community members can operate bootstrap/relay
    infrastructure.
-   Protocol and operational documentation are public.
-   Release, update, maintenance and security processes exist.

**100% means the defined product is implemented and has passed its
release criteria. It does not mean the network has millions of users,
that every file is always available, or that every internet topology
permits a direct connection.**

------------------------------------------------------------------------

# 10. How we will work stage by stage

For every stage:

1.  Read the stage requirements and identify dependencies.
2.  Inspect the actual repository and current code before editing.
3.  Break the stage into small, verifiable tasks.
4.  Implement only the agreed stage scope.
5.  Build Android, Windows and Rust targets affected by the change.
6.  Run unit, integration and relevant network tests.
7.  Fix regressions before moving forward.
8.  Update documentation and architecture decisions.
9.  Record completed work, changed files, test results, known issues and
    next steps in a handoff document.
10. Mark the stage complete only when its exit criteria pass.

Do not claim a stage is complete because code was written. It is
complete only after the relevant build and tests succeed.

## Stage tracking template

Copy this block into a progress tracker:

``` text
Current stage:
Stage status: Not started / In progress / Blocked / Complete

Tasks completed:
- [ ]

Files/modules changed:
-

Builds:
- Android:
- Windows:
- Rust:

Tests:
- Passed:
- Failed:
- Not run:

Known issues:
-

Decisions made:
-

Next task:
-

Evidence:
- Build output, test output, screenshots or commit reference
```

## Handoff rule

At the end of each work session, update a project handoff file with:

-   Current stage and exact task.
-   What was implemented.
-   Files changed.
-   Commands/builds/tests run and their actual results.
-   Errors and failed attempts.
-   Decisions and unresolved questions.
-   The exact next action.

This prevents repeated work and allows another compatible coding agent
to continue safely.

------------------------------------------------------------------------

# 11. Immediate starting sequence

Do not start by implementing every feature at once. Begin with the
foundation in this exact order:

1.  Complete Stage 01 requirements.
2.  Complete Stage 02 architecture decisions.
3.  Complete Stage 03 repository and build setup.
4.  Build the empty Android and Windows shells.
5.  Build and integrate the first Rust library.
6.  Verify that both applications can call the same Rust core.
7.  Continue through identity, file integrity, peer networking and the
    remaining stages.

**The target is the full Ember product. Staging is the implementation
and verification strategy---not a reduction of the final vision.**
