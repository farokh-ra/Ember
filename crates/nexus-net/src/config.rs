//! Bounded networking configuration and its validation rules.

use std::path::PathBuf;
use std::time::Duration;

use libp2p::{Multiaddr, PeerId};

use crate::error::NetError;

/// Default connect deadline for one direct dial.
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Default request/response deadline.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(8);
/// Default first reconnect delay.
pub const DEFAULT_RECONNECT_BASE_DELAY: Duration = Duration::from_secs(1);
/// Default maximum reconnect delay.
pub const DEFAULT_RECONNECT_MAX_DELAY: Duration = Duration::from_secs(60);
/// Default maximum established connections.
pub const DEFAULT_MAX_ESTABLISHED: u32 = 64;
/// Default maximum established connections per peer.
pub const DEFAULT_MAX_ESTABLISHED_PER_PEER: u32 = 2;
/// Default maximum pending inbound connections.
pub const DEFAULT_MAX_PENDING_INCOMING: u32 = 16;
/// Default maximum concurrent request-response streams.
pub const DEFAULT_MAX_CONCURRENT_STREAMS: usize = 32;
/// Default inbound transport-check requests allowed per peer per minute.
pub const DEFAULT_MAX_REQUESTS_PER_PEER_PER_MINUTE: u32 = 60;
/// Default idle connection timeout.
pub const DEFAULT_IDLE_CONNECTION_TIMEOUT: Duration = Duration::from_secs(60);
/// Default deadline for one DHT lookup or publication.
pub const DEFAULT_DHT_QUERY_TIMEOUT: Duration = Duration::from_secs(30);
/// Default lifetime of one stored DHT record or provider record.
pub const DEFAULT_DHT_RECORD_TTL: Duration = Duration::from_secs(24 * 60 * 60);
/// Default interval for rebootstrap and provider republication.
pub const DEFAULT_DHT_REPUBLISH_INTERVAL: Duration = Duration::from_secs(12 * 60 * 60);
/// Default maximum stored DHT value records.
pub const DEFAULT_DHT_STORE_MAX_RECORDS: usize = 1_024;
/// Default maximum providers retained for one key.
pub const DEFAULT_DHT_STORE_MAX_PROVIDERS: usize = 20;
/// Default maximum content keys this node advertises as a provider.
pub const DEFAULT_DHT_STORE_MAX_PROVIDED_KEYS: usize = 512;
/// Default deadline for one relay reservation request.
pub const DEFAULT_RELAY_RESERVATION_TIMEOUT: Duration = Duration::from_secs(30);
/// Default maximum active reservations accepted by a relay server.
pub const DEFAULT_RELAY_MAX_RESERVATIONS: usize = 128;
/// Default maximum active circuits accepted by a relay server.
pub const DEFAULT_RELAY_MAX_CIRCUITS: usize = 16;
/// Default maximum active circuits accepted per source peer.
pub const DEFAULT_RELAY_MAX_CIRCUITS_PER_PEER: usize = 4;
/// Default maximum lifetime of one relayed circuit.
pub const DEFAULT_RELAY_MAX_CIRCUIT_DURATION: Duration = Duration::from_secs(2 * 60);
/// Default maximum bytes forwarded over one relayed circuit.
pub const DEFAULT_RELAY_MAX_CIRCUIT_BYTES: u64 = 128 * 1024;

