//! libp2p networking, authenticated peer connections, and connection diagnostics.
//!
//! The public surface is synchronous and bounded. [`Network::start`] loads (or
//! creates) the encrypted device peer key, spawns one background driver thread,
//! and starts QUIC v1 and TCP listeners. Calls such as [`Network::connect`] and
//! [`Network::send_test`] submit a bounded command and wait for a one-shot
//! reply; failures never panic and never require the caller to own a runtime.
//!
//! The device peer key is separate from the user signing identity. Restoring an
//! identity backup never reads or replaces `network/peer_key.enc`.

mod behaviour;
mod bootstrap;
mod codec;
mod config;
mod driver;
mod error;
mod file;
mod index;
mod peer_key;
mod relay;
mod shared;
mod state;

use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use libp2p::{Multiaddr, PeerId};
use nexus_identity::LocalEncryptionKey;
use tokio::sync::{mpsc, oneshot};

pub use bootstrap::{BOOTSTRAP_ENV_VAR, BOOTSTRAP_FILE_NAME, BOOTSTRAP_MAX_FILE_LENGTH};
pub use config::{
    DEFAULT_CONNECT_TIMEOUT, DEFAULT_DHT_QUERY_TIMEOUT, DEFAULT_DHT_RECORD_TTL, DEFAULT_DHT_REPUBLISH_INTERVAL,
    DEFAULT_DHT_STORE_MAX_PROVIDED_KEYS, DEFAULT_DHT_STORE_MAX_PROVIDERS, DEFAULT_DHT_STORE_MAX_RECORDS,
    DEFAULT_IDLE_CONNECTION_TIMEOUT, DEFAULT_MAX_CONCURRENT_STREAMS, DEFAULT_MAX_ESTABLISHED,
    DEFAULT_MAX_ESTABLISHED_PER_PEER, DEFAULT_MAX_PENDING_INCOMING, DEFAULT_MAX_REQUESTS_PER_PEER_PER_MINUTE,
    DEFAULT_RECONNECT_BASE_DELAY, DEFAULT_RECONNECT_MAX_DELAY, DEFAULT_RELAY_MAX_CIRCUIT_BYTES,
    DEFAULT_RELAY_MAX_CIRCUIT_DURATION, DEFAULT_RELAY_MAX_CIRCUITS, DEFAULT_RELAY_MAX_CIRCUITS_PER_PEER,
    DEFAULT_RELAY_MAX_RESERVATIONS, DEFAULT_RELAY_RESERVATION_TIMEOUT, DEFAULT_REQUEST_TIMEOUT, DhtMode,
    MAX_BOOTSTRAP_PEERS, MAX_CONNECTION_LIMIT, MAX_DHT_PROVIDED_KEYS, MAX_DHT_QUERY_TIMEOUT, MAX_DHT_RECORD_TTL,
    MAX_DHT_STORE_PROVIDERS, MAX_DHT_STORE_RECORDS, MAX_EXTERNAL_ADDRESSES, MAX_LISTEN_ADDRESSES,
    MAX_RELAY_CIRCUIT_BYTES, MAX_RELAY_CIRCUIT_DURATION, MAX_RELAY_CIRCUITS, MAX_RELAY_RESERVATION_TIMEOUT,
    MAX_RELAY_RESERVATIONS, MAX_RELAYS, MAX_UPLOAD_BYTES_PER_SECOND, MIN_RELAY_CIRCUIT_BYTES,
    MIN_UPLOAD_BYTES_PER_SECOND, NetworkConfig, QUIC_LISTEN_ADDRESS, TCP_LISTEN_ADDRESS, require_peer_id,
};
pub use error::NetError;
pub use file::{FileMetadata, FileProvider};
pub use index::IndexProvider;
pub use peer_key::{
    NETWORK_DIRECTORY, PEER_KEY_FILE_NAME, PEER_KEY_FORMAT_VERSION, PEER_KEY_HEADER_LENGTH, PEER_KEY_KDF_HKDF_SHA256,
    PEER_KEY_MAGIC, PEER_KEY_MAX_FILE_LENGTH, PEER_KEY_SALT_LENGTH, PEER_KEY_SEED_LENGTH, PeerKey,
};
pub use relay::{RELAY_ENV_VAR, RELAY_FILE_NAME, RELAY_MAX_FILE_LENGTH};
pub use state::{
    ConnectedPeer, DhtValue, DiscoveredPeer, NetworkEvent, NetworkObserver, NetworkState, NetworkStatus, PeerDiscovery,
    PeerEventKind, PeerSnapshot, PeerState, TestReply, transports,
};

