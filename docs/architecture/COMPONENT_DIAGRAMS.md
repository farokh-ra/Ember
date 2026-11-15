# Ember Component Diagrams

| Field | Value |
|---|---|
| Document ID | NEXUS-ARCH-DIAGRAMS |
| Version | 1.0.0 |
| Status | Accepted for Stage 02 |
| Date | 2026-11-12 |
| Normative source | [Architecture](../ARCHITECTURE.md), [ADR index](../decisions/README.md) |

These diagrams are design views. They describe intended boundaries and flows; implementation begins in Stage 03 and platform evidence begins in Stage 08.

## 1. System context

```mermaid
flowchart LR
    User[User] --> Android[Android app]
    User --> Windows[Windows app]
    Android --> CoreA[Rust core]
    Windows --> CoreW[Rust core]
    CoreA <-->|QUIC or TCP| PeerA[Android peer]
    CoreA <-->|QUIC or TCP| PeerW[Windows peer]
    CoreW <-->|QUIC or TCP| PeerA
    CoreW <-->|QUIC or TCP| PeerW
    CoreA -.->|optional| Bootstrap[Community bootstrap nodes]
    CoreW -.->|optional| Bootstrap
    CoreA -.->|later| Index[Community index providers]
    CoreW -.->|later| Index
    CoreA -.->|later| Relay[Community relays]
    CoreW -.->|later| Relay
    CoreA --> StoreA[(SQLite and managed store)]
    CoreW --> StoreW[(SQLite and managed store)]
```

There is no Ember-operated account, file host, search database, or telemetry service. Bootstrap, index, and relay peers are optional infrastructure and never authorities for identity or ownership.

## 2. Container and module view

```mermaid
flowchart TB
    subgraph Android["Android application"]
        AA[androidApp shell]
        AS[Foreground service]
        AK[Keystore adapter]
        AP[SAF file picker]
    end
    subgraph Windows["Windows application"]
        WA[desktopApp shell]
        WT[Tray and window]
        WD[DPAPI adapter]
        WF[Native file dialog]
    end
    subgraph Shared["Shared Kotlin"]
        UI[shared/ui Compose screens]
        PRES[shared/presentation view models]
        API[shared/core-api NexusCore]
        CLIENT[shared/core-client FFI and event pump]
        MODEL[shared/model DTOs and errors]
    end
    subgraph Core["Rust core"]
        FFI[nexus-ffi C ABI JNI and JNA]
        CORE[nexus-core orchestration]
        ID[nexus-identity]
        CRYPTO[nexus-crypto]
        PROTO[nexus-protocol]
        STORE[nexus-store]
        TRANSFER[nexus-transfer]
        NET[nexus-net]
        SEARCH[nexus-search]
        OBS[nexus-observability]
    end
    AA --> UI
    WA --> UI
    AA --> AS
    AA --> AK
    AA --> AP
    WA --> WT
    WA --> WD
    WA --> WF
    UI --> PRES
    PRES --> API
    API --> CLIENT
    CLIENT --> MODEL
    CLIENT -->|JNI| FFI
    CLIENT -->|JNA| FFI
    FFI --> CORE
    CORE --> ID
    CORE --> NET
    CORE --> TRANSFER
    CORE --> SEARCH
    CORE --> STORE
    CORE --> PROTO
    CORE --> OBS
    ID --> CRYPTO
    NET --> PROTO
    TRANSFER --> STORE
    SEARCH --> STORE
    PROTO --> CRYPTO
```

Kotlin never writes SQLite or the managed store. The platform shell supplies app-private paths and the sealed local secret; the core owns everything else.

## 3. Kotlin dependency direction

```mermaid
flowchart LR
    APP[apps/androidApp and apps/desktopApp] --> UI[shared/ui]
    UI --> PRES[shared/presentation]
    PRES --> API[shared/core-api]
    API --> CLIENT[shared/core-client]
    CLIENT --> MODEL[shared/model]
    PRES --> MODEL
    UI --> MODEL
    CLIENT --> FAKE[Fake NexusCore for tests]
    FAKE --> API
```

`shared/core-api` has no JNI, JNA, SQLite, libp2p, or Compose dependency. `shared/core-client` is the only Kotlin module that knows a native bridge exists.

## 4. Rust crate dependency graph

```mermaid
flowchart BT
    TYPES[nexus-types]
    CRYPTO[nexus-crypto]
    PROTO[nexus-protocol]
    STORE[nexus-store]
    ID[nexus-identity]
    TRANSFER[nexus-transfer]
    NET[nexus-net]
    SEARCH[nexus-search]
    OBS[nexus-observability]
    CORE[nexus-core]
    FFI[nexus-ffi]
    TESTKIT[nexus-testkit dev only]

    CRYPTO --> TYPES
    PROTO --> CRYPTO
    PROTO --> TYPES
    STORE --> TYPES
    ID --> CRYPTO
    ID --> STORE
    ID --> TYPES
    TRANSFER --> PROTO
    TRANSFER --> STORE
    TRANSFER --> CRYPTO
    TRANSFER --> TYPES
    NET --> PROTO
    NET --> CRYPTO
    NET --> TYPES
    SEARCH --> STORE
    SEARCH --> PROTO
    SEARCH --> TYPES
    OBS --> TYPES
    CORE --> ID
    CORE --> NET
    CORE --> TRANSFER
    CORE --> SEARCH
    CORE --> STORE
    CORE --> PROTO
    CORE --> OBS
    FFI --> CORE
    TESTKIT -.-> CORE
```

