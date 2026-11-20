//! Stage 12 loopback integration: `/nexus/file/1` metadata exchange and
//! bounded chunk slices between two real libp2p nodes.
//!
//! Both nodes listen on loopback only. The source serves bytes from an
//! in-memory provider; the client verifies every slice against the same
//! content. No public service is contacted.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nexus_identity::LocalEncryptionKey;
use nexus_net::{FileMetadata, FileProvider, NetError, Network, NetworkConfig};
use nexus_protocol::{CHUNK_SIZE_V1, FileSelector, MAX_CHUNK_SLICE_BYTES};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_root(name: &str) -> PathBuf {
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("nexus-net-stage12-{name}-{}-{counter}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("root");
    path
}

fn config(root: &Path) -> NetworkConfig {
    let mut config = NetworkConfig::new(root);
    config.listen = vec!["/ip4/127.0.0.1/tcp/0".parse().expect("tcp listen")];
    config.connect_timeout = Duration::from_secs(5);
    config.request_timeout = Duration::from_secs(5);
    config.reconnect_base_delay = Duration::from_millis(250);
    config.reconnect_max_delay = Duration::from_secs(2);
    config.autonat_enabled = false;
    config.dcutr_enabled = false;
    config
}

fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if condition() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("timed out waiting for {what}");
}

fn dial_address(network: &Network) -> String {
    let mut address = None;
    wait_until("a loopback TCP listener", || {
        address = network
            .status()
            .listen_addresses
            .into_iter()
            .find(|candidate| candidate.contains("/tcp/"));
        address.is_some()
    });
    let address = address.expect("listen address");
    if address.ends_with(&format!("/p2p/{}", network.peer_id())) {
        address
    } else {
        format!("{address}/p2p/{}", network.peer_id())
    }
}

/// In-memory provider that serves one file and records every chunk request.
struct TestProvider {
    content: Vec<u8>,
    manifest: Vec<u8>,
    publication: Option<Vec<u8>>,
    content_id: [u8; 32],
    publication_id: [u8; 32],
    chunk_calls: Mutex<Vec<(u32, u32, u32)>>,
}