use crate::driver::{COMMAND_CAPACITY, Command};
use crate::file::EmptyFileProvider;
use crate::index::EmptyIndexProvider;
use crate::shared::Shared;
use nexus_protocol::{DHT_KEY_BYTES, DhtRecord, FileSelector, IndexRequest, IndexResponse};

/// How long [`Network::start`] waits for the driver to build the swarm.
pub const STARTUP_TIMEOUT: Duration = Duration::from_secs(5);

/// A running peer node.
///
/// Dropping the value shuts the driver down. All methods are bounded: they
/// either complete or return a typed [`NetError`].
pub struct Network {
    shared: Arc<Shared>,
    commands: mpsc::Sender<Command>,
    peer_id: String,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Network {
    /// Validates `config`, unlocks the device peer key, and starts the driver.
    ///
    /// The persisted bootstrap list is merged into the configured list before
    /// listeners start. A returned error means no driver thread is running.
    pub fn start(
        config: NetworkConfig,
        local_key: &LocalEncryptionKey,
        observer: Arc<dyn NetworkObserver>,
    ) -> Result<Self, NetError> {
        Self::start_with_provider(config, local_key, observer, Arc::new(EmptyFileProvider))
    }

    /// Starts the driver with a storage-backed file provider for inbound
    /// `/nexus/file/1` requests.
    pub fn start_with_provider(
        config: NetworkConfig,
        local_key: &LocalEncryptionKey,
        observer: Arc<dyn NetworkObserver>,
        file_provider: Arc<dyn FileProvider>,
    ) -> Result<Self, NetError> {
        Self::start_with_providers(config, local_key, observer, file_provider, Arc::new(EmptyIndexProvider))
    }

    /// Starts the driver with providers for inbound file and index requests.
    pub fn start_with_providers(
        config: NetworkConfig,
        local_key: &LocalEncryptionKey,
        observer: Arc<dyn NetworkObserver>,
        file_provider: Arc<dyn FileProvider>,
        index_provider: Arc<dyn IndexProvider>,
    ) -> Result<Self, NetError> {
        let mut config = config;
        let mut merged = bootstrap::compiled()?;
        for address in config.bootstrap.drain(..) {
            if !merged.contains(&address) {
                merged.push(address);
            }
        }
        for address in bootstrap::load(&config.data_root)? {
            if !merged.contains(&address) {
                merged.push(address);
            }
        }
        config.bootstrap = merged;

        let mut relays = relay::compiled()?;
        for address in config.relays.drain(..) {
            if !relays.contains(&address) {
                relays.push(address);
            }
        }
        for address in relay::load(&config.data_root)? {
            if !relays.contains(&address) {
                relays.push(address);
            }
        }
        config.relays = relays;
        config.validate()?;

        let peer_key = PeerKey::load_or_create(&config.data_root, local_key)?;
        let peer_id = peer_key.peer_id();
        let keypair = peer_key.keypair().clone();
        let address_texts: Vec<String> = config.bootstrap.iter().map(ToString::to_string).collect();
        let relay_texts: Vec<String> = config.relays.iter().map(ToString::to_string).collect();

        let status = NetworkStatus::starting(peer_id.to_string());
        let shared = Arc::new(Shared::new(status, observer));
        shared.set_bootstrap(address_texts);
        shared.set_relays(relay_texts);

        let config = Arc::new(config);
        let (commands, receiver) = mpsc::channel(COMMAND_CAPACITY);
        let (startup_sender, startup_receiver) = std::sync::mpsc::sync_channel(1);
        let thread_config = Arc::clone(&config);
        let thread_shared = Arc::clone(&shared);

        let thread = std::thread::Builder::new()
            .name("nexus-net".to_owned())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                    Ok(runtime) => runtime,
                    Err(_) => {
                        let _ = startup_sender.send(Err(NetError::DriverFailed));
                        return;
                    }
                };
                runtime.block_on(async move {
                    match driver::Driver::start(
                        thread_config,
                        keypair,
                        thread_shared,
                        receiver,
                        Arc::clone(&file_provider),
                        index_provider,
                    ) {
                        Ok(driver) => {
                            let _ = startup_sender.send(Ok(()));
                            driver.run().await;
                        }
                        Err(error) => {
                            let _ = startup_sender.send(Err(error));
                        }
                    }
                });
            })
            .map_err(NetError::Io)?;

        match startup_receiver.recv_timeout(STARTUP_TIMEOUT) {
            Ok(Ok(())) => Ok(Self {
                shared,
                commands,
                peer_id: peer_id.to_string(),
                thread: Mutex::new(Some(thread)),
            }),
            Ok(Err(error)) => {
                let _ = thread.join();
                Err(error)
            }
            Err(_) => {
                let _ = thread.join();
                Err(NetError::Timeout)
            }
        }
    }

    /// This node's device peer ID.
    #[must_use]
    pub fn peer_id(&self) -> &str {
        &self.peer_id
    }

    /// Current aggregate status.
    #[must_use]
    pub fn status(&self) -> NetworkStatus {
        self.shared.status()
    }

    /// Bounded snapshot of observed peers.
    #[must_use]
    pub fn peers(&self) -> Vec<PeerSnapshot> {
        self.shared.peers()
    }

    /// Persisted bootstrap and manual peer addresses.
    #[must_use]
    pub fn bootstrap_list(&self) -> Vec<String> {
        self.shared.bootstrap()
    }

    /// Dials one `/p2p`-terminated address and waits for an authenticated
    /// connection.
    pub fn connect(&self, address: &str) -> Result<ConnectedPeer, NetError> {
        let (peer_id, address) = parse_peer_address(address)?;
        let (respond, receiver) = oneshot::channel();
        self.send(Command::Dial {
            peer_id,
            address,
            respond,
        })?;
        receiver.blocking_recv().unwrap_or(Err(NetError::NotRunning))
    }

    /// Sends a bounded transport-check payload to a connected peer and returns
    /// the echoed bytes.
    pub fn send_test(&self, peer_id: &str, payload: &[u8]) -> Result<TestReply, NetError> {
        let peer_id: PeerId = peer_id.trim().parse().map_err(|_| NetError::InvalidPeerId)?;
        let (respond, receiver) = oneshot::channel();
        self.send(Command::SendTest {
            peer_id,
            payload: payload.to_vec(),
            respond,
        })?;
        receiver.blocking_recv().unwrap_or(Err(NetError::NotRunning))
    }

    /// Fetches metadata for a content ID or publication ID from one peer.
    ///
    /// The manifest bytes are bounded but not trusted here; the caller verifies
    /// the root against a signed publication or the content ID.
    pub fn fetch_file_metadata(&self, peer_id: &str, selector: FileSelector) -> Result<FileMetadata, NetError> {
        let peer_id: PeerId = peer_id.trim().parse().map_err(|_| NetError::InvalidPeerId)?;
        let (respond, receiver) = oneshot::channel();
        self.send(Command::FetchFileMetadata {
            peer_id,
            selector,
            respond,
        })?;
        receiver.blocking_recv().unwrap_or(Err(NetError::NotRunning))
    }

    /// Fetches one bounded slice from a verified content chunk.
    pub fn fetch_file_chunk(
        &self,
        peer_id: &str,
        content_id: &[u8; 32],
        index: u32,
        offset: u32,
        length: u32,
    ) -> Result<Vec<u8>, NetError> {
        let peer_id: PeerId = peer_id.trim().parse().map_err(|_| NetError::InvalidPeerId)?;
        let (respond, receiver) = oneshot::channel();
        self.send(Command::FetchFileChunk {
            peer_id,
            content_id: *content_id,
            index,
            offset,
            length,
            respond,
        })?;
        receiver.blocking_recv().unwrap_or(Err(NetError::NotRunning))
    }

    /// Fetches one bounded page from an optional index provider.
    ///
    /// The request is validated and encoded locally. The returned records are
    /// untrusted signed envelopes and must be verified by the caller before
    /// they are cached, indexed, or displayed.
    pub fn fetch_index_page(&self, peer_id: &str, request: &IndexRequest) -> Result<IndexResponse, NetError> {
        let peer_id: PeerId = peer_id.trim().parse().map_err(|_| NetError::InvalidPeerId)?;
        let request = nexus_protocol::encode_index_request(request).map_err(|_| NetError::InvalidRecord)?;
        let (respond, receiver) = oneshot::channel();
        self.send(Command::FetchIndex {
            peer_id,
            request,
            respond,
        })?;
        receiver.blocking_recv().unwrap_or(Err(NetError::NotRunning))
    }

    /// Runs one bounded peer-routing query.
    ///
    /// `target` is a 32-byte DHT key; `None` selects a random target so a
    /// fresh node samples the network instead of repeatedly probing one point.
    pub fn discover_peers(&self, target: Option<&[u8]>) -> Result<PeerDiscovery, NetError> {
        let target = match target {
            Some(target) => Some(dht_key(target)?),
            None => None,
        };
        let (respond, receiver) = oneshot::channel();
        self.send(Command::DhtDiscover { target, respond })?;
        receiver.blocking_recv().unwrap_or(Err(NetError::NotRunning))
    }

    /// Publishes one exact signed DHT envelope under its derived key.
    ///
    /// The value is verified before it is sent so a malformed record can never
    /// reach the network from this node.
    pub fn publish_record(&self, key: &[u8], value: &[u8]) -> Result<(), NetError> {
        let key = dht_key(key)?;
        DhtRecord::verify_for_key(&key, value).map_err(|_| NetError::InvalidRecord)?;
        let (respond, receiver) = oneshot::channel();
        self.send(Command::DhtPublish {
            key,
            value: value.to_vec(),
            respond,
        })?;
        receiver.blocking_recv().unwrap_or(Err(NetError::NotRunning))
    }

    /// Looks up one DHT value and verifies it against the requested key.
    pub fn lookup_record(&self, key: &[u8]) -> Result<Option<DhtValue>, NetError> {
        let key = dht_key(key)?;
        let (respond, receiver) = oneshot::channel();
        self.send(Command::DhtLookup { key, respond })?;
        receiver.blocking_recv().unwrap_or(Err(NetError::NotRunning))
    }

    /// Announces this node as a provider for one 32-byte content ID.
    pub fn start_providing(&self, content_id: &[u8]) -> Result<(), NetError> {
        let key = dht_key(content_id)?;
        let (respond, receiver) = oneshot::channel();
        self.send(Command::DhtStartProviding { key, respond })?;
        receiver.blocking_recv().unwrap_or(Err(NetError::NotRunning))
    }

    /// Withdraws this node as a provider for one 32-byte content ID.
    ///
    /// The local provider record is removed immediately. Remote peers may keep
    /// a cached provider observation until it expires, so withdrawal is not a
    /// global deletion promise.
    pub fn stop_providing(&self, content_id: &[u8]) -> Result<(), NetError> {
        let key = dht_key(content_id)?;
        let (respond, receiver) = oneshot::channel();
        self.send(Command::DhtStopProviding { key, respond })?;
        receiver.blocking_recv().unwrap_or(Err(NetError::NotRunning))
    }

    /// Finds providers for one 32-byte content ID.
    pub fn find_providers(&self, content_id: &[u8]) -> Result<Vec<DiscoveredPeer>, NetError> {
        let key = dht_key(content_id)?;
        let (respond, receiver) = oneshot::channel();
        self.send(Command::DhtFindProviders { key, respond })?;
        receiver.blocking_recv().unwrap_or(Err(NetError::NotRunning))
    }

    /// Adds a bootstrap or manual peer and persists the list.
    pub fn bootstrap_add(&self, address: &str) -> Result<(), NetError> {
        let (_, address) = parse_peer_address(address)?;
        let (respond, receiver) = oneshot::channel();
        self.send(Command::BootstrapAdd { address, respond })?;
        receiver.blocking_recv().unwrap_or(Err(NetError::NotRunning))
    }

    /// Removes a bootstrap or manual peer and persists the list.
    pub fn bootstrap_remove(&self, address: &str) -> Result<(), NetError> {
        let (_, address) = parse_peer_address(address)?;
        let (respond, receiver) = oneshot::channel();
        self.send(Command::BootstrapRemove { address, respond })?;
        receiver.blocking_recv().unwrap_or(Err(NetError::NotRunning))
    }

    /// Persisted community relay addresses.
    #[must_use]
    pub fn relay_list(&self) -> Vec<String> {
        self.shared.relays()
    }

    /// Adds a community relay and persists the list.
    pub fn relay_add(&self, address: &str) -> Result<(), NetError> {
        let (_, address) = parse_peer_address(address)?;
        let (respond, receiver) = oneshot::channel();
        self.send(Command::RelayAdd { address, respond })?;
        receiver.blocking_recv().unwrap_or(Err(NetError::NotRunning))
    }

    /// Removes a community relay and persists the list.
    pub fn relay_remove(&self, address: &str) -> Result<(), NetError> {
        let (_, address) = parse_peer_address(address)?;
        let (respond, receiver) = oneshot::channel();
        self.send(Command::RelayRemove { address, respond })?;
        receiver.blocking_recv().unwrap_or(Err(NetError::NotRunning))
    }

    /// Re-dials one target peer, or every target peer when `None`.
    pub fn reconnect(&self, peer_id: Option<&str>) -> Result<(), NetError> {
        let peer_id = match peer_id {
            Some(text) => Some(text.trim().parse().map_err(|_| NetError::InvalidPeerId)?),
            None => None,
        };
        let (respond, receiver) = oneshot::channel();
        self.send(Command::Reconnect { peer_id, respond })?;
        receiver.blocking_recv().unwrap_or(Err(NetError::NotRunning))
    }

    /// Tells the driver that the network path changed (Wi-Fi switch or
    /// sleep/wake). Returns once every target's reconnect backoff is reset.
    pub fn notify_network_change(&self) -> Result<(), NetError> {
        let (respond, receiver) = oneshot::channel();
        self.send(Command::NetworkChanged { respond })?;
        receiver.blocking_recv().map_err(|_| NetError::NotRunning)
    }

    /// Changes the served-file upload rate. Zero disables throttling.
    pub fn set_upload_rate(&self, bytes_per_second: u64) -> Result<(), NetError> {
        if bytes_per_second != 0
            && !(MIN_UPLOAD_BYTES_PER_SECOND..=MAX_UPLOAD_BYTES_PER_SECOND).contains(&bytes_per_second)
        {
            return Err(NetError::InvalidConfig("upload_bytes_per_second"));
        }
        let (respond, receiver) = oneshot::channel();
        self.send(Command::SetUploadRate {
            bytes_per_second,
            respond,
        })?;
        receiver.blocking_recv().map_err(|_| NetError::NotRunning)
    }

    /// Stops the driver and waits for its thread to exit. Idempotent.
    pub fn shutdown(&self) {
        let handle = self
            .thread
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        let Some(handle) = handle else {
            return;
        };
        let (respond, receiver) = oneshot::channel();
        if self.commands.blocking_send(Command::Shutdown { respond }).is_ok() {
            let _ = receiver.blocking_recv();
        }
        let _ = handle.join();
    }

    fn send(&self, command: Command) -> Result<(), NetError> {
        match self.commands.try_send(command) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) => Err(NetError::Busy),
            Err(mpsc::error::TrySendError::Closed(_)) => Err(NetError::NotRunning),
        }
    }
}

impl Drop for Network {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl std::fmt::Debug for Network {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Network")
            .field("peer_id", &self.peer_id)
            .field("state", &self.shared.status().state)
            .finish()
    }
}

fn parse_peer_address(text: &str) -> Result<(PeerId, Multiaddr), NetError> {
    let address: Multiaddr = text.trim().parse().map_err(|_| NetError::InvalidAddress)?;
    let (peer_id, address) = config::split_peer_address(&address)?;
    Ok((peer_id, address))
}

fn dht_key(key: &[u8]) -> Result<[u8; DHT_KEY_BYTES], NetError> {
    key.try_into().map_err(|_| NetError::InvalidKey)
}