/// Highest accepted value for any connection limit.
pub const MAX_CONNECTION_LIMIT: u32 = 512;
/// Highest accepted bootstrap/manual peer count.
pub const MAX_BOOTSTRAP_PEERS: usize = 64;
/// Highest accepted listen-address count.
pub const MAX_LISTEN_ADDRESSES: usize = 8;
/// Highest accepted explicitly advertised external-address count.
pub const MAX_EXTERNAL_ADDRESSES: usize = 8;
/// Highest accepted DHT query timeout.
pub const MAX_DHT_QUERY_TIMEOUT: Duration = Duration::from_secs(120);
/// Highest accepted DHT record lifetime.
pub const MAX_DHT_RECORD_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);
/// Highest accepted stored DHT value records.
pub const MAX_DHT_STORE_RECORDS: usize = 8_192;
/// Highest accepted providers retained for one key.
pub const MAX_DHT_STORE_PROVIDERS: usize = 128;
/// Highest accepted locally advertised provider keys.
pub const MAX_DHT_PROVIDED_KEYS: usize = 4_096;
/// Highest accepted configured relay count.
pub const MAX_RELAYS: usize = 16;
/// Highest accepted relay reservation request timeout.
pub const MAX_RELAY_RESERVATION_TIMEOUT: Duration = Duration::from_secs(120);
/// Highest accepted active reservations on a relay server.
pub const MAX_RELAY_RESERVATIONS: usize = 1_024;
/// Highest accepted active circuits on a relay server.
pub const MAX_RELAY_CIRCUITS: usize = 256;
/// Highest accepted lifetime of one relayed circuit.
pub const MAX_RELAY_CIRCUIT_DURATION: Duration = Duration::from_secs(60 * 60);
/// Highest accepted bytes forwarded over one relayed circuit.
pub const MAX_RELAY_CIRCUIT_BYTES: u64 = 16 * 1024 * 1024;
/// Smallest accepted bytes forwarded over one relayed circuit.
pub const MIN_RELAY_CIRCUIT_BYTES: u64 = 1_024;
/// Smallest accepted upload rate for hosted content.
pub const MIN_UPLOAD_BYTES_PER_SECOND: u64 = 256 * 1024;
/// Largest accepted upload rate for hosted content.
pub const MAX_UPLOAD_BYTES_PER_SECOND: u64 = 64 * 1024 * 1024;

/// Role this node takes in the Kademlia DHT.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DhtMode {
    /// Query the DHT, store only records routed to this node, and serve them.
    #[default]
    Client,
    /// Act as a public routing and storage node, typically a bootstrap node.
    Server,
}

impl DhtMode {
    /// Stable wire representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Client => "client",
            Self::Server => "server",
        }
    }
}

/// QUIC v1 over UDP, preferred transport.
pub const QUIC_LISTEN_ADDRESS: &str = "/ip4/0.0.0.0/udp/0/quic-v1";
/// TCP with Noise and Yamux, fallback transport.
pub const TCP_LISTEN_ADDRESS: &str = "/ip4/0.0.0.0/tcp/0";

