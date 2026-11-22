//! Background swarm driver: commands, deadlines, reconnect backoff, and events.
//!
//! One OS thread owns a current-thread Tokio runtime and the libp2p swarm.
//! Synchronous callers submit bounded commands and block on a one-shot reply;
//! the driver never blocks the swarm loop on a caller.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::StreamExt;
use libp2p::core::ConnectedPoint;
use libp2p::core::transport::ListenerId;
use libp2p::kad::store::RecordStore;
use libp2p::kad::{self, ProgressStep, ProviderRecord, Quorum, Record, RecordKey};
use libp2p::multiaddr::Protocol;
use libp2p::request_response::{self, OutboundFailure, OutboundRequestId, ResponseChannel};
use libp2p::swarm::dial_opts::{DialOpts, PeerCondition};
use libp2p::swarm::{DialError, SwarmEvent};
use libp2p::{Multiaddr, PeerId, Swarm, autonat, identify, ping, relay};
use nexus_protocol::{
    DHT_KEY_BYTES, DHT_MAX_VALUE_BYTES, DhtRecord, DhtRecordDecision, FileRequest, FileResponse, FileSelector,
    TestResponse, decode_file_request, decode_file_response, decode_test_request, decode_test_response,
    encode_file_request, encode_file_response, encode_test_request, encode_test_response, validate_provider_addresses,
    validate_provider_key,
};
use tokio::sync::{mpsc, oneshot};

use crate::behaviour::{Behaviour, BehaviourEvent};
use crate::bootstrap;
use crate::config::{NetworkConfig, split_peer_address};
use crate::error::NetError;
use crate::file::{FileMetadata, FileProvider};
use crate::relay as relay_list;
use crate::shared::Shared;
use crate::state::{ConnectedPeer, DhtValue, DiscoveredPeer, NetworkState, PeerDiscovery, TestReply};

/// Bounded command queue depth.
pub(crate) const COMMAND_CAPACITY: usize = 64;
/// Driver tick for deadlines, reconnect backoff, and state refresh.
const TICK: Duration = Duration::from_millis(250);
/// Extra grace beyond the request timeout before the driver fails a request.
const REQUEST_GRACE: Duration = Duration::from_secs(1);
/// Maximum retained per-peer rate-limit windows.
const MAX_RATE_WINDOWS: usize = 256;
/// Minimum delay between forced Kademlia bootstrap attempts.
const BOOTSTRAP_RETRY: Duration = Duration::from_secs(10);
/// Maximum peers returned by one discovery query.
const MAX_DISCOVERED_PEERS: usize = 32;
/// Maximum addresses retained for one discovered peer.
const MAX_DISCOVERED_ADDRESSES: usize = 8;
/// Minimum delay before retrying a refused relay reservation.
const RELAY_RETRY: Duration = Duration::from_secs(5);
/// Inbound file requests allowed per peer per minute. A 64 KiB slice means
/// this permits roughly 12 MiB/s per peer while still bounding abuse.
const MAX_FILE_REQUESTS_PER_PEER_PER_MINUTE: u32 = 12_000;

pub(crate) enum Command {
    Dial {
        peer_id: PeerId,
        address: Multiaddr,
        respond: oneshot::Sender<Result<ConnectedPeer, NetError>>,
    },
    SendTest {
        peer_id: PeerId,
        payload: Vec<u8>,
        respond: oneshot::Sender<Result<TestReply, NetError>>,
    },
    FetchFileMetadata {
        peer_id: PeerId,
        selector: FileSelector,
        respond: oneshot::Sender<Result<FileMetadata, NetError>>,
    },
    FetchFileChunk {
        peer_id: PeerId,
        content_id: [u8; 32],
        index: u32,
        offset: u32,
        length: u32,
        respond: oneshot::Sender<Result<Vec<u8>, NetError>>,
    },
    DhtDiscover {
        target: Option<[u8; DHT_KEY_BYTES]>,
        respond: oneshot::Sender<Result<PeerDiscovery, NetError>>,
    },
    DhtPublish {
        key: [u8; DHT_KEY_BYTES],
        value: Vec<u8>,
        respond: oneshot::Sender<Result<(), NetError>>,
    },
    DhtLookup {
        key: [u8; DHT_KEY_BYTES],
        respond: oneshot::Sender<Result<Option<DhtValue>, NetError>>,
    },
    DhtStartProviding {
        key: [u8; DHT_KEY_BYTES],
        respond: oneshot::Sender<Result<(), NetError>>,
    },
    DhtStopProviding {
        key: [u8; DHT_KEY_BYTES],
        respond: oneshot::Sender<Result<(), NetError>>,
    },
    DhtFindProviders {
        key: [u8; DHT_KEY_BYTES],
        respond: oneshot::Sender<Result<Vec<DiscoveredPeer>, NetError>>,
    },
    BootstrapAdd {
        address: Multiaddr,
        respond: oneshot::Sender<Result<(), NetError>>,
    },
    BootstrapRemove {
        address: Multiaddr,
        respond: oneshot::Sender<Result<(), NetError>>,
    },
    RelayAdd {
        address: Multiaddr,
        respond: oneshot::Sender<Result<(), NetError>>,
    },
    RelayRemove {
        address: Multiaddr,
        respond: oneshot::Sender<Result<(), NetError>>,
    },
    Reconnect {
        peer_id: Option<PeerId>,
        respond: oneshot::Sender<Result<(), NetError>>,
    },
    NetworkChanged {
        respond: oneshot::Sender<()>,
    },
    Shutdown {
        respond: oneshot::Sender<()>,
    },
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum DialPhase {
    Direct,
    Relay,
}

struct PendingDial {
    respond: oneshot::Sender<Result<ConnectedPeer, NetError>>,
    relay_candidates: VecDeque<Multiaddr>,
    phase: DialPhase,
    deadline: Instant,
    direct_code: Option<&'static str>,
    relay_attempted: bool,
}

#[derive(Clone, Copy, Default)]
struct RelayLimit {
    duration: Option<Duration>,
    bytes: Option<u64>,
}

#[derive(Default)]
struct RelayState {
    reservation: bool,
    listener: Option<ListenerId>,
    pending_since: Option<Instant>,
    retry_at: Option<Instant>,
    limit: RelayLimit,
}

struct PendingRequest {
    respond: oneshot::Sender<Result<TestReply, NetError>>,
    peer_id: PeerId,
    started: Instant,
    deadline: Instant,
}

enum PendingFileRequest {
    Metadata {
        respond: oneshot::Sender<Result<FileMetadata, NetError>>,
        deadline: Instant,
    },
    Chunk {
        respond: oneshot::Sender<Result<Vec<u8>, NetError>>,
        deadline: Instant,
    },
}

impl PendingFileRequest {
    fn deadline(&self) -> Instant {
        match self {
            Self::Metadata { deadline, .. } | Self::Chunk { deadline, .. } => *deadline,
        }
    }
}

enum PendingQuery {
    Discover {
        respond: oneshot::Sender<Result<PeerDiscovery, NetError>>,
        started: Instant,
        deadline: Instant,
    },
    Publish {
        respond: oneshot::Sender<Result<(), NetError>>,
        deadline: Instant,
    },
    Lookup {
        key: [u8; DHT_KEY_BYTES],
        respond: oneshot::Sender<Result<Option<DhtValue>, NetError>>,
        deadline: Instant,
    },
    StartProviding {
        respond: oneshot::Sender<Result<(), NetError>>,
        deadline: Instant,
    },
    FindProviders {
        key: [u8; DHT_KEY_BYTES],
        respond: oneshot::Sender<Result<Vec<DiscoveredPeer>, NetError>>,
        providers: Vec<PeerId>,
        deadline: Instant,
    },
}

impl PendingQuery {
    fn deadline(&self) -> Instant {
        match self {
            Self::Discover { deadline, .. }
            | Self::Publish { deadline, .. }
            | Self::Lookup { deadline, .. }
            | Self::StartProviding { deadline, .. }
            | Self::FindProviders { deadline, .. } => *deadline,
        }
    }
}

struct Backoff {
    attempts: u32,
    next_at: Instant,
}

struct RateWindow {
    started: Instant,
    count: u32,
}

pub(crate) struct Driver {
    swarm: Swarm<Behaviour>,
    shared: Arc<Shared>,
    config: Arc<NetworkConfig>,
    commands: mpsc::Receiver<Command>,
    pending_dials: HashMap<PeerId, PendingDial>,
    pending_requests: HashMap<OutboundRequestId, PendingRequest>,
    pending_file_requests: HashMap<OutboundRequestId, PendingFileRequest>,
    pending_queries: HashMap<kad::QueryId, PendingQuery>,
    backoff: HashMap<PeerId, Backoff>,
    targets: BTreeMap<PeerId, Vec<Multiaddr>>,
    relays: BTreeMap<PeerId, Multiaddr>,
    relay_state: HashMap<PeerId, RelayState>,
    background_relay_attempted: HashSet<PeerId>,
    rate: HashMap<PeerId, RateWindow>,
    file_rate: HashMap<PeerId, RateWindow>,
    file_provider: Arc<dyn FileProvider>,
    dht_bootstrapped: bool,
    last_bootstrap: Option<Instant>,
    routing_peers: usize,
}

impl Driver {
    /// Builds the swarm, starts listeners, and dials configured bootstrap peers.
    pub(crate) fn start(
        config: Arc<NetworkConfig>,
        keypair: libp2p::identity::Keypair,
        shared: Arc<Shared>,
        commands: mpsc::Receiver<Command>,
        file_provider: Arc<dyn FileProvider>,
    ) -> Result<Self, NetError> {
        let mut targets: BTreeMap<PeerId, Vec<Multiaddr>> = BTreeMap::new();
        for address in &config.bootstrap {
            let (peer_id, address) = split_peer_address(address)?;
            targets.entry(peer_id).or_default().push(address);
        }
        let mut relays: BTreeMap<PeerId, Multiaddr> = BTreeMap::new();
        for address in &config.relays {
            let (peer_id, _) = split_peer_address(address)?;
            relays.insert(peer_id, address.clone());
            targets.entry(peer_id).or_default().push(address.clone());
        }

        let idle_timeout = config.idle_connection_timeout;
        let behaviour_config = Arc::clone(&config);
        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(keypair)
            .with_tokio()
            .with_tcp(
                libp2p::tcp::Config::default().nodelay(true),
                libp2p::noise::Config::new,
                libp2p::yamux::Config::default,
            )
            .map_err(|_| NetError::DriverFailed)?
            .with_quic_config(|mut quic| {
                quic.max_idle_timeout = u32::try_from(idle_timeout.as_millis()).unwrap_or(u32::MAX);
                quic.keep_alive_interval = idle_timeout / 3;
                quic
            })
            .with_dns()
            .map_err(|_| NetError::DriverFailed)?
            .with_relay_client(libp2p::noise::Config::new, libp2p::yamux::Config::default)
            .map_err(|_| NetError::DriverFailed)?
            .with_behaviour(|key, relay_client| {
                Ok::<Behaviour, Box<dyn std::error::Error + Send + Sync>>(Behaviour::new(
                    key,
                    &behaviour_config,
                    relay_client,
                ))
            })
            .map_err(|_| NetError::DriverFailed)?
            .with_swarm_config(|swarm| swarm.with_idle_connection_timeout(idle_timeout))
            .build();

        let mut listeners = 0_usize;
        for address in config.effective_listen()? {
            match swarm.listen_on(address) {
                Ok(_) => listeners += 1,
                Err(_) => shared.record_last_error("listen_failed"),
            }
        }
        if listeners == 0 {
            return Err(NetError::Unavailable);
        }
        // A relay server is advertised only once libp2p knows a reachable
        // external address. Operators supply those explicitly; loopback
        // listen addresses are registered locally in `NewListenAddr`.
        for address in &config.external_addresses {
            swarm.add_external_address(address.clone());
        }

        let mut driver = Self {
            swarm,
            shared,
            config,
            commands,
            pending_dials: HashMap::new(),
            pending_requests: HashMap::new(),
            pending_file_requests: HashMap::new(),
            pending_queries: HashMap::new(),
            backoff: HashMap::new(),
            targets,
            relays,
            relay_state: HashMap::new(),
            background_relay_attempted: HashSet::new(),
            rate: HashMap::new(),
            file_rate: HashMap::new(),
            file_provider,
            dht_bootstrapped: false,
            last_bootstrap: None,
            routing_peers: 0,
        };
        driver.shared.set_dht_mode(driver.config.dht_mode.as_str());
        driver.shared.configure_relay(
            driver.config.relay_client_enabled,
            driver.config.relay_fallback,
            driver.config.relay_server_enabled,
            driver.config.relay_max_circuit_bytes,
            driver.config.relay_max_circuit_duration,
        );

        let peers: Vec<PeerId> = driver.targets.keys().copied().collect();
        for peer_id in peers {
            let addresses = driver.targets.get(&peer_id).cloned().unwrap_or_default();
            for address in &addresses {
                driver
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .add_address(&peer_id, address.clone());
            }
            driver.dial_background(peer_id, addresses);
        }

        Ok(driver)
    }

