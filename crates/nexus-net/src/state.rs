//! Connection state model, snapshots, and observable network events.

/// Lifecycle state of the networking layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkState {
    /// Networking is disabled by configuration.
    Disabled,
    /// The node is loading its peer key or starting listeners.
    Starting,
    /// At least one peer is connected.
    Online,
    /// Listeners are active but no peer is connected, or peers are failing.
    Degraded,
    /// No listener or usable path is available.
    Unavailable,
    /// The node is shutting down.
    Stopping,
}

impl NetworkState {
    /// Stable wire representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Starting => "starting",
            Self::Online => "online",
            Self::Degraded => "degraded",
            Self::Unavailable => "unavailable",
            Self::Stopping => "stopping",
        }
    }
}

/// State of one observed peer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PeerState {
    /// A dial is in progress.
    Connecting,
    /// At least one authenticated connection is established.
    Connected,
    /// The peer disconnected and may be retried.
    Disconnected,
    /// The last attempt failed.
    Failed,
}

impl PeerState {
    /// Stable wire representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Connecting => "connecting",
            Self::Connected => "connected",
            Self::Disconnected => "disconnected",
            Self::Failed => "failed",
        }
    }
}

/// Bounded, redacted status for one peer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeerSnapshot {
    /// Authenticated libp2p peer ID.
    pub peer_id: String,
    /// Current connection state.
    pub state: PeerState,
    /// `inbound` or `outbound` for the active connection.
    pub direction: Option<String>,
    /// `quic` or `tcp` for the active connection.
    pub transport: Option<String>,
    /// Remote socket address used by the active connection.
    pub remote_address: Option<String>,
    /// Unix milliseconds of the last connection or disconnection.
    pub last_seen_ms: u64,
    /// Latest ping round-trip time in milliseconds.
    pub rtt_ms: Option<u64>,
    /// Protocols advertised through identify, capped to 16 entries.
    pub protocols: Vec<String>,
    /// Agent string advertised through identify, capped to 64 bytes.
    pub agent_version: Option<String>,
    /// Number of established connections to this peer.
    pub connections: u32,
    /// Stable failure code from the last failed attempt.
    pub error: Option<String>,
}

/// Aggregate network status exposed by `network.status`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkStatus {
    /// Lifecycle state.
    pub state: NetworkState,
    /// This node's device peer ID.
    pub peer_id: String,
    /// Local addresses that accepted a listener.
    pub listen_addresses: Vec<String>,
    /// Addresses a remote peer observed for us, not confirmed reachable.
    pub observed_addresses: Vec<String>,
    /// Addresses confirmed by an external address event.
    pub confirmed_addresses: Vec<String>,
    /// Enabled transports in preference order.
    pub transports: Vec<String>,
    /// Connected peer count.
    pub peer_count: usize,
    /// Established connection count across all peers.
    pub connection_count: usize,
    /// Configured bootstrap or manual peers.
    pub bootstrap_configured: usize,
    /// Bootstrap or manual peers currently connected.
    pub bootstrap_connected: usize,
    /// All dial attempts since start.
    pub dial_attempts: u64,
    /// Failed dial attempts since start.
    pub dial_failures: u64,
    /// Inbound transport-check requests served.
    pub requests_served: u64,
    /// Inbound transport-check requests rejected by the rate limit.
    pub requests_rejected: u64,
    /// Kademlia role: `client`, `server`, or `disabled`.
    pub dht_mode: String,
    /// Peers currently present in the local Kademlia routing table.
    pub dht_routing_peers: usize,
    /// Whether at least one Kademlia bootstrap query has succeeded.
    pub dht_bootstrapped: bool,
    /// DHT queries started since this node started.
    pub dht_queries_started: u64,
    /// DHT queries that timed out or returned a protocol error.
    pub dht_queries_failed: u64,
    /// Inbound DHT records accepted and stored after validation.
    pub dht_records_accepted: u64,
    /// Inbound DHT records rejected as malformed, wrongly keyed, or over quota.
    pub dht_records_rejected: u64,
    /// Inbound provider records accepted and stored after validation.
    pub dht_providers_accepted: u64,
    /// Inbound provider records rejected as malformed or over quota.
    pub dht_providers_rejected: u64,
    /// Local record publications that reached quorum.
    pub dht_publish_successes: u64,
    /// Local record publications that failed or timed out.
    pub dht_publish_failures: u64,
    /// Stable code for the most recent failure.
    pub last_error: Option<String>,
}