/// Configuration for one peer node.
#[derive(Clone, Debug)]
pub struct NetworkConfig {
    /// Absolute app-private root that owns `network/peer_key.enc`.
    pub data_root: PathBuf,
    /// Addresses to listen on. Empty selects QUIC and TCP on ephemeral ports.
    pub listen: Vec<Multiaddr>,
    /// Addresses other peers can use to reach this node, as confirmed by the
    /// operator. A relay server must advertise at least one reachable address
    /// before libp2p offers the relay protocol. Loopback listen addresses are
    /// advertised automatically when relay serving is enabled.
    pub external_addresses: Vec<Multiaddr>,
    /// Bootstrap or manual peers dialed at startup and retried with backoff.
    pub bootstrap: Vec<Multiaddr>,
    /// Maximum established connections across all peers.
    pub max_established: u32,
    /// Maximum established connections per peer.
    pub max_established_per_peer: u32,
    /// Maximum pending inbound connections.
    pub max_pending_incoming: u32,
    /// Maximum concurrent request-response streams.
    pub max_concurrent_streams: usize,
    /// Deadline for one direct dial.
    pub connect_timeout: Duration,
    /// Deadline for one request/response exchange.
    pub request_timeout: Duration,
    /// First reconnect delay.
    pub reconnect_base_delay: Duration,
    /// Maximum reconnect delay.
    pub reconnect_max_delay: Duration,
    /// Inbound transport-check requests allowed per peer per minute.
    pub max_requests_per_peer_per_minute: u32,
    /// Idle connection timeout.
    pub idle_connection_timeout: Duration,
    /// Kademlia role for this node.
    pub dht_mode: DhtMode,
    /// Deadline for one DHT query.
    pub dht_query_timeout: Duration,
    /// Lifetime of a stored DHT record or provider record.
    pub dht_record_ttl: Duration,
    /// Rebootstrap and provider republication interval.
    pub dht_republish_interval: Duration,
    /// Maximum value records retained in the local DHT store.
    pub dht_store_max_records: usize,
    /// Maximum providers retained for one key.
    pub dht_store_max_providers: usize,
    /// Maximum content keys this node advertises as a provider.
    pub dht_store_max_provided_keys: usize,
    /// Relay servers offered to the client transport.
    pub relays: Vec<Multiaddr>,
    /// Whether this node requests relay reservations and dials circuits.
    pub relay_client_enabled: bool,
    /// Whether a failed direct dial falls back to an available relay.
    pub relay_fallback: bool,
    /// Deadline for one relay reservation request.
    pub relay_reservation_timeout: Duration,
    /// Whether this node serves as a community relay.
    pub relay_server_enabled: bool,
    /// Maximum reservations accepted by this relay server.
    pub relay_max_reservations: usize,
    /// Maximum circuits accepted by this relay server.
    pub relay_max_circuits: usize,
    /// Maximum circuits accepted per source peer by this relay server.
    pub relay_max_circuits_per_peer: usize,
    /// Maximum lifetime of one circuit served by this relay.
    pub relay_max_circuit_duration: Duration,
    /// Maximum bytes forwarded over one circuit served by this relay.
    pub relay_max_circuit_bytes: u64,
    /// Whether AutoNAT probes estimate public reachability.
    pub autonat_enabled: bool,
    /// Whether relayed connections try a DCUtR direct upgrade.
    pub dcutr_enabled: bool,
    /// Maximum upload rate for served file chunks in bytes per second; zero
    /// means unlimited.
    pub upload_bytes_per_second: u64,
}

impl NetworkConfig {
    /// Creates a configuration with the documented defaults.
    #[must_use]
    pub fn new(data_root: impl Into<PathBuf>) -> Self {
        Self {
            data_root: data_root.into(),
            listen: Vec::new(),
            external_addresses: Vec::new(),
            bootstrap: Vec::new(),
            max_established: DEFAULT_MAX_ESTABLISHED,
            max_established_per_peer: DEFAULT_MAX_ESTABLISHED_PER_PEER,
            max_pending_incoming: DEFAULT_MAX_PENDING_INCOMING,
            max_concurrent_streams: DEFAULT_MAX_CONCURRENT_STREAMS,
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
            reconnect_base_delay: DEFAULT_RECONNECT_BASE_DELAY,
            reconnect_max_delay: DEFAULT_RECONNECT_MAX_DELAY,
            max_requests_per_peer_per_minute: DEFAULT_MAX_REQUESTS_PER_PEER_PER_MINUTE,
            idle_connection_timeout: DEFAULT_IDLE_CONNECTION_TIMEOUT,
            dht_mode: DhtMode::Client,
            dht_query_timeout: DEFAULT_DHT_QUERY_TIMEOUT,
            dht_record_ttl: DEFAULT_DHT_RECORD_TTL,
            dht_republish_interval: DEFAULT_DHT_REPUBLISH_INTERVAL,
            dht_store_max_records: DEFAULT_DHT_STORE_MAX_RECORDS,
            dht_store_max_providers: DEFAULT_DHT_STORE_MAX_PROVIDERS,
            dht_store_max_provided_keys: DEFAULT_DHT_STORE_MAX_PROVIDED_KEYS,
            relays: Vec::new(),
            relay_client_enabled: true,
            relay_fallback: true,
            relay_reservation_timeout: DEFAULT_RELAY_RESERVATION_TIMEOUT,
            relay_server_enabled: false,
            relay_max_reservations: DEFAULT_RELAY_MAX_RESERVATIONS,
            relay_max_circuits: DEFAULT_RELAY_MAX_CIRCUITS,
            relay_max_circuits_per_peer: DEFAULT_RELAY_MAX_CIRCUITS_PER_PEER,
            relay_max_circuit_duration: DEFAULT_RELAY_MAX_CIRCUIT_DURATION,
            relay_max_circuit_bytes: DEFAULT_RELAY_MAX_CIRCUIT_BYTES,
            autonat_enabled: true,
            dcutr_enabled: true,
            upload_bytes_per_second: 0,
        }
    }