    /// Runs until shutdown or until every command sender is dropped.
    pub(crate) async fn run(mut self) {
        let mut ticker = tokio::time::interval(TICK);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                event = self.swarm.select_next_some() => self.on_swarm_event(event),
                command = self.commands.recv() => match command {
                    Some(Command::Shutdown { respond }) => {
                        self.shared.force_state(NetworkState::Stopping, Some("shutdown"));
                        self.pending_dials.clear();
                        self.pending_requests.clear();
                        self.pending_file_requests.clear();
                        self.pending_queries.clear();
                        let _ = respond.send(());
                        return;
                    }
                    Some(command) => self.on_command(command),
                    None => return,
                },
                _ = ticker.tick() => self.on_tick(),
            }
        }
    }

    fn on_command(&mut self, command: Command) {
        match command {
            Command::Dial {
                peer_id,
                address,
                respond,
            } => self.dial_foreground(peer_id, address, respond),
            Command::SendTest {
                peer_id,
                payload,
                respond,
            } => self.send_test(peer_id, payload, respond),
            Command::FetchFileMetadata {
                peer_id,
                selector,
                respond,
            } => self.fetch_file_metadata(peer_id, selector, respond),
            Command::FetchFileChunk {
                peer_id,
                content_id,
                index,
                offset,
                length,
                respond,
            } => self.fetch_file_chunk(peer_id, content_id, index, offset, length, respond),
            Command::DhtDiscover { target, respond } => self.dht_discover(target, respond),
            Command::DhtPublish { key, value, respond } => self.dht_publish(key, value, respond),
            Command::DhtLookup { key, respond } => self.dht_lookup(key, respond),
            Command::DhtStartProviding { key, respond } => self.dht_start_providing(key, respond),
            Command::DhtStopProviding { key, respond } => self.dht_stop_providing(key, respond),
            Command::DhtFindProviders { key, respond } => self.dht_find_providers(key, respond),
            Command::BootstrapAdd { address, respond } => self.bootstrap_add(address, respond),
            Command::BootstrapRemove { address, respond } => self.bootstrap_remove(&address, respond),
            Command::RelayAdd { address, respond } => self.relay_add(address, respond),
            Command::RelayRemove { address, respond } => self.relay_remove(&address, respond),
            Command::Reconnect { peer_id, respond } => {
                let result = self.reconnect(peer_id);
                let _ = respond.send(result);
            }
            Command::NetworkChanged { respond } => {
                self.network_changed();
                let _ = respond.send(());
            }
            Command::Shutdown { respond } => {
                let _ = respond.send(());
            }
        }
    }

    fn dial_foreground(
        &mut self,
        peer_id: PeerId,
        address: Multiaddr,
        respond: oneshot::Sender<Result<ConnectedPeer, NetError>>,
    ) {
        if self.pending_dials.contains_key(&peer_id) {
            let _ = respond.send(Err(NetError::Busy));
            return;
        }
        let relay_candidates = self.relay_candidates(peer_id);
        self.shared.record_dialing(&peer_id.to_string());
        self.swarm.add_peer_address(peer_id, address.clone());
        let options = DialOpts::peer_id(peer_id)
            .addresses(vec![address.clone()])
            .condition(PeerCondition::Always)
            .build();
        match self.swarm.dial(options) {
            Ok(()) => {
                self.targets.insert(peer_id, vec![address]);
                self.backoff.remove(&peer_id);
                self.pending_dials.insert(
                    peer_id,
                    PendingDial {
                        respond,
                        relay_candidates,
                        phase: DialPhase::Direct,
                        deadline: Instant::now() + self.config.connect_timeout,
                        direct_code: None,
                        relay_attempted: false,
                    },
                );
            }
            Err(error) => {
                let (code, mapped) = map_dial_error(&error);
                self.shared.record_failed(&peer_id.to_string(), code);
                let _ = respond.send(Err(mapped));
            }
        }
    }

    fn dial_background(&mut self, peer_id: PeerId, addresses: Vec<Multiaddr>) {
        if addresses.is_empty() || self.swarm.is_connected(&peer_id) {
            return;
        }
        self.shared.record_dialing(&peer_id.to_string());
        for address in &addresses {
            self.swarm.add_peer_address(peer_id, address.clone());
        }
        let options = DialOpts::peer_id(peer_id).addresses(addresses).build();
        if let Err(error) = self.swarm.dial(options) {
            let (code, _) = map_dial_error(&error);
            self.shared.record_failed(&peer_id.to_string(), code);
            self.schedule_retry(&peer_id);
        }
    }

    /// Relayed circuit addresses for one target, using relays that are
    /// currently connected. A relay never relays to itself.
    fn relay_candidates(&self, peer_id: PeerId) -> VecDeque<Multiaddr> {
        if !self.config.relay_client_enabled || !self.config.relay_fallback {
            return VecDeque::new();
        }
        self.relays
            .iter()
            .filter(|(relay, _)| **relay != peer_id && self.swarm.is_connected(relay))
            .take(crate::config::MAX_RELAYS)
            .map(|(_, address)| {
                let mut circuit = address.clone();
                circuit.push(Protocol::P2pCircuit);
                circuit.push(Protocol::P2p(peer_id));
                circuit
            })
            .collect()
    }

    fn start_relay_phase(&mut self, peer_id: PeerId, direct_code: &'static str) {
        let relay_available = {
            let candidates = self.relay_candidates(peer_id);
            let Some(pending) = self.pending_dials.get_mut(&peer_id) else {
                return;
            };
            if pending.relay_candidates.is_empty() {
                pending.relay_candidates = candidates;
            }
            if pending.relay_candidates.is_empty() {
                false
            } else {
                pending.phase = DialPhase::Relay;
                pending.direct_code = Some(direct_code);
                pending.relay_attempted = true;
                pending.deadline = Instant::now() + self.config.relay_reservation_timeout;
                true
            }
        };
        if !relay_available {
            self.fail_direct_dial(peer_id, direct_code);
            return;
        }
        self.shared.record_fallback_attempt();
        if !self.dial_next_relay(peer_id) {
            self.fail_no_viable_path(peer_id);
        }
    }

    /// Dials the next relayed candidate. Returns false when none are left.
    fn dial_next_relay(&mut self, peer_id: PeerId) -> bool {
        loop {
            let address = self
                .pending_dials
                .get_mut(&peer_id)
                .and_then(|pending| pending.relay_candidates.pop_front());
            let Some(address) = address else {
                return false;
            };
            self.swarm.add_peer_address(peer_id, address.clone());
            let options = DialOpts::peer_id(peer_id)
                .addresses(vec![address])
                .condition(PeerCondition::Always)
                .build();
            match self.swarm.dial(options) {
                Ok(()) => {
                    if let Some(pending) = self.pending_dials.get_mut(&peer_id) {
                        pending.deadline = Instant::now() + self.config.relay_reservation_timeout;
                    }
                    return true;
                }
                Err(error) => {
                    let (code, _) = map_dial_error(&error);
                    self.shared.record_last_error(code);
                }
            }
        }
    }

    fn fail_direct_dial(&mut self, peer_id: PeerId, code: &'static str) {
        let Some(pending) = self.pending_dials.remove(&peer_id) else {
            return;
        };
        let error = error_for_dial_code(code);
        self.shared.record_failed(&peer_id.to_string(), code);
        let _ = pending.respond.send(Err(error));
        self.schedule_retry(&peer_id);
    }

    fn fail_no_viable_path(&mut self, peer_id: PeerId) {
        let Some(pending) = self.pending_dials.remove(&peer_id) else {
            return;
        };
        self.shared.record_failed(&peer_id.to_string(), "no_viable_path");
        let _ = pending.respond.send(Err(NetError::NoViablePath));
        self.schedule_retry(&peer_id);
    }

    fn background_relay_fallback(&mut self, peer_id: PeerId) {
        if !self.config.relay_fallback
            || !self.config.relay_client_enabled
            || self.relays.contains_key(&peer_id)
            || !self.targets.contains_key(&peer_id)
            || self.pending_dials.contains_key(&peer_id)
            || !self.background_relay_attempted.insert(peer_id)
        {
            return;
        }
        let candidates = self.relay_candidates(peer_id);
        if candidates.is_empty() {
            self.background_relay_attempted.remove(&peer_id);
            return;
        }
        let addresses: Vec<Multiaddr> = candidates.into_iter().collect();
        for address in &addresses {
            self.swarm.add_peer_address(peer_id, address.clone());
        }
        self.shared.record_fallback_attempt();
        let options = DialOpts::peer_id(peer_id).addresses(addresses).build();
        if self.swarm.dial(options).is_err() {
            self.shared.record_last_error("relay_fallback_failed");
            self.background_relay_attempted.remove(&peer_id);
        }
    }

    /// Publishes the number of connected relays and active reservations.
    fn refresh_relay_counts(&self) {
        let connected = self
            .relays
            .keys()
            .filter(|peer_id| self.swarm.is_connected(peer_id))
            .count();
        let reservations = self.relay_state.values().filter(|state| state.reservation).count();
        self.shared.set_relay_counts(connected, reservations);
    }

    /// Requests our own reservation on a connected relay.
    ///
    /// libp2p renews the reservation for as long as the listener lives, so a
    /// reservation is requested once per connection and retried only after a
    /// bounded failure delay.
    fn ensure_relay_reservation(&mut self, relay_peer_id: PeerId) {
        if !self.config.relay_client_enabled || !self.swarm.is_connected(&relay_peer_id) {
            return;
        }
        let Some(address) = self.relays.get(&relay_peer_id).cloned() else {
            return;
        };
        if self
            .relay_state
            .get(&relay_peer_id)
            .is_some_and(|state| state.listener.is_some())
        {
            return;
        }
        let circuit = {
            let mut circuit = address;
            circuit.push(Protocol::P2pCircuit);
            circuit
        };
        let now = Instant::now();
        match self.swarm.listen_on(circuit) {
            Ok(listener) => {
                let state = self.relay_state.entry(relay_peer_id).or_default();
                state.listener = Some(listener);
                state.pending_since = Some(now);
                state.retry_at = None;
            }
            Err(_) => {
                let state = self.relay_state.entry(relay_peer_id).or_default();
                state.pending_since = None;
                state.retry_at = Some(now + RELAY_RETRY);
                self.shared.record_relay_reservation(false);
                self.shared.record_last_error("relay_reservation_failed");
            }
        }
        self.refresh_relay_counts();
    }

    /// Fails reservations that did not complete within the configured deadline.
    fn expire_relay_reservations(&mut self, now: Instant) {
        let timeout = self.config.relay_reservation_timeout;
        let expired: Vec<PeerId> = self
            .relay_state
            .iter()
            .filter(|(_, state)| {
                state
                    .pending_since
                    .is_some_and(|started| now.saturating_duration_since(started) >= timeout)
            })
            .map(|(relay, _)| *relay)
            .collect();
        for relay_peer_id in expired {
            let listener = self.relay_state.get_mut(&relay_peer_id).and_then(|state| {
                state.pending_since = None;
                state.retry_at = Some(now + RELAY_RETRY);
                state.listener.take()
            });
            if let Some(listener) = listener {
                let _ = self.swarm.remove_listener(listener);
            }
            self.shared.record_relay_reservation(false);
            self.shared.record_last_error("relay_reservation_timeout");
        }
    }

    /// Retries reservations after a bounded delay once the relay is connected.
    fn retry_relay_reservations(&mut self, now: Instant) {
        let due: Vec<PeerId> = self
            .relay_state
            .iter()
            .filter(|(relay, state)| {
                state.listener.is_none()
                    && state.retry_at.is_some_and(|retry_at| retry_at <= now)
                    && self.swarm.is_connected(relay)
            })
            .map(|(relay, _)| *relay)
            .collect();
        for relay_peer_id in due {
            if let Some(state) = self.relay_state.get_mut(&relay_peer_id) {
                state.retry_at = None;
            }
            self.ensure_relay_reservation(relay_peer_id);
        }
    }

    /// Relay peer ID encoded before `/p2p-circuit` in a relayed address.
    fn relay_for_circuit(address: &Multiaddr) -> Option<PeerId> {
        let mut previous = None;
        for protocol in address.iter() {
            if protocol == Protocol::P2pCircuit {
                return previous;
            }
            previous = match protocol {
                Protocol::P2p(peer_id) => Some(peer_id),
                _ => None,
            };
        }
        None
    }

    fn handle_outgoing_error(&mut self, peer_id: PeerId, error: &DialError) {
        let (code, _) = map_dial_error(error);
        self.shared.record_failed(&peer_id.to_string(), code);
        match self.pending_dials.get(&peer_id).map(|pending| pending.phase) {
            Some(DialPhase::Direct) => self.start_relay_phase(peer_id, code),
            Some(DialPhase::Relay) => {
                if !self.dial_next_relay(peer_id) {
                    self.fail_no_viable_path(peer_id);
                }
            }
            None => {
                if self.targets.contains_key(&peer_id) {
                    self.background_relay_fallback(peer_id);
                    self.schedule_retry(&peer_id);
                }
            }
        }
    }

    fn fetch_file_metadata(
        &mut self,
        peer_id: PeerId,
        selector: FileSelector,
        respond: oneshot::Sender<Result<FileMetadata, NetError>>,
    ) {
        if !self.shared.is_connected(&peer_id.to_string()) {
            let _ = respond.send(Err(NetError::NotConnected));
            return;
        }
        let request = match encode_file_request(&FileRequest::Metadata(selector)) {
            Ok(request) => request,
            Err(_) => {
                let _ = respond.send(Err(NetError::MessageTooLarge));
                return;
            }
        };
        let now = Instant::now();
        let request_id = self.swarm.behaviour_mut().file.send_request(&peer_id, request);
        self.pending_file_requests.insert(
            request_id,
            PendingFileRequest::Metadata {
                respond,
                deadline: now + self.config.request_timeout + REQUEST_GRACE,
            },
        );
    }

    fn fetch_file_chunk(
        &mut self,
        peer_id: PeerId,
        content_id: [u8; 32],
        index: u32,
        offset: u32,
        length: u32,
        respond: oneshot::Sender<Result<Vec<u8>, NetError>>,
    ) {
        if !self.shared.is_connected(&peer_id.to_string()) {
            let _ = respond.send(Err(NetError::NotConnected));
            return;
        }
        let request = FileRequest::Chunk(nexus_protocol::ChunkRequest {
            content_id,
            index,
            offset,
            length,
        });
        let request = match encode_file_request(&request) {
            Ok(request) => request,
            Err(_) => {
                let _ = respond.send(Err(NetError::MessageTooLarge));
                return;
            }
        };
        let now = Instant::now();
        let request_id = self.swarm.behaviour_mut().file.send_request(&peer_id, request);
        self.pending_file_requests.insert(
            request_id,
            PendingFileRequest::Chunk {
                respond,
                deadline: now + self.config.request_timeout + REQUEST_GRACE,
            },
        );
    }

    fn send_test(&mut self, peer_id: PeerId, payload: Vec<u8>, respond: oneshot::Sender<Result<TestReply, NetError>>) {
        if !self.shared.is_connected(&peer_id.to_string()) {
            let _ = respond.send(Err(NetError::NotConnected));
            return;
        }
        let request = match encode_test_request(&payload) {
            Ok(request) => request,
            Err(_) => {
                let _ = respond.send(Err(NetError::MessageTooLarge));
                return;
            }
        };
        let now = Instant::now();
        let request_id = self.swarm.behaviour_mut().test.send_request(&peer_id, request);
        self.pending_requests.insert(
            request_id,
            PendingRequest {
                respond,
                peer_id,
                started: now,
                deadline: now + self.config.request_timeout + REQUEST_GRACE,
            },
        );
    }

    fn dht_discover(
        &mut self,
        target: Option<[u8; DHT_KEY_BYTES]>,
        respond: oneshot::Sender<Result<PeerDiscovery, NetError>>,
    ) {
        let target = match target {
            Some(target) => target,
            None => match nexus_crypto::random_array::<DHT_KEY_BYTES>() {
                Ok(target) => target,
                Err(error) => {
                    let _ = respond.send(Err(NetError::Crypto(error)));
                    return;
                }
            },
        };
        let query = self.swarm.behaviour_mut().kademlia.get_closest_peers(target.to_vec());
        self.shared.record_dht_query_started();
        self.pending_queries.insert(
            query,
            PendingQuery::Discover {
                respond,
                started: Instant::now(),
                deadline: self.dht_deadline(),
            },
        );
    }

    fn dht_publish(
        &mut self,
        key: [u8; DHT_KEY_BYTES],
        value: Vec<u8>,
        respond: oneshot::Sender<Result<(), NetError>>,
    ) {
        if value.len() > DHT_MAX_VALUE_BYTES {
            let _ = respond.send(Err(NetError::RecordTooLarge));
            return;
        }
        let record = Record::new(RecordKey::new(&key), value);
        let query = match self.swarm.behaviour_mut().kademlia.put_record(record, Quorum::One) {
            Ok(query) => query,
            Err(_) => {
                let _ = respond.send(Err(NetError::RecordTooLarge));
                return;
            }
        };
        self.shared.record_dht_query_started();
        self.pending_queries.insert(
            query,
            PendingQuery::Publish {
                respond,
                deadline: self.dht_deadline(),
            },
        );
    }

    fn dht_lookup(&mut self, key: [u8; DHT_KEY_BYTES], respond: oneshot::Sender<Result<Option<DhtValue>, NetError>>) {
        let query = self.swarm.behaviour_mut().kademlia.get_record(RecordKey::new(&key));
        self.shared.record_dht_query_started();
        self.pending_queries.insert(
            query,
            PendingQuery::Lookup {
                key,
                respond,
                deadline: self.dht_deadline(),
            },
        );
    }

    fn dht_start_providing(&mut self, key: [u8; DHT_KEY_BYTES], respond: oneshot::Sender<Result<(), NetError>>) {
        if validate_provider_key(&key).is_err() {
            let _ = respond.send(Err(NetError::InvalidKey));
            return;
        }
        let query = match self
            .swarm
            .behaviour_mut()
            .kademlia
            .start_providing(RecordKey::new(&key))
        {
            Ok(query) => query,
            Err(_) => {
                let _ = respond.send(Err(NetError::Busy));
                return;
            }
        };
        self.shared.record_dht_query_started();
        self.pending_queries.insert(
            query,
            PendingQuery::StartProviding {
                respond,
                deadline: self.dht_deadline(),
            },
        );
    }

    fn dht_stop_providing(&mut self, key: [u8; DHT_KEY_BYTES], respond: oneshot::Sender<Result<(), NetError>>) {
        if validate_provider_key(&key).is_err() {
            let _ = respond.send(Err(NetError::InvalidKey));
            return;
        }
        self.swarm
            .behaviour_mut()
            .kademlia
            .stop_providing(&RecordKey::new(&key));
        let _ = respond.send(Ok(()));
    }

    fn dht_find_providers(
        &mut self,
        key: [u8; DHT_KEY_BYTES],
        respond: oneshot::Sender<Result<Vec<DiscoveredPeer>, NetError>>,
    ) {
        if validate_provider_key(&key).is_err() {
            let _ = respond.send(Err(NetError::InvalidKey));
            return;
        }
        let query = self.swarm.behaviour_mut().kademlia.get_providers(RecordKey::new(&key));
        self.shared.record_dht_query_started();
        self.pending_queries.insert(
            query,
            PendingQuery::FindProviders {
                key,
                respond,
                providers: Vec::new(),
                deadline: self.dht_deadline(),
            },
        );
    }

    fn dht_deadline(&self) -> Instant {
        Instant::now() + self.config.dht_query_timeout + REQUEST_GRACE
    }

    fn bootstrap_add(&mut self, address: Multiaddr, respond: oneshot::Sender<Result<(), NetError>>) {
        let mut addresses: Vec<Multiaddr> = self
            .shared
            .bootstrap()
            .iter()
            .filter_map(|candidate| candidate.parse().ok())
            .collect();
        if !addresses.iter().any(|candidate| candidate == &address) {
            addresses.push(address.clone());
        }
        if addresses.len() > crate::config::MAX_BOOTSTRAP_PEERS {
            let _ = respond.send(Err(NetError::InvalidConfig("bootstrap")));
            return;
        }
        match bootstrap::save(&self.config.data_root, &addresses) {
            Ok(()) => {
                self.shared
                    .set_bootstrap(addresses.iter().map(ToString::to_string).collect());
                if let Ok((peer_id, address)) = split_peer_address(&address) {
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, address.clone());
                    self.targets.insert(peer_id, vec![address]);
                }
                let _ = respond.send(Ok(()));
            }
            Err(error) => {
                let _ = respond.send(Err(error));
            }
        }
    }

    fn bootstrap_remove(&mut self, address: &Multiaddr, respond: oneshot::Sender<Result<(), NetError>>) {
        let text = address.to_string();
        let current: Vec<Multiaddr> = self
            .shared
            .bootstrap()
            .iter()
            .filter_map(|candidate| candidate.parse().ok())
            .collect();
        if !current.iter().any(|candidate| candidate.to_string() == text) {
            let _ = respond.send(Err(NetError::PeerNotFound));
            return;
        }
        let addresses: Vec<Multiaddr> = current
            .into_iter()
            .filter(|candidate| candidate.to_string() != text)
            .collect();
        match bootstrap::save(&self.config.data_root, &addresses) {
            Ok(()) => {
                self.shared
                    .set_bootstrap(addresses.iter().map(ToString::to_string).collect());
                if let Ok((peer_id, plain_address)) = split_peer_address(address) {
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .remove_address(&peer_id, &plain_address);
                    self.targets.remove(&peer_id);
                    self.backoff.remove(&peer_id);
                }
                let _ = respond.send(Ok(()));
            }
            Err(error) => {
                let _ = respond.send(Err(error));
            }
        }
    }

    fn relay_add(&mut self, address: Multiaddr, respond: oneshot::Sender<Result<(), NetError>>) {
        let mut addresses: Vec<Multiaddr> = self
            .shared
            .relays()
            .iter()
            .filter_map(|candidate| candidate.parse().ok())
            .collect();
        if !addresses.iter().any(|candidate| candidate == &address) {
            addresses.push(address.clone());
        }
        if addresses.len() > crate::config::MAX_RELAYS {
            let _ = respond.send(Err(NetError::InvalidConfig("relays")));
            return;
        }
        match relay_list::save(&self.config.data_root, &addresses) {
            Ok(()) => {
                self.shared
                    .set_relays(addresses.iter().map(ToString::to_string).collect());
                if let Ok((peer_id, full_address)) = split_peer_address(&address) {
                    self.relays.insert(peer_id, full_address.clone());
                    let targets = self.targets.entry(peer_id).or_default();
                    if !targets.contains(&full_address) {
                        targets.push(full_address.clone());
                    }
                    self.swarm.add_peer_address(peer_id, full_address.clone());
                    self.dial_background(peer_id, self.targets.get(&peer_id).cloned().unwrap_or_default());
                }
                self.refresh_relay_counts();
                let _ = respond.send(Ok(()));
            }
            Err(error) => {
                let _ = respond.send(Err(error));
            }
        }
    }

    fn relay_remove(&mut self, address: &Multiaddr, respond: oneshot::Sender<Result<(), NetError>>) {
        let text = address.to_string();
        let current: Vec<Multiaddr> = self
            .shared
            .relays()
            .iter()
            .filter_map(|candidate| candidate.parse().ok())
            .collect();
        if !current.iter().any(|candidate| candidate.to_string() == text) {
            let _ = respond.send(Err(NetError::PeerNotFound));
            return;
        }
        let addresses: Vec<Multiaddr> = current
            .into_iter()
            .filter(|candidate| candidate.to_string() != text)
            .collect();
        match relay_list::save(&self.config.data_root, &addresses) {
            Ok(()) => {
                self.shared
                    .set_relays(addresses.iter().map(ToString::to_string).collect());
                if let Ok((peer_id, full_address)) = split_peer_address(address) {
                    self.relays.remove(&peer_id);
                    let empty = if let Some(targets) = self.targets.get_mut(&peer_id) {
                        targets.retain(|candidate| candidate != &full_address);
                        targets.is_empty()
                    } else {
                        false
                    };
                    if empty {
                        self.targets.remove(&peer_id);
                        self.backoff.remove(&peer_id);
                        let _ = self.swarm.disconnect_peer_id(peer_id);
                    }
                    if let Some(listener) = self
                        .relay_state
                        .remove(&peer_id)
                        .and_then(|mut state| state.listener.take())
                    {
                        let _ = self.swarm.remove_listener(listener);
                    }
                }
                self.refresh_relay_counts();
                let _ = respond.send(Ok(()));
            }
            Err(error) => {
                let _ = respond.send(Err(error));
            }
        }
    }

    fn reconnect(&mut self, peer_id: Option<PeerId>) -> Result<(), NetError> {
        match peer_id {
            Some(peer_id) => {
                if !self.targets.contains_key(&peer_id) {
                    return Err(NetError::PeerNotFound);
                }
                let _ = self.swarm.disconnect_peer_id(peer_id);
                self.backoff.insert(
                    peer_id,
                    Backoff {
                        attempts: 0,
                        next_at: Instant::now(),
                    },
                );
                Ok(())
            }
            None => {
                let now = Instant::now();
                for peer_id in self.targets.keys() {
                    let _ = self.swarm.disconnect_peer_id(*peer_id);
                    self.backoff.insert(
                        *peer_id,
                        Backoff {
                            attempts: 0,
                            next_at: now,
                        },
                    );
                }
                Ok(())
            }
        }
    }

    fn network_changed(&mut self) {
        let now = Instant::now();
        for peer_id in self.targets.keys() {
            self.backoff.insert(
                *peer_id,
                Backoff {
                    attempts: 0,
                    next_at: now,
                },
            );
        }
    }

    fn on_tick(&mut self) {
        let now = Instant::now();

        let expired: Vec<PeerId> = self
            .pending_dials
            .iter()
            .filter(|(_, pending)| pending.deadline <= now)
            .map(|(peer_id, _)| *peer_id)
            .collect();
        for peer_id in expired {
            let Some(pending) = self.pending_dials.get(&peer_id) else {
                continue;
            };
            match pending.phase {
                DialPhase::Direct => self.start_relay_phase(peer_id, "timeout"),
                DialPhase::Relay => {
                    if !self.dial_next_relay(peer_id) {
                        self.fail_no_viable_path(peer_id);
                    }
                }
            }
        }

        self.expire_relay_reservations(now);
        self.retry_relay_reservations(now);

        let expired: Vec<OutboundRequestId> = self
            .pending_requests
            .iter()
            .filter(|(_, pending)| pending.deadline <= now)
            .map(|(request_id, _)| *request_id)
            .collect();
        for request_id in expired {
            if let Some(pending) = self.pending_requests.remove(&request_id) {
                let _ = pending.respond.send(Err(NetError::Timeout));
            }
        }

        let expired: Vec<OutboundRequestId> = self
            .pending_file_requests
            .iter()
            .filter(|(_, pending)| pending.deadline() <= now)
            .map(|(request_id, _)| *request_id)
            .collect();
        for request_id in expired {
            match self.pending_file_requests.remove(&request_id) {
                Some(PendingFileRequest::Metadata { respond, .. }) => {
                    let _ = respond.send(Err(NetError::Timeout));
                }
                Some(PendingFileRequest::Chunk { respond, .. }) => {
                    let _ = respond.send(Err(NetError::Timeout));
                }
                None => {}
            }
        }

        let expired: Vec<kad::QueryId> = self
            .pending_queries
            .iter()
            .filter(|(_, pending)| pending.deadline() <= now)
            .map(|(query, _)| *query)
            .collect();
        for query in expired {
            if let Some(pending) = self.pending_queries.remove(&query) {
                self.fail_pending(pending, "dht_timeout", NetError::Timeout);
            }
        }

        self.refresh_routing_peers();
        self.maybe_bootstrap();

        if self.rate.len() > MAX_RATE_WINDOWS {
            self.rate
                .retain(|_, window| window.started.elapsed() < Duration::from_secs(60));
        }

        let due: Vec<PeerId> = self
            .backoff
            .iter()
            .filter(|(peer_id, backoff)| {
                backoff.next_at <= now
                    && !self.pending_dials.contains_key(peer_id)
                    && !self.background_relay_attempted.contains(peer_id)
                    && !self.swarm.is_connected(peer_id)
            })
            .map(|(peer_id, _)| *peer_id)
            .collect();
        for peer_id in due {
            let addresses = self.targets.get(&peer_id).cloned().unwrap_or_default();
            self.dial_background(peer_id, addresses);
        }

        self.shared.refresh();
    }

    fn refresh_routing_peers(&mut self) {
        let count: usize = self
            .swarm
            .behaviour_mut()
            .kademlia
            .kbuckets()
            .map(|bucket| bucket.iter().count())
            .sum();
        if count != self.routing_peers {
            self.routing_peers = count;
            self.shared.set_dht_routing_peers(count);
        }
    }

    fn routing_addresses(&mut self, peer: &PeerId) -> Vec<Multiaddr> {
        let mut addresses = Vec::new();
        for bucket in self.swarm.behaviour_mut().kademlia.kbuckets() {
            if let Some(entry) = bucket.iter().find(|entry| entry.node.key.preimage() == peer) {
                addresses = entry.node.value.iter().cloned().collect();
                break;
            }
        }
        addresses
    }

    fn maybe_bootstrap(&mut self) {
        if self.dht_bootstrapped || self.routing_peers == 0 {
            return;
        }
        if self.last_bootstrap.is_some_and(|last| last.elapsed() < BOOTSTRAP_RETRY) {
            return;
        }
        self.last_bootstrap = Some(Instant::now());
        // An empty routing table is a normal state for an offline node. The
        // periodic and on-low-peers bootstrap in `kad` retries once a peer is
        // known, and the next tick re-evaluates this throttle.
        let _ = self.swarm.behaviour_mut().kademlia.bootstrap();
    }

    fn on_kademlia_event(&mut self, event: kad::Event) {
        match event {
            kad::Event::InboundRequest { request } => match request {
                kad::InboundRequest::PutRecord {
                    record: Some(record), ..
                } => self.store_inbound_record(record),
                kad::InboundRequest::AddProvider { record: Some(record) } => self.store_inbound_provider(record),
                _ => {}
            },
            kad::Event::OutboundQueryProgressed { id, result, step, .. } => self.on_query_progressed(id, result, step),
            kad::Event::RoutingUpdated { .. } => self.refresh_routing_peers(),
            kad::Event::ModeChanged { new_mode } => {
                let mode = match new_mode {
                    kad::Mode::Client => "client",
                    kad::Mode::Server => "server",
                };
                self.shared.set_dht_mode(mode);
            }
            _ => {}
        }
    }

    /// Validates an inbound DHT value before it reaches the bounded store.
    ///
    /// Values must be exact signed envelopes whose derived key matches the
    /// record key. Revisions are monotonic per key, so stale or conflicting
    /// copies cannot roll back a newer signed record.
    fn store_inbound_record(&mut self, record: Record) {
        if record.key.as_ref().len() != DHT_KEY_BYTES || record.value.len() > DHT_MAX_VALUE_BYTES {
            self.shared.record_dht_record(false);
            return;
        }
        let parsed = match DhtRecord::verify_for_key(record.key.as_ref(), &record.value) {
            Ok(parsed) => parsed,
            Err(_) => {
                self.shared.record_dht_record(false);
                return;
            }
        };
        let existing = self
            .swarm
            .behaviour_mut()
            .kademlia
            .store_mut()
            .get(&record.key)
            .and_then(|existing| DhtRecord::parse(&existing.value).ok());
        match DhtRecord::classify_update(existing.as_ref(), &parsed) {
            DhtRecordDecision::Store => {
                let stored = self.swarm.behaviour_mut().kademlia.store_mut().put(record).is_ok();
                self.shared.record_dht_record(stored);
            }
            DhtRecordDecision::AlreadyStored => self.shared.record_dht_record(true),
            DhtRecordDecision::Stale | DhtRecordDecision::Conflict => {
                self.shared.record_dht_record(false);
            }
        }
    }

    /// Validates an inbound provider announcement before storing it.
    fn store_inbound_provider(&mut self, mut record: ProviderRecord) {
        if record.addresses.is_empty() {
            record.addresses = self.routing_addresses(&record.provider);
        }
        let key = record.key.as_ref();
        let lengths: Vec<usize> = record.addresses.iter().map(Multiaddr::len).collect();
        if validate_provider_key(key).is_err() || validate_provider_addresses(&lengths).is_err() {
            self.shared.record_dht_provider(false);
            return;
        }
        let stored = self
            .swarm
            .behaviour_mut()
            .kademlia
            .store_mut()
            .add_provider(record)
            .is_ok();
        self.shared.record_dht_provider(stored);
    }

    fn on_query_progressed(&mut self, id: kad::QueryId, result: kad::QueryResult, step: ProgressStep) {
        match result {
            kad::QueryResult::Bootstrap(result) => match result {
                Ok(_) => {
                    self.dht_bootstrapped = true;
                    self.last_bootstrap = None;
                    self.shared.mark_dht_bootstrapped();
                }
                Err(_) => self.shared.record_last_error("dht_bootstrap_failed"),
            },
            kad::QueryResult::GetClosestPeers(result) => {
                let Some(PendingQuery::Discover { respond, started, .. }) = self.pending_queries.remove(&id) else {
                    return;
                };
                match result {
                    Ok(ok) => {
                        let mut peers: Vec<DiscoveredPeer> = ok
                            .peers
                            .into_iter()
                            .map(|info| DiscoveredPeer {
                                peer_id: info.peer_id.to_string(),
                                addresses: info
                                    .addrs
                                    .iter()
                                    .take(MAX_DISCOVERED_ADDRESSES)
                                    .map(ToString::to_string)
                                    .collect(),
                            })
                            .collect();
                        peers.sort_by(|left, right| left.peer_id.cmp(&right.peer_id));
                        peers.dedup_by(|left, right| left.peer_id == right.peer_id);
                        peers.truncate(MAX_DISCOVERED_PEERS);
                        let discovery = PeerDiscovery {
                            peers,
                            routing_peers: self.routing_peers,
                            elapsed_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                        };
                        let _ = respond.send(Ok(discovery));
                    }
                    Err(_) => {
                        self.shared.record_dht_query_failed("dht_discovery_failed");
                        let _ = respond.send(Err(NetError::Timeout));
                    }
                }
            }
            kad::QueryResult::GetRecord(result) => {
                let key = match self.pending_queries.get(&id) {
                    Some(PendingQuery::Lookup { key, .. }) => *key,
                    _ => return,
                };
                match result {
                    Ok(kad::GetRecordOk::FoundRecord(peer_record)) => {
                        if DhtRecord::verify_for_key(&key, &peer_record.record.value).is_err() {
                            self.shared.record_dht_record(false);
                            return;
                        }
                        let Some(PendingQuery::Lookup { respond, .. }) = self.pending_queries.remove(&id) else {
                            return;
                        };
                        let value = DhtValue {
                            key: key.to_vec(),
                            value: peer_record.record.value,
                            peer_id: peer_record.peer.map(|peer| peer.to_string()),
                        };
                        let _ = respond.send(Ok(Some(value)));
                    }
                    Ok(kad::GetRecordOk::FinishedWithNoAdditionalRecord { .. }) => {
                        if let Some(PendingQuery::Lookup { respond, .. }) = self.pending_queries.remove(&id) {
                            let _ = respond.send(Ok(None));
                        }
                    }
                    Err(kad::GetRecordError::NotFound { .. }) => {
                        if let Some(PendingQuery::Lookup { respond, .. }) = self.pending_queries.remove(&id) {
                            let _ = respond.send(Ok(None));
                        }
                    }
                    Err(kad::GetRecordError::Timeout { .. }) => {
                        if let Some(pending) = self.pending_queries.remove(&id) {
                            self.fail_pending(pending, "dht_lookup_timeout", NetError::Timeout);
                        }
                    }
                }
            }
            kad::QueryResult::PutRecord(result) => {
                if !step.last {
                    return;
                }
                let Some(pending) = self.pending_queries.remove(&id) else {
                    return;
                };
                match result {
                    Ok(_) => {
                        self.shared.record_dht_publish(true);
                        if let PendingQuery::Publish { respond, .. } = pending {
                            let _ = respond.send(Ok(()));
                        }
                    }
                    Err(_) => {
                        // `fail_pending` records both the query failure and the
                        // publish failure; counting the publish here again would
                        // double the `dht_publish_failures` status field.
                        self.fail_pending(pending, "dht_publish_failed", NetError::Rejected);
                    }
                }
            }
            kad::QueryResult::StartProviding(result) => {
                let Some(pending) = self.pending_queries.remove(&id) else {
                    return;
                };
                match result {
                    Ok(_) => {
                        if let PendingQuery::StartProviding { respond, .. } = pending {
                            let _ = respond.send(Ok(()));
                        }
                    }
                    Err(_) => {
                        self.fail_pending(pending, "dht_provider_failed", NetError::Rejected);
                    }
                }
            }
            kad::QueryResult::GetProviders(result) => match result {
                Ok(kad::GetProvidersOk::FoundProviders { providers, .. }) => {
                    let Some(PendingQuery::FindProviders { providers: found, .. }) = self.pending_queries.get_mut(&id)
                    else {
                        return;
                    };
                    for provider in providers {
                        if found.len() >= MAX_DISCOVERED_PEERS {
                            break;
                        }
                        if !found.contains(&provider) {
                            found.push(provider);
                        }
                    }
                }
                Ok(kad::GetProvidersOk::FinishedWithNoAdditionalRecord { .. }) => {
                    let Some(PendingQuery::FindProviders {
                        key,
                        respond,
                        mut providers,
                        ..
                    }) = self.pending_queries.remove(&id)
                    else {
                        return;
                    };
                    let records = self
                        .swarm
                        .behaviour_mut()
                        .kademlia
                        .store_mut()
                        .providers(&RecordKey::new(&key));
                    let mut peers: Vec<DiscoveredPeer> = Vec::new();
                    for record in records {
                        providers.retain(|provider| provider != &record.provider);
                        let mut addresses: Vec<String> = record
                            .addresses
                            .iter()
                            .take(MAX_DISCOVERED_ADDRESSES)
                            .map(ToString::to_string)
                            .collect();
                        if addresses.is_empty() {
                            let mut fallback = self.routing_addresses(&record.provider);
                            fallback.truncate(MAX_DISCOVERED_ADDRESSES);
                            addresses = fallback.into_iter().map(|address| address.to_string()).collect();
                        }
                        addresses.sort();
                        addresses.dedup();
                        peers.push(DiscoveredPeer {
                            peer_id: record.provider.to_string(),
                            addresses,
                        });
                    }
                    for provider in providers {
                        let mut addresses: Vec<String> = self
                            .routing_addresses(&provider)
                            .into_iter()
                            .take(MAX_DISCOVERED_ADDRESSES)
                            .map(|address| address.to_string())
                            .collect();
                        addresses.sort();
                        addresses.dedup();
                        peers.push(DiscoveredPeer {
                            peer_id: provider.to_string(),
                            addresses,
                        });
                    }
                    peers.sort_by(|left, right| left.peer_id.cmp(&right.peer_id));
                    peers.dedup_by(|left, right| left.peer_id == right.peer_id);
                    peers.truncate(MAX_DISCOVERED_PEERS);
                    let _ = respond.send(Ok(peers));
                }
                Err(_) => {
                    if let Some(pending) = self.pending_queries.remove(&id) {
                        self.fail_pending(pending, "dht_provider_lookup_failed", NetError::Timeout);
                    }
                }
            },
            kad::QueryResult::RepublishRecord(result) => {
                self.shared.record_dht_publish(result.is_ok());
            }
            kad::QueryResult::RepublishProvider(result) => {
                if result.is_err() {
                    self.shared.record_last_error("dht_republish_provider_failed");
                }
            }
        }
    }

    fn fail_pending(&mut self, pending: PendingQuery, code: &'static str, error: NetError) {
        match pending {
            PendingQuery::Discover { respond, .. } => {
                self.shared.record_dht_query_failed(code);
                let _ = respond.send(Err(error));
            }
            PendingQuery::Publish { respond, .. } => {
                self.shared.record_dht_query_failed(code);
                self.shared.record_dht_publish(false);
                let _ = respond.send(Err(error));
            }
            PendingQuery::Lookup { respond, .. } => {
                self.shared.record_dht_query_failed(code);
                let _ = respond.send(Err(error));
            }
            PendingQuery::StartProviding { respond, .. } => {
                self.shared.record_dht_query_failed(code);
                let _ = respond.send(Err(error));
            }
            PendingQuery::FindProviders { respond, .. } => {
                self.shared.record_dht_query_failed(code);
                let _ = respond.send(Err(error));
            }
        }
    }

    fn schedule_retry(&mut self, peer_id: &PeerId) {
        if !self.targets.contains_key(peer_id) {
            return;
        }
        let attempts = self
            .backoff
            .get(peer_id)
            .map_or(1, |backoff| backoff.attempts.saturating_add(1));
        self.backoff.insert(
            *peer_id,
            Backoff {
                attempts,
                next_at: Instant::now() + self.config.reconnect_delay(attempts),
            },
        );
    }

    fn on_swarm_event(&mut self, event: SwarmEvent<BehaviourEvent>) {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                if let Some(relay_peer_id) = Self::relay_for_circuit(&address) {
                    if let Some(state) = self.relay_state.get_mut(&relay_peer_id) {
                        state.reservation = true;
                        state.pending_since = None;
                        state.retry_at = None;
                    }
                    self.refresh_relay_counts();
                } else if self.config.relay_server_enabled && is_loopback(&address) {
                    // Local relay nodes have no public address. Advertising the
                    // loopback listener keeps reservations working in tests and
                    // on a single machine without pretending to be public.
                    self.swarm.add_external_address(address.clone());
                }
                self.shared.add_listen(address);
            }
            SwarmEvent::ExpiredListenAddr { address, .. } => {
                if let Some(relay_peer_id) = Self::relay_for_circuit(&address) {
                    if let Some(state) = self.relay_state.get_mut(&relay_peer_id) {
                        state.reservation = false;
                    }
                    self.refresh_relay_counts();
                }
                self.shared.remove_listen(&address);
            }
            SwarmEvent::ListenerClosed {
                listener_id,
                addresses,
                reason,
            } => {
                let closed_relay = self
                    .relay_state
                    .iter()
                    .find(|(_, state)| state.listener == Some(listener_id))
                    .map(|(relay, _)| *relay);
                if let Some(relay_peer_id) = closed_relay {
                    if let Some(state) = self.relay_state.get_mut(&relay_peer_id) {
                        state.listener = None;
                        state.reservation = false;
                        state.pending_since = None;
                        if state.retry_at.is_none() {
                            state.retry_at = Some(Instant::now() + RELAY_RETRY);
                        }
                    }
                    self.shared.record_relay_reservation(false);
                    self.shared.record_last_error("relay_reservation_closed");
                    self.refresh_relay_counts();
                }
                for address in addresses {
                    self.shared.remove_listen(&address);
                }
                if reason.is_err() {
                    self.shared.record_last_error("listener_closed");
                }
            }
            SwarmEvent::ListenerError { listener_id, .. } => {
                if self
                    .relay_state
                    .values()
                    .any(|state| state.listener == Some(listener_id))
                {
                    self.shared.record_relay_reservation(false);
                    self.shared.record_last_error("relay_listener_error");
                } else {
                    self.shared.record_last_error("listener_error");
                }
            }
            SwarmEvent::ConnectionEstablished {
                peer_id,
                endpoint,
                num_established,
                established_in,
                ..
            } => {
                let direction = if endpoint.is_dialer() { "outbound" } else { "inbound" };
                let relayed = endpoint.is_relayed();
                let path = if relayed { "relay" } else { "direct" };
                let endpoint_address = match &endpoint {
                    ConnectedPoint::Dialer { address, .. } => address.clone(),
                    ConnectedPoint::Listener { local_addr, .. } => local_addr.clone(),
                };
                let relay_peer_id = relayed.then(|| Self::relay_for_circuit(&endpoint_address)).flatten();
                let relay_text = relay_peer_id.map(|relay| relay.to_string());
                let transport = transport_label(&endpoint_address);
                let connected = self.shared.record_connected(
                    &peer_id.to_string(),
                    direction,
                    transport,
                    &endpoint_address,
                    established_in,
                    path,
                    relay_text.as_deref(),
                );
                if !relayed {
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, endpoint_address);
                }
                if num_established.get() == 1 {
                    self.backoff.remove(&peer_id);
                }
                if self.relays.contains_key(&peer_id) && !relayed {
                    self.ensure_relay_reservation(peer_id);
                }
                self.background_relay_attempted.remove(&peer_id);
                if let Some(pending) = self.pending_dials.remove(&peer_id) {
                    if pending.phase == DialPhase::Relay {
                        self.shared.record_fallback_success();
                    }
                    let _ = pending.respond.send(Ok(connected));
                }
                self.refresh_relay_counts();
            }
            SwarmEvent::ConnectionClosed {
                peer_id,
                endpoint,
                num_established,
                cause,
                ..
            } => {
                let path = if endpoint.is_relayed() { "relay" } else { "direct" };
                if num_established == 0 {
                    let reason = if cause.is_some() {
                        "connection_closed"
                    } else {
                        "peer_closed"
                    };
                    self.shared
                        .record_disconnected(&peer_id.to_string(), Some(reason), path);
                    if self.relays.contains_key(&peer_id) {
                        if let Some(listener) = self
                            .relay_state
                            .remove(&peer_id)
                            .and_then(|mut state| state.listener.take())
                        {
                            let _ = self.swarm.remove_listener(listener);
                        }
                        self.refresh_relay_counts();
                    }
                    self.background_relay_attempted.remove(&peer_id);
                    if self.targets.contains_key(&peer_id) {
                        self.schedule_retry(&peer_id);
                    }
                } else {
                    self.shared.record_connection_removed(&peer_id.to_string(), path);
                }
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                let (code, _) = map_dial_error(&error);
                self.shared.record_last_error(code);
                if let Some(peer_id) = peer_id {
                    self.handle_outgoing_error(peer_id, &error);
                }
            }
            SwarmEvent::IncomingConnectionError { .. } => {
                self.shared.record_last_error("incoming_connection_failed");
            }
            SwarmEvent::NewExternalAddrCandidate { address, .. } => self.shared.add_observed(&address),
            SwarmEvent::ExternalAddrConfirmed { address, .. } => self.shared.add_confirmed(&address),
            SwarmEvent::ExternalAddrExpired { address, .. } => self.shared.remove_external(&address),
            SwarmEvent::Behaviour(event) => self.on_behaviour_event(event),
            _ => {}
        }
    }

    fn on_behaviour_event(&mut self, event: BehaviourEvent) {
        match event {
            BehaviourEvent::Identify(identify::Event::Received { peer_id, info, .. }) => {
                let protocols: Vec<String> = info.protocols.iter().take(16).map(ToString::to_string).collect();
                let agent_version = Some(info.agent_version.chars().take(64).collect::<String>());
                self.shared
                    .record_identity(&peer_id.to_string(), protocols, agent_version);
                self.shared.add_observed(&info.observed_addr);
                for address in info.listen_addrs.iter().take(8) {
                    self.swarm.add_peer_address(peer_id, address.clone());
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, address.clone());
                }
            }
            BehaviourEvent::Ping(ping::Event {
                peer, result: Ok(rtt), ..
            }) => self.shared.record_rtt(&peer.to_string(), rtt),
            BehaviourEvent::Kademlia(event) => self.on_kademlia_event(event),
            BehaviourEvent::Test(request_response::Event::Message { peer, message, .. }) => match message {
                request_response::Message::Request { request, channel, .. } => {
                    self.on_inbound_request(peer, request, channel);
                }
                request_response::Message::Response { request_id, response } => {
                    self.on_inbound_response(request_id, response);
                }
            },
            BehaviourEvent::Test(request_response::Event::OutboundFailure {
                peer,
                request_id,
                error,
                ..
            }) => {
                let (code, mapped) = map_outbound_failure(error);
                self.shared.record_last_error(code);
                if let Some(pending) = self.pending_requests.remove(&request_id) {
                    let _ = pending.respond.send(Err(mapped));
                }
                let _ = peer;
            }
            BehaviourEvent::Test(request_response::Event::InboundFailure { error, .. }) => {
                self.shared.record_request(false);
                self.shared.record_last_error(map_inbound_failure(&error));
            }
            BehaviourEvent::File(request_response::Event::Message { peer, message, .. }) => match message {
                request_response::Message::Request { request, channel, .. } => {
                    self.on_inbound_file_request(peer, request, channel);
                }
                request_response::Message::Response { request_id, response } => {
                    self.on_file_response(request_id, response);
                }
            },
            BehaviourEvent::File(request_response::Event::OutboundFailure { request_id, error, .. }) => {
                let (code, mapped) = map_outbound_failure(error);
                self.shared.record_last_error(code);
                match self.pending_file_requests.remove(&request_id) {
                    Some(PendingFileRequest::Metadata { respond, .. }) => {
                        let _ = respond.send(Err(mapped));
                    }
                    Some(PendingFileRequest::Chunk { respond, .. }) => {
                        let _ = respond.send(Err(mapped));
                    }
                    None => {}
                }
            }
            BehaviourEvent::File(request_response::Event::InboundFailure { error, .. }) => {
                self.shared.record_last_error(map_inbound_failure(&error));
            }
            BehaviourEvent::Autonat(event) => self.on_autonat_event(event),
            BehaviourEvent::Dcutr(event) => match event.result {
                Ok(_) => self.shared.record_direct_upgrade(&event.remote_peer_id.to_string()),
                Err(_) => {
                    self.shared.record_hole_punch_failure();
                    self.shared.record_last_error("hole_punch_failed");
                }
            },
            BehaviourEvent::RelayClient(event) => self.on_relay_client_event(event),
            BehaviourEvent::RelayServer(event) => self.on_relay_server_event(event),
            _ => {}
        }
    }

    fn on_autonat_event(&mut self, event: autonat::v1::Event) {
        match event {
            autonat::v1::Event::StatusChanged { new, .. } => match new {
                autonat::v1::NatStatus::Public(address) => {
                    self.shared.set_nat_status("public", Some(&address));
                }
                autonat::v1::NatStatus::Private => self.shared.set_nat_status("private", None),
                autonat::v1::NatStatus::Unknown => self.shared.set_nat_status("unknown", None),
            },
            autonat::v1::Event::OutboundProbe(_) => self.shared.record_nat_probe(),
            autonat::v1::Event::InboundProbe(_) => {}
        }
    }

    fn on_relay_client_event(&mut self, event: relay::client::Event) {
        match event {
            relay::client::Event::ReservationReqAccepted {
                relay_peer_id,
                renewal,
                limit,
            } => {
                if let Some(limit) = limit {
                    self.apply_relay_limit(relay_peer_id, limit.duration(), limit.data_in_bytes());
                }
                if let Some(state) = self.relay_state.get_mut(&relay_peer_id) {
                    state.reservation = true;
                    state.pending_since = None;
                    state.retry_at = None;
                } else {
                    self.relay_state.insert(
                        relay_peer_id,
                        RelayState {
                            reservation: true,
                            ..RelayState::default()
                        },
                    );
                }
                if !renewal {
                    self.shared.record_relay_reservation(true);
                }
                self.refresh_relay_counts();
            }
            relay::client::Event::OutboundCircuitEstablished { relay_peer_id, limit } => {
                if let Some(limit) = limit {
                    self.apply_relay_limit(relay_peer_id, limit.duration(), limit.data_in_bytes());
                }
                self.shared.record_relay_circuit();
            }
            relay::client::Event::InboundCircuitEstablished { src_peer_id, limit } => {
                let _ = src_peer_id;
                if let Some(limit) = limit {
                    self.shared.set_relay_limits(limit.duration(), limit.data_in_bytes());
                }
                self.shared.record_relay_circuit();
            }
        }
    }

    fn on_relay_server_event(&mut self, event: relay::Event) {
        match event {
            relay::Event::ReservationReqAccepted { renewed, .. } => {
                if !renewed {
                    self.shared.record_relay_server_reservation(true);
                }
            }
            relay::Event::ReservationReqDenied { .. } => self.shared.record_relay_server_denied(),
            relay::Event::ReservationClosed { .. } | relay::Event::ReservationTimedOut { .. } => {
                self.shared.record_relay_server_reservation(false);
            }
            relay::Event::CircuitReqAccepted { .. } => {
                self.shared.record_relay_server_circuit(true);
            }
            relay::Event::CircuitReqDenied { .. } => self.shared.record_relay_server_denied(),
            relay::Event::CircuitClosed { .. } => self.shared.record_relay_server_circuit(false),
            relay::Event::StatusChanged { status } => {
                self.shared.set_relay_server_status(status == relay::Status::Enable);
            }
            _ => {}
        }
    }

    fn apply_relay_limit(&mut self, relay_peer_id: PeerId, duration: Option<Duration>, bytes: Option<u64>) {
        let applied = RelayLimit { duration, bytes };
        if let Some(state) = self.relay_state.get_mut(&relay_peer_id) {
            state.limit = applied;
        }
        self.shared.set_relay_limits(applied.duration, applied.bytes);
    }

    fn on_inbound_request(&mut self, peer: PeerId, request: Vec<u8>, channel: ResponseChannel<Vec<u8>>) {
        if !self.allow_request(peer) {
            self.shared.record_request(false);
            let response = TestResponse {
                payload: None,
                error: Some("rate_limited".to_owned()),
            };
            if let Ok(body) = encode_test_response(&response) {
                let _ = self.swarm.behaviour_mut().test.send_response(channel, body);
            }
            return;
        }
        let response = match decode_test_request(&request) {
            Ok(request) => {
                self.shared.record_request(true);
                TestResponse {
                    payload: Some(request.payload),
                    error: None,
                }
            }
            Err(_) => {
                self.shared.record_request(false);
                TestResponse {
                    payload: None,
                    error: Some("malformed".to_owned()),
                }
            }
        };
        if let Ok(body) = encode_test_response(&response) {
            let _ = self.swarm.behaviour_mut().test.send_response(channel, body);
        }
    }

    fn on_inbound_file_request(&mut self, peer: PeerId, request: Vec<u8>, channel: ResponseChannel<Vec<u8>>) {
        if !self.allow_file_request(peer) {
            let response = FileResponse {
                error: Some("rate_limited".to_owned()),
                ..FileResponse::default()
            };
            if let Ok(body) = encode_file_response(&response) {
                let _ = self.swarm.behaviour_mut().file.send_response(channel, body);
            }
            return;
        }
        let response = match decode_file_request(&request) {
            Ok(FileRequest::Metadata(selector)) => match self.file_provider.metadata(selector) {
                Some(metadata) => FileResponse {
                    manifest: Some(metadata.manifest),
                    publication: metadata.publication,
                    ..FileResponse::default()
                },
                None => unavailable_response(),
            },
            Ok(FileRequest::Chunk(chunk)) => {
                match self
                    .file_provider
                    .chunk(&chunk.content_id, chunk.index, chunk.offset, chunk.length)
                {
                    Some(data) if data.len() == usize::try_from(chunk.length).unwrap_or(usize::MAX) => FileResponse {
                        data: Some(data),
                        ..FileResponse::default()
                    },
                    _ => unavailable_response(),
                }
            }
            Err(_) => FileResponse {
                error: Some("malformed".to_owned()),
                ..FileResponse::default()
            },
        };
        if let Ok(body) = encode_file_response(&response) {
            let _ = self.swarm.behaviour_mut().file.send_response(channel, body);
        }
    }

    fn on_file_response(&mut self, request_id: OutboundRequestId, response: Vec<u8>) {
        let Some(pending) = self.pending_file_requests.remove(&request_id) else {
            return;
        };
        let decoded = decode_file_response(&response);
        match pending {
            PendingFileRequest::Metadata { respond, .. } => {
                let result = match decoded {
                    Ok(response) => match (response.manifest, response.error) {
                        (Some(manifest), None) => Ok(FileMetadata {
                            manifest,
                            publication: response.publication,
                        }),
                        (None, Some(error)) => Err(map_file_error(&error)),
                        _ => Err(NetError::ConnectionFailed),
                    },
                    Err(_) => Err(NetError::ConnectionFailed),
                };
                let _ = respond.send(result);
            }
            PendingFileRequest::Chunk { respond, .. } => {
                let result = match decoded {
                    Ok(response) => match (response.data, response.error) {
                        (Some(data), None) => Ok(data),
                        (None, Some(error)) => Err(map_file_error(&error)),
                        _ => Err(NetError::ConnectionFailed),
                    },
                    Err(_) => Err(NetError::ConnectionFailed),
                };
                let _ = respond.send(result);
            }
        }
    }

    fn on_inbound_response(&mut self, request_id: OutboundRequestId, response: Vec<u8>) {
        let Some(pending) = self.pending_requests.remove(&request_id) else {
            return;
        };
        let result = match decode_test_response(&response) {
            Ok(response) => match response.payload {
                Some(payload) => Ok(TestReply {
                    peer_id: pending.peer_id.to_string(),
                    payload,
                    round_trip_ms: u64::try_from(pending.started.elapsed().as_millis()).unwrap_or(u64::MAX),
                }),
                None => Err(NetError::Rejected),
            },
            Err(_) => Err(NetError::ConnectionFailed),
        };
        let _ = pending.respond.send(result);
    }

    fn allow_request(&mut self, peer: PeerId) -> bool {
        let limit = self.config.max_requests_per_peer_per_minute;
        Self::allow_window(&mut self.rate, peer, limit)
    }

    fn allow_file_request(&mut self, peer: PeerId) -> bool {
        if self.file_rate.len() >= MAX_RATE_WINDOWS && !self.file_rate.contains_key(&peer) {
            self.file_rate.clear();
        }
        Self::allow_window(&mut self.file_rate, peer, MAX_FILE_REQUESTS_PER_PEER_PER_MINUTE)
    }

    fn allow_window(windows: &mut HashMap<PeerId, RateWindow>, peer: PeerId, limit: u32) -> bool {
        let window = windows.entry(peer).or_insert(RateWindow {
            started: Instant::now(),
            count: 0,
        });
        if window.started.elapsed() >= Duration::from_secs(60) {
            window.started = Instant::now();
            window.count = 0;
        }
        if window.count >= limit {
            false
        } else {
            window.count = window.count.saturating_add(1);
            true
        }
    }
}