impl NetworkStatus {
    /// Creates the disabled status.
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            state: NetworkState::Disabled,
            peer_id: String::new(),
            listen_addresses: Vec::new(),
            observed_addresses: Vec::new(),
            confirmed_addresses: Vec::new(),
            transports: transports(),
            peer_count: 0,
            connection_count: 0,
            bootstrap_configured: 0,
            bootstrap_connected: 0,
            dial_attempts: 0,
            dial_failures: 0,
            requests_served: 0,
            requests_rejected: 0,
            dht_mode: "disabled".to_owned(),
            dht_routing_peers: 0,
            dht_bootstrapped: false,
            dht_queries_started: 0,
            dht_queries_failed: 0,
            dht_records_accepted: 0,
            dht_records_rejected: 0,
            dht_providers_accepted: 0,
            dht_providers_rejected: 0,
            dht_publish_successes: 0,
            dht_publish_failures: 0,
            last_error: None,
        }
    }

    /// Creates the starting status for a loaded peer key.
    #[must_use]
    pub fn starting(peer_id: String) -> Self {
        Self {
            state: NetworkState::Starting,
            peer_id,
            listen_addresses: Vec::new(),
            observed_addresses: Vec::new(),
            confirmed_addresses: Vec::new(),
            transports: transports(),
            peer_count: 0,
            connection_count: 0,
            bootstrap_configured: 0,
            bootstrap_connected: 0,
            dial_attempts: 0,
            dial_failures: 0,
            requests_served: 0,
            requests_rejected: 0,
            dht_mode: "client".to_owned(),
            dht_routing_peers: 0,
            dht_bootstrapped: false,
            dht_queries_started: 0,
            dht_queries_failed: 0,
            dht_records_accepted: 0,
            dht_records_rejected: 0,
            dht_providers_accepted: 0,
            dht_providers_rejected: 0,
            dht_publish_successes: 0,
            dht_publish_failures: 0,
            last_error: None,
        }
    }
}

/// Enabled transports in preference order.
#[must_use]
pub fn transports() -> Vec<String> {
    vec!["quic".to_owned(), "tcp".to_owned()]
}

/// Result of a direct connection request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectedPeer {
    /// Authenticated peer ID.
    pub peer_id: String,
    /// Negotiated transport label.
    pub transport: String,
    /// Remote socket address.
    pub remote_address: String,
    /// Time from dial start to established connection.
    pub established_ms: u64,
}

/// Result of one transport-check exchange.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestReply {
    /// Authenticated peer ID that answered.
    pub peer_id: String,
    /// Echoed payload.
    pub payload: Vec<u8>,
    /// Round-trip time for the request/response exchange.
    pub round_trip_ms: u64,
}

/// One verified DHT value returned by a lookup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DhtValue {
    /// 32-byte DHT key the value was stored under.
    pub key: Vec<u8>,
    /// Exact signed-envelope bytes, verified before this value was returned.
    pub value: Vec<u8>,
    /// Peer that served the value, when known.
    pub peer_id: Option<String>,
}

/// One peer returned by a Kademlia peer-routing query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredPeer {
    /// Authenticated libp2p peer ID.
    pub peer_id: String,
    /// Addresses advertised for this peer, capped to 8 entries.
    pub addresses: Vec<String>,
}

/// Result of one bounded peer-routing query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeerDiscovery {
    /// Closest peers returned by the query, sorted by peer ID.
    pub peers: Vec<DiscoveredPeer>,
    /// Peers currently present in the local routing table.
    pub routing_peers: usize,
    /// Query duration.
    pub elapsed_ms: u64,
}

/// Peer event kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PeerEventKind {
    /// A dial started.
    Dialing,
    /// An authenticated connection was established.
    Connected,
    /// The last connection closed.
    Disconnected,
    /// The last attempt failed.
    Failed,
}

impl PeerEventKind {
    /// Stable wire representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dialing => "dialing",
            Self::Connected => "connected",
            Self::Disconnected => "disconnected",
            Self::Failed => "failed",
        }
    }
}

/// Observable network state change.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NetworkEvent {
    /// Aggregate state changed.
    State {
        /// New state.
        state: NetworkState,
        /// Connected peers at the time of the change.
        peer_count: usize,
        /// Enabled transports.
        transports: Vec<String>,
        /// Stable reason code when the state is not online.
        reason: Option<String>,
    },
    /// One peer changed state.
    Peer {
        /// Authenticated peer ID.
        peer_id: String,
        /// Event kind.
        event: PeerEventKind,
        /// `inbound` or `outbound` when known.
        direction: Option<String>,
        /// `quic` or `tcp` when known.
        transport: Option<String>,
        /// Stable reason code for failures and disconnections.
        reason: Option<String>,
    },
}

/// Receives state and peer events from the network driver.
///
/// Implementations must not block: the driver calls this from its event loop.
pub trait NetworkObserver: Send + Sync {
    /// Handles one event.
    fn on_event(&self, event: NetworkEvent);
}

impl NetworkObserver for () {
    fn on_event(&self, _event: NetworkEvent) {}
}

#[cfg(test)]
mod tests {
    use super::{NetworkState, PeerEventKind, PeerState, transports};

    #[test]
    fn wire_names_are_stable() {
        assert_eq!(NetworkState::Online.as_str(), "online");
        assert_eq!(NetworkState::Degraded.as_str(), "degraded");
        assert_eq!(PeerState::Connected.as_str(), "connected");
        assert_eq!(PeerEventKind::Disconnected.as_str(), "disconnected");
        assert_eq!(transports(), vec!["quic".to_owned(), "tcp".to_owned()]);
    }
}
