//! Shared state between the network driver and synchronous callers.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use libp2p::Multiaddr;

use crate::state::{
    ConnectedPeer, NetworkEvent, NetworkObserver, NetworkState, NetworkStatus, PeerEventKind, PeerSnapshot, PeerState,
};

/// How long a freshly started node may take to report a listener.
pub(crate) const LISTENER_GRACE: Duration = Duration::from_secs(2);
/// Maximum retained peer records.
pub(crate) const MAX_PEER_RECORDS: usize = 256;
/// Maximum retained observed or confirmed addresses.
const MAX_TRACKED_ADDRESSES: usize = 16;

pub(crate) struct Shared {
    status: RwLock<NetworkStatus>,
    peers: RwLock<BTreeMap<String, PeerSnapshot>>,
    bootstrap: RwLock<Vec<String>>,
    relays: RwLock<Vec<String>>,
    blocked: RwLock<BTreeSet<String>>,
    observer: Arc<dyn NetworkObserver>,
    started_at: Instant,
}

impl Shared {
    pub(crate) fn new(mut status: NetworkStatus, observer: Arc<dyn NetworkObserver>) -> Self {
        status.bootstrap_configured = 0;
        Self {
            status: RwLock::new(status),
            peers: RwLock::new(BTreeMap::new()),
            bootstrap: RwLock::new(Vec::new()),
            relays: RwLock::new(Vec::new()),
            blocked: RwLock::new(BTreeSet::new()),
            observer,
            started_at: Instant::now(),
        }
    }