impl TestProvider {
    fn new(content: Vec<u8>, with_publication: bool) -> Self {
        let content_id = [0x11_u8; 32];
        let publication_id = [0x22_u8; 32];
        let manifest = (0..384)
            .map(|index| u8::try_from((index * 7 + 3) % 251).unwrap_or(0))
            .collect();
        let publication = with_publication.then(|| vec![0x33_u8; 96]);
        Self {
            content,
            manifest,
            publication,
            content_id,
            publication_id,
            chunk_calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<(u32, u32, u32)> {
        self.chunk_calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

impl FileProvider for TestProvider {
    fn metadata(&self, selector: FileSelector) -> Option<FileMetadata> {
        let known = match selector {
            FileSelector::ContentId(id) => id == self.content_id,
            FileSelector::PublicationId(id) => id == self.publication_id,
        };
        known.then(|| FileMetadata {
            manifest: self.manifest.clone(),
            publication: self.publication.clone(),
        })
    }

    fn chunk(&self, content_id: &[u8; 32], index: u32, offset: u32, length: u32) -> Option<Vec<u8>> {
        if *content_id != self.content_id {
            return None;
        }
        self.chunk_calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push((index, offset, length));
        let chunk_size = usize::try_from(CHUNK_SIZE_V1).ok()?;
        if usize::try_from(offset)
            .ok()?
            .checked_add(usize::try_from(length).ok()?)?
            > chunk_size
        {
            return None;
        }
        let start = usize::try_from(index)
            .ok()?
            .checked_mul(chunk_size)?
            .checked_add(usize::try_from(offset).ok()?)?;
        let end = start.checked_add(usize::try_from(length).ok()?)?;
        self.content.get(start..end).map(<[u8]>::to_vec)
    }
}

struct Fixture {
    source: Network,
    client: Network,
    provider: Arc<TestProvider>,
    source_address: String,
    _source_root: PathBuf,
    _client_root: PathBuf,
}

impl Fixture {
    fn start(with_publication: bool) -> Self {
        let content: Vec<u8> = (0..(2 * usize::try_from(CHUNK_SIZE_V1).expect("chunk") + 4_096))
            .map(|index| u8::try_from((index * 31 + 5) % 251).unwrap_or(0))
            .collect();
        let provider = Arc::new(TestProvider::new(content, with_publication));
        let source_root = temp_root("source");
        let client_root = temp_root("client");
        let source = Network::start_with_provider(
            config(&source_root),
            &LocalEncryptionKey::new([21_u8; 32]),
            Arc::new(()),
            provider.clone(),
        )
        .expect("start source");
        let source_address = dial_address(&source);
        let client = Network::start(
            config(&client_root),
            &LocalEncryptionKey::new([22_u8; 32]),
            Arc::new(()),
        )
        .expect("start client");
        Self {
            source,
            client,
            provider,
            source_address,
            _source_root: source_root,
            _client_root: client_root,
        }
    }

    fn source_peer(&self) -> String {
        self.source.peer_id().to_string()
    }

    fn connect(&self) {
        let connected = self
            .client
            .connect(&self.source_address)
            .expect("direct loopback connect");
        assert_eq!(connected.peer_id, self.source.peer_id());
        assert_eq!(connected.path, "direct");
    }
}

#[test]
fn metadata_and_bounded_chunk_slices_cross_the_wire() {
    let fixture = Fixture::start(true);
    let provider = &fixture.provider;
    let source_peer = fixture.source_peer();
    let content_id = provider.content_id;

    let not_connected = fixture
        .client
        .fetch_file_metadata(&source_peer, FileSelector::ContentId(content_id))
        .expect_err("a metadata request needs a connected peer");
    assert!(matches!(not_connected, NetError::NotConnected));

    fixture.connect();

    let metadata = fixture
        .client
        .fetch_file_metadata(&source_peer, FileSelector::ContentId(content_id))
        .expect("content metadata");
    assert_eq!(metadata.manifest, provider.manifest);
    assert_eq!(metadata.publication.as_deref(), provider.publication.as_deref());

    let by_publication = fixture
        .client
        .fetch_file_metadata(&source_peer, FileSelector::PublicationId(provider.publication_id))
        .expect("publication metadata");
    assert_eq!(by_publication.manifest, provider.manifest);

    let chunk_size = usize::try_from(CHUNK_SIZE_V1).expect("chunk");
    let slice = MAX_CHUNK_SLICE_BYTES;
    let slice_len = u32::try_from(MAX_CHUNK_SLICE_BYTES).expect("slice length");
    let requests = [
        (0_u32, 0_u32, slice_len),
        (0, u32::try_from(chunk_size - slice).expect("offset"), slice_len),
        (
            1,
            u32::try_from(slice).expect("offset"),
            u32::try_from(slice / 2).expect("length"),
        ),
        (2, 0, 4_096),
    ];
    for (index, offset, length) in requests {
        let bytes = fixture
            .client
            .fetch_file_chunk(&source_peer, &content_id, index, offset, length)
            .expect("chunk slice");
        let start = usize::try_from(index).expect("index") * chunk_size + usize::try_from(offset).expect("offset");
        let end = start + usize::try_from(length).expect("length");
        assert_eq!(bytes, provider.content[start..end], "slice {index}:{offset}");
    }
    assert_eq!(
        provider.calls(),
        vec![(0, 0, 65_536), (0, 983_040, 65_536), (1, 65_536, 32_768), (2, 0, 4_096),]
    );
}

#[test]
fn unknown_content_and_out_of_bounds_slices_are_rejected() {
    let fixture = Fixture::start(false);
    let provider = &fixture.provider;
    let source_peer = fixture.source_peer();
    fixture.connect();

    let missing = fixture
        .client
        .fetch_file_metadata(&source_peer, FileSelector::ContentId([0x99_u8; 32]))
        .expect_err("unknown content");
    assert!(matches!(missing, NetError::FileUnavailable));

    let no_publication = fixture
        .client
        .fetch_file_metadata(&source_peer, FileSelector::PublicationId(provider.publication_id))
        .expect("publication metadata");
    assert!(no_publication.publication.is_none());

    let missing_chunk = fixture
        .client
        .fetch_file_chunk(&source_peer, &[0x99_u8; 32], 0, 0, 1_024)
        .expect_err("unknown content chunk");
    assert!(matches!(missing_chunk, NetError::FileUnavailable));

    let past_end = fixture
        .client
        .fetch_file_chunk(&source_peer, &provider.content_id, 3, 0, 1)
        .expect_err("chunk past the manifest");
    assert!(matches!(past_end, NetError::FileUnavailable));

    let oversized = u32::try_from(MAX_CHUNK_SLICE_BYTES + 1).expect("length");
    let too_large = fixture
        .client
        .fetch_file_chunk(&source_peer, &provider.content_id, 0, 0, oversized)
        .expect_err("oversized slice");
    assert!(matches!(too_large, NetError::MessageTooLarge));
}