The graph is acyclic. A lower crate never depends on `nexus-core` or `nexus-ffi`.

## 5. Core runtime and threads

```mermaid
flowchart TB
    subgraph JVM["Application process - JVM"]
        MAIN[Compose UI main thread]
        IO[Kotlin IO dispatcher]
        PUMP[Single event-pump thread]
    end
    subgraph NATIVE["Rust core"]
        FFI[nexus-ffi]
        EVENT[Bounded event queue]
        CMD[Command bus]
        TOKIO[Tokio runtime 2-4 workers]
        DBW[SQLite writer thread]
        DBR[SQLite read pool 2]
        HASH[Hashing pool max 2]
        NET[libp2p node]
        FS[Managed file store]
    end
    MAIN --> IO
    IO -->|invoke and streaming calls| FFI
    PUMP -->|next_event| EVENT
    FFI --> CMD
    CMD --> TOKIO
    TOKIO --> NET
    TOKIO --> DBW
    TOKIO --> DBR
    TOKIO --> HASH
    HASH --> FS
    DBW --> FS
    TOKIO --> EVENT
```

- No disk, network, or hashing work runs on the UI thread.
- The event pump is the only blocking native consumer.
- SQLite has one writer and a small read pool.
- Hashing is bounded to two concurrent jobs so several downloads cannot exhaust memory.

## 6. Identity creation and backup sequence

```mermaid
sequenceDiagram
    actor U as User
    participant UI as Compose UI
    participant H as CoreHost
    participant S as Platform secure storage
    participant C as Rust core
    participant D as SQLite and secrets

    U->>UI: Create identity
    UI->>H: createIdentity()
    H->>S: Create or load local secret
    S-->>H: 32-byte LEK
    H->>C: create(config, LEK)
    C->>C: Generate Ed25519 keypair with OS CSPRNG
    C->>C: Derive nx1 identity ID
    C->>D: Write encrypted identity file and identity reference
    C-->>H: identity.created
    H-->>UI: Show identity ID and backup warning
    U->>UI: Export encrypted backup
    UI->>H: exportBackup(passphrase)
    H->>C: identity.export_backup (operation)
    C->>C: Argon2id, XChaCha20-Poly1305, versioned header
    C-->>H: completed with backup bytes
    H->>UI: Save through platform file picker
```

The private key never enters Kotlin. The platform secret seals the key at rest; the backup is the only portable copy.

## 7. Publish sequence

```mermaid
sequenceDiagram
    actor U as User
    participant UI as Compose UI
    participant H as CoreHost
    participant C as Rust core
    participant T as nexus-transfer
    participant S as nexus-store
    participant N as nexus-net

    U->>UI: Select file and metadata
    UI->>H: import and publish
    H->>C: import_begin, import_write, import_finish
    C->>T: Stream bytes, hash, build manifest
    T->>S: Quota check, temporary file, verified blob
    T-->>C: content ID, manifest root
    C->>C: Sign publication body
    C->>S: Store exact signed envelope
    C->>N: Announce record and provide content
    N-->>C: Announcement result
    C-->>H: publication.completed
    H-->>UI: File page and honest availability state
```

Publishing imports a managed copy. The external original is not modified or deleted.

## 8. Download and verification sequence

```mermaid
sequenceDiagram
    participant UI as Compose UI
    participant C as Rust core
    participant N as nexus-net
    participant P as Source peer
    participant T as nexus-transfer
    participant S as nexus-store

    UI->>C: download.start(publication)
    C->>N: Resolve providers and connect
    N->>P: Request manifest
    P-->>N: Canonical manifest
    N->>C: Bounded manifest bytes
    C->>T: Verify root, content ID, size
    T->>S: Create partial and verified prefix
    loop Each chunk
        C->>N: Request chunk index
        N->>P: Chunk request
        P-->>N: Chunk bytes
        N->>T: Bounded chunk
        T->>T: Verify length and SHA-256
        T->>S: Append to verified prefix
        T-->>C: progress event
    end
    T->>T: Final full-file SHA-256
    alt Verification passes
        T->>S: Promote to immutable blob
        C-->>UI: download.completed
    else Verification fails
        T->>S: Keep partial, stop promotion
        C-->>UI: download.failed with retry or delete
    end
```

A failed source or chunk never becomes a completed file. The next source starts from the verified prefix.

## 9. Replica lifecycle