/// Transport label derived from the negotiated remote address.
pub(crate) fn transport_label(address: &Multiaddr) -> String {
    let text = address.to_string();
    if text.contains("/quic-v1") {
        "quic".to_owned()
    } else if text.contains("/tcp/") {
        "tcp".to_owned()
    } else {
        "unknown".to_owned()
    }
}

/// Whether an address names a loopback interface.
fn is_loopback(address: &Multiaddr) -> bool {
    address.iter().any(|protocol| match protocol {
        Protocol::Ip4(ip) => ip.is_loopback(),
        Protocol::Ip6(ip) => ip.is_loopback(),
        _ => false,
    })
}

fn map_dial_error(error: &DialError) -> (&'static str, NetError) {
    match error {
        DialError::WrongPeerId { .. } => ("wrong_peer_id", NetError::WrongPeerId),
        DialError::NoAddresses => ("no_addresses", NetError::InvalidAddress),
        DialError::LocalPeerId { .. } => ("local_peer_id", NetError::InvalidAddress),
        DialError::DialPeerConditionFalse(_) => ("dial_condition_false", NetError::ConnectionFailed),
        DialError::Aborted => ("dial_aborted", NetError::ConnectionFailed),
        DialError::Denied { .. } => ("dial_denied", NetError::Unavailable),
        DialError::Transport(_) => ("transport_failure", NetError::ConnectionFailed),
    }
}

