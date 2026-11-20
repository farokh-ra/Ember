//! Typed networking failures with stable, redacted codes.

use std::io;

/// Failure returned by the networking layer.
#[derive(Debug, thiserror::Error)]
pub enum NetError {
    /// Networking is disabled by configuration.
    #[error("networking is disabled")]
    Disabled,
    /// A configuration value is out of range.
    #[error("network configuration is invalid")]
    InvalidConfig(&'static str),
    /// A multiaddress could not be parsed.
    #[error("the multiaddress is invalid")]
    InvalidAddress,
    /// A multiaddress did not end with `/p2p/<peer-id>`.
    #[error("the multiaddress must include a /p2p peer ID")]
    MissingPeerId,
    /// A peer ID could not be parsed.
    #[error("the peer ID is invalid")]
    InvalidPeerId,
    /// The peer key file could not be created or read.
    #[error("the peer key file is unavailable")]
    PeerKeyUnavailable(#[source] io::Error),
    /// The peer key file was corrupt or used an unsupported format.
    #[error("the peer key file is invalid")]
    PeerKeyInvalid,
    /// The peer key could not be unlocked with the supplied local secret.
    #[error("the peer key could not be unlocked")]
    PeerKeyLocked,
    /// The background network driver is not running.
    #[error("the network driver is not running")]
    NotRunning,
    /// The bounded command queue is full.
    #[error("the network driver is busy")]
    Busy,
    /// No listener, bootstrap peer, or path is available.
    #[error("no network path is available")]
    Unavailable,
    /// A bounded deadline expired.
    #[error("the network operation timed out")]
    Timeout,
    /// The connection was refused, reset, or closed.
    #[error("the connection failed")]
    ConnectionFailed,
    /// The authenticated remote identity did not match the requested peer ID.
    #[error("the remote peer identity did not match the request")]
    WrongPeerId,
    /// The remote peer does not implement the requested protocol.
    #[error("the remote peer does not support this protocol")]
    UnsupportedProtocol,
    /// The peer is not known to this node.
    #[error("the peer is not known")]
    PeerNotFound,
    /// The peer is not currently connected.
    #[error("the peer is not connected")]
    NotConnected,
    /// No configured relay is connected or reachable.
    #[error("no relay is available")]
    RelayUnavailable,
    /// A relay reservation was denied or timed out.
    #[error("the relay reservation failed")]
    RelayReservationFailed,
    /// A relayed connection could not be upgraded to a direct path.
    #[error("the hole punch failed")]
    HolePunchFailed,
    /// Direct and relayed attempts both failed.
    #[error("no direct or relay path is available")]
    NoViablePath,
    /// A transport-check message exceeded its size limit.
    #[error("the message exceeds the transport-check limit")]
    MessageTooLarge,
    /// A DHT value was not a valid signed publication, profile, or tombstone.
    #[error("the DHT record is not a valid signed record")]
    InvalidRecord,
    /// A DHT key or provider content ID was the wrong length.
    #[error("the DHT key is invalid")]
    InvalidKey,
    /// A DHT value exceeded the signed-envelope cap.
    #[error("the DHT record exceeds its size limit")]
    RecordTooLarge,
    /// The remote peer rejected the request.
    #[error("the remote peer rejected the request")]
    Rejected,
    /// The remote peer does not have the requested verified content.
    #[error("the remote peer does not have the requested content")]
    FileUnavailable,
    /// A local socket or file operation failed.
    #[error("local network I/O failed")]
    Io(#[source] io::Error),
    /// A cryptographic primitive failed while handling the peer key.
    #[error("peer key cryptography failed")]
    Crypto(#[source] nexus_crypto::CryptoError),
    /// The background driver could not build or start the swarm.
    #[error("the network driver failed to start")]
    DriverFailed,
}

impl NetError {
    /// Stable wire code used in logs and diagnostics.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::InvalidConfig(_) => "invalid_config",
            Self::InvalidAddress => "invalid_address",
            Self::MissingPeerId => "missing_peer_id",
            Self::InvalidPeerId => "invalid_peer_id",
            Self::PeerKeyUnavailable(_) => "peer_key_unavailable",
            Self::PeerKeyInvalid => "peer_key_invalid",
            Self::PeerKeyLocked => "peer_key_locked",
            Self::NotRunning => "not_running",
            Self::Busy => "busy",
            Self::Unavailable => "network_unavailable",
            Self::Timeout => "timeout",
            Self::ConnectionFailed => "connection_failed",
            Self::WrongPeerId => "wrong_peer_id",
            Self::UnsupportedProtocol => "unsupported_protocol",
            Self::PeerNotFound => "not_found",
            Self::NotConnected => "not_connected",
            Self::RelayUnavailable => "relay_unavailable",
            Self::RelayReservationFailed => "relay_reservation_failed",
            Self::HolePunchFailed => "hole_punch_failed",
            Self::NoViablePath => "no_viable_path",
            Self::MessageTooLarge => "message_too_large",
            Self::InvalidRecord => "invalid_record",
            Self::InvalidKey => "invalid_key",
            Self::RecordTooLarge => "record_too_large",
            Self::Rejected => "rejected",
            Self::FileUnavailable => "content_unavailable",
            Self::Io(_) => "io_error",
            Self::Crypto(_) => "crypto_error",
            Self::DriverFailed => "driver_failed",
        }
    }

    /// Whether an immediate retry can reasonably succeed.
    #[must_use]
    pub const fn retryable(&self) -> bool {
        matches!(
            self,
            Self::Busy
                | Self::Unavailable
                | Self::Timeout
                | Self::ConnectionFailed
                | Self::RelayUnavailable
                | Self::RelayReservationFailed
                | Self::NoViablePath
                | Self::NotRunning
        )
    }
}

impl From<io::Error> for NetError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<nexus_crypto::CryptoError> for NetError {
    fn from(error: nexus_crypto::CryptoError) -> Self {
        Self::Crypto(error)
    }
}