```mermaid
stateDiagram-v2
    [*] --> Candidate
    Candidate --> Verifying: user chooses replicate
    Verifying --> Rejected: signature, manifest, or content failure
    Verifying --> Active: complete verification passes
    Active --> Paused: user pauses
    Paused --> Active: user resumes
    Active --> Rechecking: startup, local change, or 7-day timer
    Rechecking --> Active: verification passes
    Rechecking --> Failed: verification fails
    Failed --> Verifying: user retries
    Active --> Removed: user confirms removal
    Failed --> Removed: user confirms removal
    Rejected --> [*]
    Removed --> [*]
```

Only `Active` replicas are advertised. A failed re-verification immediately stops serving and advertising.

## 10. Storage entities

```mermaid
erDiagram
    IDENTITY ||--o{ PUBLICATION : signs
    IDENTITY ||--o{ PROFILE : signs
    IDENTITY ||--o{ TOMBSTONE : signs
    CONTENT ||--o{ PUBLICATION : describes
    CONTENT ||--o{ CONTENT_REF : has
    CONTENT ||--o| MANIFEST : uses
    PUBLICATION ||--o{ TOMBSTONE : may_have
    PUBLICATION ||--o{ DOWNLOAD : requested_as
    CONTENT ||--o{ REPLICA : hosted_as
    PEER ||--o{ PEER_OBSERVATION : observed

    IDENTITY {
        blob identity_id
        text display_name
        int created_at
    }
    CONTENT {
        blob content_id PK
        int size
        blob manifest_root
        text blob_state
        int verified_at
    }
    PUBLICATION {
        blob publication_id PK
        blob identity_id
        blob content_id
        int revision
        blob supersedes
        blob raw_envelope
        text verification_state
    }
    PROFILE {
        blob identity_id
        int revision
        blob raw_envelope
        text conflict_group
    }
    TOMBSTONE {
        blob publication_id
        blob identity_id
        blob raw_envelope
    }
    MANIFEST {
        blob manifest_root PK
        blob content_id
        int size
        int chunk_size
    }
    CONTENT_REF {
        blob content_id
        text ref_type
        blob ref_id
    }
    DOWNLOAD {
        blob download_id PK
        blob publication_id
        text state
        int verified_bytes
        int total_bytes
    }
    REPLICA {
        blob replica_id PK
        blob content_id
        blob publication_id
        text state
        int last_verified_at
    }
    PEER {
        text peer_id PK
        text addresses
        int last_seen
    }
    PEER_OBSERVATION {
        text peer_id
        text source
        int observed_at
    }
```

File bytes are not in this diagram. They live in the managed store and are referenced by content ID. Search tables are derived and omitted for clarity.

## 11. Trust boundaries

```mermaid
flowchart TB
    subgraph Untrusted["Untrusted input"]
        U[User input and file names]
        P[Peer records, manifests, chunks]
        X[External files and export destinations]
    end
    subgraph Platform["Platform trust"]
        KS[Android Keystore or Windows DPAPI]
        FP[File picker and share destination]
    end
    subgraph Core["Trusted core boundary"]
        V[Validation and signature checks]
        ST[(SQLite and managed store)]
        K[Encrypted keys]
    end
    U --> V
    P --> V
    V --> ST
    KS -->|LEK at start| K
    ST -->|export read cursor| FP
    FP --> X
    P -.->|never trusted directly| U
    ST -.->|redacted logs only| O[Diagnostics]
```

Validation happens before any record reaches SQLite, the search index, or the UI. The platform secure store is trusted only for sealing the LEK; the core still authenticates the identity file.

## 12. Core lifecycle

```mermaid
stateDiagram-v2
    [*] --> Created
    Created --> Starting: create and start
    Starting --> Running: database, identity, network ready
    Starting --> Locked: LEK or identity unavailable
    Locked --> Starting: restore or reset
    Starting --> Failed: migration, ABI, or store failure
    Running --> Stopping: stop or app exit
    Stopping --> Stopped: transfers paused and state flushed
    Stopped --> Starting: start again
    Stopped --> Destroyed: destroy
    Failed --> Destroyed: destroy after diagnostics
    Destroyed --> [*]
```

`Running` does not imply online. Network state is reported separately and can be `Online`, `Degraded`, or `Offline`.

## 13. Platform lifecycle view

```mermaid
flowchart LR
    subgraph Android
        A1[Application start] --> A2{Identity exists}
        A2 -->|No| A3[Onboarding]
        A2 -->|Yes| A4[Start core]
        A4 --> A5{Active transfer or background hosting}
        A5 -->|Yes| A6[Foreground service and notification]
        A5 -->|No| A7[Foreground UI only]
        A6 --> A8[OS stop or transfer end]
        A8 --> A9[Pause, persist, remain resumable]
    end
    subgraph Windows
        W1[Application start] --> W2{Identity exists}
        W2 -->|No| W3[Onboarding]
        W2 -->|Yes| W4[Start core]
        W4 --> W5{Active work or tray hosting}
        W5 -->|Yes| W6[Tray icon and core running]
        W5 -->|No| W7[Window only]
        W6 --> W8[Exit or no work]
        W8 --> W9[Stop core and flush state]
    end
```

No platform starts a background node at boot or without a user action.
