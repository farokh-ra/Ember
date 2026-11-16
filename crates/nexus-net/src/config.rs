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

/// Highest accepted value for any connection limit.
pub const MAX_CONNECTION_LIMIT: u32 = 512;
/// Highest accepted bootstrap/manual peer count.
pub const MAX_BOOTSTRAP_PEERS: usize = 64;
/// Highest accepted listen-address count.
pub const MAX_LISTEN_ADDRESSES: usize = 8;

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
}

impl NetworkConfig {
    /// Creates a configuration with the documented defaults.
    #[must_use]
    pub fn new(data_root: impl Into<PathBuf>) -> Self {
        Self {
            data_root: data_root.into(),
            listen: Vec::new(),
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
        if self.bootstrap.len() > MAX_BOOTSTRAP_PEERS {
            return Err(NetError::InvalidConfig("bootstrap"));
        }
        for address in &self.listen {
            require_peer_id(address, false)?;
        }
        for address in &self.bootstrap {
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

    use super::{NetworkConfig, QUIC_LISTEN_ADDRESS, TCP_LISTEN_ADDRESS, require_peer_id};
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
}