    /// Returns the effective listen addresses.
    pub fn effective_listen(&self) -> Result<Vec<Multiaddr>, NetError> {
        if self.listen.is_empty() {
            let quic = QUIC_LISTEN_ADDRESS
                .parse()
                .map_err(|_| NetError::InvalidConfig("listen"))?;
            let tcp = TCP_LISTEN_ADDRESS
                .parse()
                .map_err(|_| NetError::InvalidConfig("listen"))?;
            Ok(vec![quic, tcp])
        } else {
            Ok(self.listen.clone())
        }
    }

    /// Validates bounds before any socket or key work begins.
    pub fn validate(&self) -> Result<(), NetError> {
        if !self.data_root.is_absolute() {
            return Err(NetError::InvalidConfig("data_root"));
        }
        if self.listen.len() > MAX_LISTEN_ADDRESSES {
            return Err(NetError::InvalidConfig("listen"));
        }
        if self.external_addresses.len() > MAX_EXTERNAL_ADDRESSES {
            return Err(NetError::InvalidConfig("external_addresses"));
        }
        if self.bootstrap.len() > MAX_BOOTSTRAP_PEERS {
            return Err(NetError::InvalidConfig("bootstrap"));
        }
        if self.relays.len() > MAX_RELAYS {
            return Err(NetError::InvalidConfig("relays"));
        }
        for address in &self.listen {
            require_peer_id(address, false)?;
        }
        for address in &self.external_addresses {
            require_peer_id(address, false)?;
        }
        for address in &self.bootstrap {
            require_peer_id(address, true)?;
        }
        for address in &self.relays {
            require_peer_id(address, true)?;
        }
        for limit in [
            self.max_established,
            self.max_established_per_peer,
            self.max_pending_incoming,
        ] {
            if limit == 0 || limit > MAX_CONNECTION_LIMIT {
                return Err(NetError::InvalidConfig("connection_limit"));
            }
        }
        if self.max_concurrent_streams == 0 || self.max_concurrent_streams > 256 {
            return Err(NetError::InvalidConfig("max_concurrent_streams"));
        }
        if !(1..=100).contains(&self.max_requests_per_peer_per_minute) {
            return Err(NetError::InvalidConfig("max_requests_per_peer_per_minute"));
        }
        for timeout in [self.connect_timeout, self.request_timeout, self.idle_connection_timeout] {
            if timeout.is_zero() || timeout > Duration::from_secs(120) {
                return Err(NetError::InvalidConfig("timeout"));
            }
        }
        if self.reconnect_base_delay.is_zero()
            || self.reconnect_base_delay > self.reconnect_max_delay
            || self.reconnect_max_delay > Duration::from_secs(600)
        {
            return Err(NetError::InvalidConfig("reconnect_delay"));
        }
        if self.dht_query_timeout.is_zero() || self.dht_query_timeout > MAX_DHT_QUERY_TIMEOUT {
            return Err(NetError::InvalidConfig("dht_query_timeout"));
        }
        if self.dht_record_ttl < Duration::from_secs(60) || self.dht_record_ttl > MAX_DHT_RECORD_TTL {
            return Err(NetError::InvalidConfig("dht_record_ttl"));
        }
        if self.dht_republish_interval < Duration::from_secs(60) || self.dht_republish_interval > self.dht_record_ttl {
            return Err(NetError::InvalidConfig("dht_republish_interval"));
        }
        if !(1..=MAX_DHT_STORE_RECORDS).contains(&self.dht_store_max_records)
            || !(1..=MAX_DHT_STORE_PROVIDERS).contains(&self.dht_store_max_providers)
            || !(1..=MAX_DHT_PROVIDED_KEYS).contains(&self.dht_store_max_provided_keys)
        {
            return Err(NetError::InvalidConfig("dht_store"));
        }
        if self.relay_reservation_timeout.is_zero() || self.relay_reservation_timeout > MAX_RELAY_RESERVATION_TIMEOUT {
            return Err(NetError::InvalidConfig("relay_reservation_timeout"));
        }
        if !(1..=MAX_RELAY_RESERVATIONS).contains(&self.relay_max_reservations)
            || !(1..=MAX_RELAY_CIRCUITS).contains(&self.relay_max_circuits)
            || !(1..=self.relay_max_circuits).contains(&self.relay_max_circuits_per_peer)
        {
            return Err(NetError::InvalidConfig("relay_server_limits"));
        }
        if self.relay_max_circuit_duration.is_zero() || self.relay_max_circuit_duration > MAX_RELAY_CIRCUIT_DURATION {
            return Err(NetError::InvalidConfig("relay_max_circuit_duration"));
        }
        if !(MIN_RELAY_CIRCUIT_BYTES..=MAX_RELAY_CIRCUIT_BYTES).contains(&self.relay_max_circuit_bytes) {
            return Err(NetError::InvalidConfig("relay_max_circuit_bytes"));
        }
        if self.upload_bytes_per_second != 0
            && !(MIN_UPLOAD_BYTES_PER_SECOND..=MAX_UPLOAD_BYTES_PER_SECOND).contains(&self.upload_bytes_per_second)
        {
            return Err(NetError::InvalidConfig("upload_bytes_per_second"));
        }
        Ok(())
    }

