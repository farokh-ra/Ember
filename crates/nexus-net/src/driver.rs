//! Background swarm driver: commands, deadlines, reconnect backoff, and events.
//!
//! One OS thread owns a current-thread Tokio runtime and the libp2p swarm.
//! Synchronous callers submit bounded commands and block on a one-shot reply;
//! the driver never blocks the swarm loop on a caller.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::StreamExt;
use libp2p::request_response::{self, OutboundFailure, OutboundRequestId, ResponseChannel};
use libp2p::swarm::dial_opts::{DialOpts, PeerCondition};
use libp2p::swarm::{DialError, SwarmEvent};
use libp2p::{Multiaddr, PeerId, Swarm, identify, ping};
use nexus_protocol::{
    TestResponse, decode_test_request, decode_test_response, encode_test_request, encode_test_response,
};
use tokio::sync::{mpsc, oneshot};

use crate::behaviour::{Behaviour, BehaviourEvent};
use crate::bootstrap;
use crate::config::{NetworkConfig, split_peer_address};
use crate::error::NetError;
use crate::shared::Shared;
use crate::state::{ConnectedPeer, NetworkState, TestReply};

/// Bounded command queue depth.
pub(crate) const COMMAND_CAPACITY: usize = 64;
/// Driver tick for deadlines, reconnect backoff, and state refresh.
const TICK: Duration = Duration::from_millis(250);
/// Extra grace beyond the request timeout before the driver fails a request.
const REQUEST_GRACE: Duration = Duration::from_secs(1);
/// Maximum retained per-peer rate-limit windows.
const MAX_RATE_WINDOWS: usize = 256;

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
    BootstrapAdd {
        address: Multiaddr,
        respond: oneshot::Sender<Result<(), NetError>>,
    },
    BootstrapRemove {
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

struct PendingDial {
    respond: oneshot::Sender<Result<ConnectedPeer, NetError>>,
    deadline: Instant,
}

struct PendingRequest {
    respond: oneshot::Sender<Result<TestReply, NetError>>,
    peer_id: PeerId,
    started: Instant,
    deadline: Instant,
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
    backoff: HashMap<PeerId, Backoff>,
    targets: BTreeMap<PeerId, Vec<Multiaddr>>,
    rate: HashMap<PeerId, RateWindow>,
}

impl Driver {
    /// Builds the swarm, starts listeners, and dials configured bootstrap peers.
    pub(crate) fn start(
        config: Arc<NetworkConfig>,
        keypair: libp2p::identity::Keypair,
        shared: Arc<Shared>,
        commands: mpsc::Receiver<Command>,
    ) -> Result<Self, NetError> {
        let mut targets: BTreeMap<PeerId, Vec<Multiaddr>> = BTreeMap::new();
        for address in &config.bootstrap {
            let (peer_id, address) = split_peer_address(address)?;
            targets.entry(peer_id).or_default().push(address);
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
            .with_behaviour(|key| {
                Ok::<Behaviour, Box<dyn std::error::Error + Send + Sync>>(Behaviour::new(key, &behaviour_config))
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

        let mut driver = Self {
            swarm,
            shared,
            config,
            commands,
            pending_dials: HashMap::new(),
            pending_requests: HashMap::new(),
            backoff: HashMap::new(),
            targets,
            rate: HashMap::new(),
        };

        let peers: Vec<PeerId> = driver.targets.keys().copied().collect();
        for peer_id in peers {
            let addresses = driver.targets.get(&peer_id).cloned().unwrap_or_default();
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
            Command::BootstrapAdd { address, respond } => self.bootstrap_add(address, respond),
            Command::BootstrapRemove { address, respond } => self.bootstrap_remove(&address, respond),
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
                        deadline: Instant::now() + self.config.connect_timeout,
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
                if let Ok((peer_id, _)) = split_peer_address(address) {
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
            if let Some(pending) = self.pending_dials.remove(&peer_id) {
                let _ = pending.respond.send(Err(NetError::Timeout));
                self.shared.record_failed(&peer_id.to_string(), "timeout");
                self.schedule_retry(&peer_id);
            }
        }

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

        if self.rate.len() > MAX_RATE_WINDOWS {
            self.rate
                .retain(|_, window| window.started.elapsed() < Duration::from_secs(60));
        }

        let due: Vec<PeerId> = self
            .backoff
            .iter()
            .filter(|(peer_id, backoff)| {
                backoff.next_at <= now && !self.pending_dials.contains_key(peer_id) && !self.swarm.is_connected(peer_id)
            })
            .map(|(peer_id, _)| *peer_id)
            .collect();
        for peer_id in due {
            let addresses = self.targets.get(&peer_id).cloned().unwrap_or_default();
            self.dial_background(peer_id, addresses);
        }

        self.shared.refresh();
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
            SwarmEvent::NewListenAddr { address, .. } => self.shared.add_listen(address),
            SwarmEvent::ExpiredListenAddr { address, .. } => self.shared.remove_listen(&address),
            SwarmEvent::ListenerClosed { addresses, reason, .. } => {
                for address in addresses {
                    self.shared.remove_listen(&address);
                }
                if reason.is_err() {
                    self.shared.record_last_error("listener_closed");
                }
            }
            SwarmEvent::ListenerError { .. } => self.shared.record_last_error("listener_error"),
            SwarmEvent::ConnectionEstablished {
                peer_id,
                endpoint,
                num_established,
                established_in,
                ..
            } => {
                let direction = if endpoint.is_dialer() { "outbound" } else { "inbound" };
                let transport = transport_label(endpoint.get_remote_address());
                let connected = self.shared.record_connected(
                    &peer_id.to_string(),
                    direction,
                    transport,
                    endpoint.get_remote_address(),
                    established_in,
                );
                if num_established.get() == 1 {
                    self.backoff.remove(&peer_id);
                }
                if let Some(pending) = self.pending_dials.remove(&peer_id) {
                    let _ = pending.respond.send(Ok(connected));
                }
            }
            SwarmEvent::ConnectionClosed {
                peer_id,
                num_established,
                cause,
                ..
            } => {
                if num_established == 0 {
                    let reason = if cause.is_some() {
                        "connection_closed"
                    } else {
                        "peer_closed"
                    };
                    self.shared.record_disconnected(&peer_id.to_string(), Some(reason));
                    if self.targets.contains_key(&peer_id) {
                        self.schedule_retry(&peer_id);
                    }
                } else {
                    self.shared.record_connection_removed(&peer_id.to_string());
                }
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                let (code, _) = map_dial_error(&error);
                self.shared.record_last_error(code);
                if let Some(peer_id) = peer_id {
                    self.shared.record_failed(&peer_id.to_string(), code);
                    if let Some(pending) = self.pending_dials.remove(&peer_id) {
                        let _ = pending.respond.send(Err(map_dial_error(&error).1));
                    }
                    self.schedule_retry(&peer_id);
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
                }
            }
            BehaviourEvent::Ping(ping::Event {
                peer, result: Ok(rtt), ..
            }) => self.shared.record_rtt(&peer.to_string(), rtt),
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
            _ => {}
        }
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
        let window = self.rate.entry(peer).or_insert(RateWindow {
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

fn map_outbound_failure(error: OutboundFailure) -> (&'static str, NetError) {
    match error {
        OutboundFailure::DialFailure => ("dial_failure", NetError::ConnectionFailed),
        OutboundFailure::ConnectionClosed => ("connection_closed", NetError::ConnectionFailed),
        OutboundFailure::Timeout => ("timeout", NetError::Timeout),
        OutboundFailure::UnsupportedProtocols => ("unsupported_protocol", NetError::UnsupportedProtocol),
        OutboundFailure::Io(_) => ("io_error", NetError::ConnectionFailed),
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