    pub(crate) fn status(&self) -> NetworkStatus {
        self.status
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub(crate) fn peers(&self) -> Vec<PeerSnapshot> {
        self.peers
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .values()
            .cloned()
            .collect()
    }

    pub(crate) fn bootstrap(&self) -> Vec<String> {
        self.bootstrap
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub(crate) fn set_bootstrap(&self, addresses: Vec<String>) {
        let mut bootstrap = self.bootstrap.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        *bootstrap = addresses;
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        status.bootstrap_configured = bootstrap.len();
    }

    pub(crate) fn relays(&self) -> Vec<String> {
        self.relays
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub(crate) fn set_relays(&self, addresses: Vec<String>) {
        let mut relays = self.relays.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        *relays = addresses;
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        status.relays_configured = relays.len();
    }

    /// Replaces the local block list. Blocking is a local policy decision and
    /// is never shared with other peers.
    pub(crate) fn set_blocked(&self, peers: Vec<String>) {
        let mut blocked = self.blocked.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        blocked.clear();
        blocked.extend(peers.into_iter().filter(|peer| !peer.is_empty()));
    }

    /// Returns true when local policy blocks the peer.
    pub(crate) fn is_blocked(&self, peer_id: &str) -> bool {
        self.blocked
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .contains(peer_id)
    }

    /// Number of locally blocked peers.
    pub(crate) fn blocked_count(&self) -> usize {
        self.blocked
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len()
    }

    pub(crate) fn configure_relay(
        &self,
        client: bool,
        fallback: bool,
        server: bool,
        max_bytes: u64,
        max_duration: Duration,
    ) {
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        status.relay_client_enabled = client;
        status.relay_fallback_enabled = fallback;
        status.relay_server_enabled = server;
        status.relay_server_max_circuit_bytes = server.then_some(max_bytes);
        status.relay_server_max_circuit_duration_secs = server.then_some(max_duration.as_secs());
        if client && !status.transports.iter().any(|transport| transport == "relay") {
            status.transports.push("relay".to_owned());
        }
    }

    pub(crate) fn set_relay_server_status(&self, enabled: bool) {
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        status.relay_server_enabled = enabled;
    }

    pub(crate) fn is_connected(&self, peer_id: &str) -> bool {
        self.peers
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(peer_id)
            .is_some_and(|peer| peer.state == PeerState::Connected)
    }

    pub(crate) fn record_dialing(&self, peer_id: &str) {
        let mut peers = self.peers.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        let peer = peers.entry(peer_id.to_owned()).or_insert_with(|| PeerSnapshot {
            peer_id: peer_id.to_owned(),
            state: PeerState::Connecting,
            direction: None,
            transport: None,
            remote_address: None,
            last_seen_ms: now_ms(),
            rtt_ms: None,
            protocols: Vec::new(),
            agent_version: None,
            error: None,
            connections: 0,
            path: None,
            relay_peer_id: None,
        });
        peer.state = PeerState::Connecting;
        peer.error = None;
        peer.last_seen_ms = now_ms();
        trim_peers(&mut peers);
        drop(peers);

        self.record_dial_attempt();
        self.emit_peer(peer_id, PeerEventKind::Dialing, None, None, None, None, None);
        self.recompute(None);
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record_connected(
        &self,
        peer_id: &str,
        direction: &'static str,
        transport: String,
        remote: &Multiaddr,
        established_in: Duration,
        path: &'static str,
        relay_peer_id: Option<&str>,
    ) -> ConnectedPeer {
        let mut peers = self.peers.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        let relay_text = relay_peer_id.map(str::to_owned);
        let peer = peers.entry(peer_id.to_owned()).or_insert_with(|| PeerSnapshot {
            peer_id: peer_id.to_owned(),
            state: PeerState::Connected,
            direction: Some(direction.to_owned()),
            transport: Some(transport.clone()),
            remote_address: Some(remote.to_string()),
            last_seen_ms: now_ms(),
            rtt_ms: None,
            protocols: Vec::new(),
            agent_version: None,
            error: None,
            connections: 0,
            path: Some(path.to_owned()),
            relay_peer_id: relay_text.clone(),
        });
        peer.state = PeerState::Connected;
        peer.direction = Some(direction.to_owned());
        peer.transport = Some(transport.clone());
        peer.remote_address = Some(remote.to_string());
        peer.last_seen_ms = now_ms();
        peer.error = None;
        peer.connections = peer.connections.saturating_add(1);
        if path == "direct" || peer.path.as_deref() != Some("direct") {
            peer.path = Some(path.to_owned());
            peer.relay_peer_id = relay_text.clone();
        }
        trim_peers(&mut peers);
        drop(peers);

        {
            let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
            if path == "relay" {
                status.relayed_connections = status.relayed_connections.saturating_add(1);
            } else {
                status.direct_connections = status.direct_connections.saturating_add(1);
            }
            status.last_path = Some(path.to_owned());
        }

        self.recompute(None);
        self.emit_peer(
            peer_id,
            PeerEventKind::Connected,
            Some(direction),
            Some(transport.clone()),
            Some(path),
            relay_peer_id,
            None,
        );
        ConnectedPeer {
            peer_id: peer_id.to_owned(),
            transport,
            remote_address: remote.to_string(),
            established_ms: u64::try_from(established_in.as_millis()).unwrap_or(u64::MAX),
            path: path.to_owned(),
            relay_peer_id: relay_text,
        }
    }

    pub(crate) fn record_disconnected(&self, peer_id: &str, reason: Option<&str>, path: &'static str) {
        let (remaining, known) = {
            let mut peers = self.peers.write().unwrap_or_else(|poisoned| poisoned.into_inner());
            match peers.get_mut(peer_id) {
                Some(peer) => {
                    peer.connections = peer.connections.saturating_sub(1);
                    if peer.connections == 0 {
                        peer.state = PeerState::Disconnected;
                        peer.direction = None;
                        peer.transport = None;
                        peer.remote_address = None;
                        peer.path = None;
                        peer.relay_peer_id = None;
                        peer.last_seen_ms = now_ms();
                    }
                    (peer.connections, true)
                }
                None => (0, false),
            }
        };
        if known {
            let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
            if path == "relay" {
                status.relayed_connections = status.relayed_connections.saturating_sub(1);
            } else {
                status.direct_connections = status.direct_connections.saturating_sub(1);
            }
        }
        self.emit_peer(peer_id, PeerEventKind::Disconnected, None, None, None, None, reason);
        if remaining == 0 {
            self.recompute(reason);
        }
    }

    pub(crate) fn record_failed(&self, peer_id: &str, code: &str) {
        {
            let mut peers = self.peers.write().unwrap_or_else(|poisoned| poisoned.into_inner());
            let peer = peers.entry(peer_id.to_owned()).or_insert_with(|| PeerSnapshot {
                peer_id: peer_id.to_owned(),
                state: PeerState::Failed,
                direction: None,
                transport: None,
                remote_address: None,
                last_seen_ms: now_ms(),
                rtt_ms: None,
                protocols: Vec::new(),
                agent_version: None,
                error: Some(code.to_owned()),
                connections: 0,
                path: None,
                relay_peer_id: None,
            });
            if peer.state != PeerState::Connected {
                peer.state = PeerState::Failed;
                peer.connections = 0;
                peer.path = None;
                peer.relay_peer_id = None;
            }
            peer.last_seen_ms = now_ms();
            peer.error = Some(code.to_owned());
            trim_peers(&mut peers);
        }
        self.record_dial_failure(code);
        self.emit_peer(peer_id, PeerEventKind::Failed, None, None, None, None, Some(code));
        self.recompute(Some(code));
    }

    pub(crate) fn record_identity(&self, peer_id: &str, protocols: Vec<String>, agent_version: Option<String>) {
        let mut peers = self.peers.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(peer) = peers.get_mut(peer_id) {
            peer.protocols = protocols;
            peer.agent_version = agent_version;
        }
    }

    /// Drops one connection record without emitting a peer-state event.
    pub(crate) fn record_connection_removed(&self, peer_id: &str, path: &'static str) {
        {
            let mut peers = self.peers.write().unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(peer) = peers.get_mut(peer_id) {
                peer.connections = peer.connections.saturating_sub(1);
            }
        }
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        if path == "relay" {
            status.relayed_connections = status.relayed_connections.saturating_sub(1);
        } else {
            status.direct_connections = status.direct_connections.saturating_sub(1);
        }
    }

    pub(crate) fn record_rtt(&self, peer_id: &str, rtt: Duration) {
        let mut peers = self.peers.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(peer) = peers.get_mut(peer_id) {
            peer.rtt_ms = Some(u64::try_from(rtt.as_millis()).unwrap_or(u64::MAX));
            peer.last_seen_ms = now_ms();
        }
    }

    pub(crate) fn add_listen(&self, address: Multiaddr) {
        let changed = {
            let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
            push_unique(&mut status.listen_addresses, address.to_string(), MAX_TRACKED_ADDRESSES)
        };
        if changed {
            self.recompute(None);
        }
    }

    pub(crate) fn remove_listen(&self, address: &Multiaddr) {
        let text = address.to_string();
        {
            let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
            status.listen_addresses.retain(|candidate| candidate != &text);
        }
        self.recompute(None);
    }

    pub(crate) fn add_observed(&self, address: &Multiaddr) {
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        push_unique(
            &mut status.observed_addresses,
            address.to_string(),
            MAX_TRACKED_ADDRESSES,
        );
    }

    pub(crate) fn add_confirmed(&self, address: &Multiaddr) {
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        push_unique(
            &mut status.confirmed_addresses,
            address.to_string(),
            MAX_TRACKED_ADDRESSES,
        );
    }

    pub(crate) fn remove_external(&self, address: &Multiaddr) {
        let text = address.to_string();
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        status.observed_addresses.retain(|candidate| candidate != &text);
        status.confirmed_addresses.retain(|candidate| candidate != &text);
    }

    pub(crate) fn record_request(&self, served: bool) {
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        if served {
            status.requests_served = status.requests_served.saturating_add(1);
        } else {
            status.requests_rejected = status.requests_rejected.saturating_add(1);
        }
    }

    pub(crate) fn set_dht_mode(&self, mode: &str) {
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        status.dht_mode = mode.to_owned();
    }

    pub(crate) fn set_dht_routing_peers(&self, count: usize) {
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        status.dht_routing_peers = count;
    }

    pub(crate) fn mark_dht_bootstrapped(&self) {
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        status.dht_bootstrapped = true;
    }

    pub(crate) fn record_dht_query_started(&self) {
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        status.dht_queries_started = status.dht_queries_started.saturating_add(1);
    }

    pub(crate) fn record_dht_query_failed(&self, code: &str) {
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        status.dht_queries_failed = status.dht_queries_failed.saturating_add(1);
        status.last_error = Some(code.to_owned());
    }

    pub(crate) fn record_dht_record(&self, accepted: bool) {
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        if accepted {
            status.dht_records_accepted = status.dht_records_accepted.saturating_add(1);
        } else {
            status.dht_records_rejected = status.dht_records_rejected.saturating_add(1);
        }
    }

    pub(crate) fn record_dht_provider(&self, accepted: bool) {
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        if accepted {
            status.dht_providers_accepted = status.dht_providers_accepted.saturating_add(1);
        } else {
            status.dht_providers_rejected = status.dht_providers_rejected.saturating_add(1);
        }
    }

    pub(crate) fn record_dht_publish(&self, success: bool) {
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        if success {
            status.dht_publish_successes = status.dht_publish_successes.saturating_add(1);
        } else {
            status.dht_publish_failures = status.dht_publish_failures.saturating_add(1);
        }
    }

    pub(crate) fn set_nat_status(&self, status_text: &str, public_address: Option<&Multiaddr>) {
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        status.nat_status = status_text.to_owned();
        status.nat_public_address = public_address.map(ToString::to_string);
    }

    pub(crate) fn record_nat_probe(&self) {
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        status.nat_probes = status.nat_probes.saturating_add(1);
    }

    pub(crate) fn set_relay_counts(&self, connected: usize, reservations: usize) {
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        status.relays_connected = connected;
        status.relay_reservations = reservations;
    }

    pub(crate) fn record_relay_reservation(&self, success: bool) {
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        if !success {
            status.relay_reservation_failures = status.relay_reservation_failures.saturating_add(1);
            status.last_error = Some("relay_reservation_failed".to_owned());
        }
    }

    pub(crate) fn set_relay_limits(&self, duration: Option<Duration>, bytes: Option<u64>) {
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        status.relay_duration_limit_secs = duration.map(|value| value.as_secs());
        status.relay_data_limit_bytes = bytes;
    }

    pub(crate) fn record_relay_circuit(&self) {
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        status.relay_circuits_established = status.relay_circuits_established.saturating_add(1);
    }

    pub(crate) fn record_fallback_attempt(&self) {
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        status.fallback_attempts = status.fallback_attempts.saturating_add(1);
    }

    pub(crate) fn record_fallback_success(&self) {
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        status.fallback_successes = status.fallback_successes.saturating_add(1);
    }

    pub(crate) fn record_hole_punch(&self, success: bool) {
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        if success {
            status.direct_upgrades = status.direct_upgrades.saturating_add(1);
            status.last_path = Some("direct".to_owned());
        } else {
            status.hole_punch_failures = status.hole_punch_failures.saturating_add(1);
        }
    }

    pub(crate) fn record_direct_upgrade(&self, peer_id: &str) {
        {
            let mut peers = self.peers.write().unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(peer) = peers.get_mut(peer_id) {
                peer.path = Some("direct".to_owned());
                peer.relay_peer_id = None;
                peer.last_seen_ms = now_ms();
            }
        }
        self.record_hole_punch(true);
    }

    pub(crate) fn record_hole_punch_failure(&self) {
        self.record_hole_punch(false);
    }

    pub(crate) fn record_relay_server_reservation(&self, opened: bool) {
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        if opened {
            status.relay_server_reservations = status.relay_server_reservations.saturating_add(1);
        } else {
            status.relay_server_reservations = status.relay_server_reservations.saturating_sub(1);
        }
    }

    pub(crate) fn record_relay_server_circuit(&self, opened: bool) {
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        if opened {
            status.relay_server_circuits = status.relay_server_circuits.saturating_add(1);
            status.relay_server_circuits_accepted = status.relay_server_circuits_accepted.saturating_add(1);
        } else {
            status.relay_server_circuits = status.relay_server_circuits.saturating_sub(1);
            status.relay_server_circuits_closed = status.relay_server_circuits_closed.saturating_add(1);
        }
    }

    pub(crate) fn record_relay_server_denied(&self) {
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        status.relay_server_denied = status.relay_server_denied.saturating_add(1);
    }

    pub(crate) fn record_last_error(&self, code: &str) {
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        status.last_error = Some(code.to_owned());
    }

    pub(crate) fn force_state(&self, state: NetworkState, reason: Option<&str>) {
        let (peer_count, transports) = {
            let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
            if status.state == state {
                return;
            }
            status.state = state;
            (status.peer_count, status.transports.clone())
        };
        self.observer.on_event(NetworkEvent::State {
            state,
            peer_count,
            transports,
            reason: reason.map(str::to_owned),
        });
    }

    /// Recomputes the aggregate state without a peer transition.
    pub(crate) fn refresh(&self) {
        self.recompute(None);
    }

    fn recompute(&self, reason: Option<&str>) {
        let (peer_count, connection_count, connected_ids) = {
            let peers = self.peers.read().unwrap_or_else(|poisoned| poisoned.into_inner());
            let connected_ids: Vec<String> = peers
                .values()
                .filter(|peer| peer.state == PeerState::Connected)
                .map(|peer| peer.peer_id.clone())
                .collect();
            (
                connected_ids.len(),
                peers.values().map(|peer| peer.connections).sum::<u32>(),
                connected_ids,
            )
        };
        let (state, has_listeners, transports) = {
            let status = self.status.read().unwrap_or_else(|poisoned| poisoned.into_inner());
            (
                status.state,
                !status.listen_addresses.is_empty(),
                status.transports.clone(),
            )
        };
        let next = if peer_count > 0 {
            NetworkState::Online
        } else if has_listeners {
            NetworkState::Degraded
        } else if self.started_at.elapsed() >= LISTENER_GRACE {
            NetworkState::Unavailable
        } else {
            NetworkState::Starting
        };
        let bootstrap_connected = self
            .bootstrap
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .iter()
            .filter_map(|address| address.rsplit_once("/p2p/").map(|(_, peer_id)| peer_id))
            .filter(|peer_id| connected_ids.iter().any(|connected| connected == peer_id))
            .count();
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        status.peer_count = peer_count;
        status.connection_count = usize::try_from(connection_count).unwrap_or(usize::MAX);
        status.bootstrap_connected = bootstrap_connected;
        if status.state == next && state == next {
            return;
        }
        status.state = next;
        drop(status);
        self.observer.on_event(NetworkEvent::State {
            state: next,
            peer_count,
            transports,
            reason: reason.map(str::to_owned),
        });
    }

    fn record_dial_attempt(&self) {
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        status.dial_attempts = status.dial_attempts.saturating_add(1);
    }

    fn record_dial_failure(&self, code: &str) {
        let mut status = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        status.dial_failures = status.dial_failures.saturating_add(1);
        status.last_error = Some(code.to_owned());
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_peer(
        &self,
        peer_id: &str,
        event: PeerEventKind,
        direction: Option<&str>,
        transport: Option<String>,
        path: Option<&str>,
        relay_peer_id: Option<&str>,
        reason: Option<&str>,
    ) {
        self.observer.on_event(NetworkEvent::Peer {
            peer_id: peer_id.to_owned(),
            event,
            direction: direction.map(str::to_owned),
            transport,
            path: path.map(str::to_owned),
            relay_peer_id: relay_peer_id.map(str::to_owned),
            reason: reason.map(str::to_owned),
        });
    }
}

fn trim_peers(peers: &mut BTreeMap<String, PeerSnapshot>) {
    while peers.len() > MAX_PEER_RECORDS {
        let candidate = peers
            .values()
            .filter(|peer| peer.state != PeerState::Connected)
            .min_by_key(|peer| peer.last_seen_ms)
            .map(|peer| peer.peer_id.clone());
        match candidate {
            Some(peer_id) => {
                peers.remove(&peer_id);
            }
            None => break,
        }
    }
}

fn push_unique(values: &mut Vec<String>, value: String, max: usize) -> bool {
    if values.iter().any(|candidate| candidate == &value) {
        return false;
    }
    if values.len() >= max {
        values.remove(0);
    }
    values.push(value);
    true
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::Shared;
    use crate::state::NetworkStatus;

    fn shared() -> Shared {
        Shared::new(NetworkStatus::starting("self".to_owned()), Arc::new(()))
    }

    #[test]
    fn block_list_replaces_deduplicates_and_clears() {
        let shared = shared();
        assert_eq!(shared.blocked_count(), 0);
        shared.set_blocked(vec![
            "peer-one".to_owned(),
            "peer-two".to_owned(),
            "peer-one".to_owned(),
            String::new(),
        ]);
        assert_eq!(shared.blocked_count(), 2);
        assert!(shared.is_blocked("peer-one"));
        assert!(shared.is_blocked("peer-two"));
        assert!(!shared.is_blocked("peer-three"));

        shared.set_blocked(vec!["peer-three".to_owned()]);
        assert_eq!(shared.blocked_count(), 1);
        assert!(!shared.is_blocked("peer-one"));
        assert!(shared.is_blocked("peer-three"));

        shared.set_blocked(Vec::new());
        assert_eq!(shared.blocked_count(), 0);
    }
}