    /// Exponential reconnect delay capped at the configured maximum.
    #[must_use]
    pub fn reconnect_delay(&self, attempts: u32) -> Duration {
        let exponent = attempts.saturating_sub(1).min(16);
        let factor = 1_u32 << exponent;
        self.reconnect_base_delay
            .saturating_mul(factor)
            .min(self.reconnect_max_delay)
    }
}

/// Requires a `/p2p/<peer-id>` suffix when `required` is true.
pub fn require_peer_id(address: &Multiaddr, required: bool) -> Result<(), NetError> {
    use libp2p::multiaddr::Protocol;

    let peer_id = address.iter().last();
    match peer_id {
        Some(Protocol::P2p(_)) => Ok(()),
        _ if required => Err(NetError::MissingPeerId),
        _ => Ok(()),
    }
}

/// Splits a `/p2p/<peer-id>` terminated address into its authenticated peer ID.
pub(crate) fn split_peer_address(address: &Multiaddr) -> Result<(PeerId, Multiaddr), NetError> {
    use libp2p::multiaddr::Protocol;

    match address.iter().last() {
        Some(Protocol::P2p(peer_id)) => Ok((peer_id, address.clone())),
        _ => Err(NetError::MissingPeerId),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use libp2p::Multiaddr;

    use super::{
        DhtMode, MAX_RELAY_CIRCUIT_BYTES, NetworkConfig, QUIC_LISTEN_ADDRESS, TCP_LISTEN_ADDRESS, require_peer_id,
    };
    use crate::error::NetError;

    fn config() -> NetworkConfig {
        NetworkConfig::new(std::env::temp_dir().join("nexus-net-config"))
    }

    #[test]
    fn defaults_enable_quic_and_tcp() {
        let addresses = config().effective_listen().expect("listen");
        assert_eq!(addresses.len(), 2);
        assert_eq!(addresses[0].to_string(), QUIC_LISTEN_ADDRESS);
        assert_eq!(addresses[1].to_string(), TCP_LISTEN_ADDRESS);
    }

    #[test]
    fn validates_bounds() {
        assert!(config().validate().is_ok());

        let mut invalid = config();
        invalid.max_established = 0;
        assert!(matches!(invalid.validate(), Err(NetError::InvalidConfig(_))));

        let mut invalid = config();
        invalid.connect_timeout = Duration::ZERO;
        assert!(matches!(invalid.validate(), Err(NetError::InvalidConfig(_))));

        let mut relative = config();
        relative.data_root = "relative".into();
        assert!(matches!(relative.validate(), Err(NetError::InvalidConfig(_))));
    }

    #[test]
    fn requires_peer_id_for_bootstrap_addresses() {
        let address: Multiaddr = "/ip4/127.0.0.1/tcp/4001".parse().expect("address");
        assert!(matches!(require_peer_id(&address, true), Err(NetError::MissingPeerId)));
        assert!(require_peer_id(&address, false).is_ok());
    }

    #[test]
    fn reconnect_delay_is_exponential_and_capped() {
        let config = config();
        assert_eq!(config.reconnect_delay(1), Duration::from_secs(1));
        assert_eq!(config.reconnect_delay(2), Duration::from_secs(2));
        assert_eq!(config.reconnect_delay(4), Duration::from_secs(8));
        assert_eq!(config.reconnect_delay(20), config.reconnect_max_delay);
    }

    #[test]
    fn dht_defaults_are_client_mode_and_bounded() {
        let config = config();
        assert_eq!(config.dht_mode, DhtMode::Client);
        assert_eq!(DhtMode::Client.as_str(), "client");
        assert_eq!(DhtMode::Server.as_str(), "server");

        let mut invalid = config.clone();
        invalid.dht_query_timeout = Duration::ZERO;
        assert!(matches!(
            invalid.validate(),
            Err(NetError::InvalidConfig("dht_query_timeout"))
        ));

        let mut invalid = config.clone();
        invalid.dht_record_ttl = Duration::from_secs(30);
        assert!(matches!(
            invalid.validate(),
            Err(NetError::InvalidConfig("dht_record_ttl"))
        ));

        let mut invalid = config.clone();
        invalid.dht_republish_interval = invalid.dht_record_ttl + Duration::from_secs(1);
        assert!(matches!(
            invalid.validate(),
            Err(NetError::InvalidConfig("dht_republish_interval"))
        ));

        let mut invalid = config;
        invalid.dht_store_max_records = 0;
        assert!(matches!(invalid.validate(), Err(NetError::InvalidConfig("dht_store"))));
    }

    #[test]
    fn relay_defaults_are_bounded_and_disabled_only_for_serving() {
        let config = config();
        assert!(config.relay_client_enabled);
        assert!(config.relay_fallback);
        assert!(!config.relay_server_enabled);
        assert!(config.autonat_enabled);
        assert!(config.dcutr_enabled);
        assert!(config.validate().is_ok());

        let mut invalid = config.clone();
        invalid.relays = vec!["/ip4/127.0.0.1/tcp/4001".parse().expect("address")];
        assert!(matches!(invalid.validate(), Err(NetError::MissingPeerId)));

        let mut invalid = config.clone();
        invalid.relay_reservation_timeout = Duration::ZERO;
        assert!(matches!(
            invalid.validate(),
            Err(NetError::InvalidConfig("relay_reservation_timeout"))
        ));

        let mut invalid = config.clone();
        invalid.relay_max_circuits = 2;
        invalid.relay_max_circuits_per_peer = 3;
        assert!(matches!(
            invalid.validate(),
            Err(NetError::InvalidConfig("relay_server_limits"))
        ));

        let mut invalid = config;
        invalid.relay_max_circuit_bytes = MAX_RELAY_CIRCUIT_BYTES + 1;
        assert!(matches!(
            invalid.validate(),
            Err(NetError::InvalidConfig("relay_max_circuit_bytes"))
        ));
    }

    #[test]
    fn external_addresses_are_bounded() {
        let mut config = config();
        assert!(config.external_addresses.is_empty());
        config.external_addresses = vec!["/ip4/203.0.113.7/tcp/4001".parse().expect("address")];
        assert!(config.validate().is_ok());

        let mut invalid = config;
        invalid.external_addresses = (0..=super::MAX_EXTERNAL_ADDRESSES)
            .map(|index| {
                format!("/ip4/203.0.113.7/tcp/{}", 4000 + u32::try_from(index).expect("index"))
                    .parse()
                    .expect("address")
            })
            .collect();
        assert!(matches!(
            invalid.validate(),
            Err(NetError::InvalidConfig("external_addresses"))
        ));
    }
}