fn error_for_dial_code(code: &'static str) -> NetError {
    match code {
        "wrong_peer_id" => NetError::WrongPeerId,
        "no_addresses" | "local_peer_id" => NetError::InvalidAddress,
        "timeout" => NetError::Timeout,
        "dial_denied" => NetError::Unavailable,
        _ => NetError::ConnectionFailed,
    }
}

fn map_outbound_failure(error: OutboundFailure) -> (&'static str, NetError) {
    match error {
        OutboundFailure::DialFailure => ("dial_failure", NetError::ConnectionFailed),
        OutboundFailure::ConnectionClosed => ("connection_closed", NetError::ConnectionFailed),
        OutboundFailure::Timeout => ("timeout", NetError::Timeout),
        OutboundFailure::UnsupportedProtocols => ("unsupported_protocol", NetError::UnsupportedProtocol),
        OutboundFailure::Io(_) => ("io_error", NetError::ConnectionFailed),
    }
}

fn unavailable_response() -> FileResponse {
    FileResponse {
        error: Some("content_unavailable".to_owned()),
        ..FileResponse::default()
    }
}

fn map_file_error(code: &str) -> NetError {
    match code {
        "content_unavailable" => NetError::FileUnavailable,
        "rate_limited" | "malformed" => NetError::Rejected,
        _ => NetError::Rejected,
    }
}

fn map_inbound_failure(error: &request_response::InboundFailure) -> &'static str {
    match error {
        request_response::InboundFailure::Timeout => "inbound_timeout",
        request_response::InboundFailure::ConnectionClosed => "inbound_connection_closed",
        request_response::InboundFailure::Io(_) => "inbound_io_error",
        request_response::InboundFailure::ResponseOmission => "inbound_response_omitted",
        _ => "inbound_failure",
    }
}

#[cfg(test)]
mod tests {
    use libp2p::Multiaddr;

    use super::transport_label;

    #[test]
    fn labels_quic_and_tcp_addresses() {
        let quic: Multiaddr = "/ip4/127.0.0.1/udp/4001/quic-v1".parse().expect("quic");
        assert_eq!(transport_label(&quic), "quic");
        let tcp: Multiaddr = "/ip4/127.0.0.1/tcp/4001".parse().expect("tcp");
        assert_eq!(transport_label(&tcp), "tcp");
        let other: Multiaddr = "/ip4/127.0.0.1/udp/9999".parse().expect("other");
        assert_eq!(transport_label(&other), "unknown");
    }
}
