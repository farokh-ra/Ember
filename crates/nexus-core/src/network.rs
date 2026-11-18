//! Mapping between the networking layer and the core's command/event bus.
//!
//! `nexus-net` reports state and peer transitions through a non-blocking
//! observer. This module turns those reports into durable, bounded core events
//! and renders status snapshots as canonical CBOR values. It also maps typed
//! networking failures to stable domain errors so callers never see socket or
//! key material details.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use nexus_crypto::Value;
use nexus_net::{ConnectedPeer, NetError, NetworkEvent, NetworkObserver, NetworkStatus, PeerSnapshot, TestReply};

use crate::error::{DomainCode, DomainError};
use crate::event::{EventQueue, PendingEvent};

/// Bridges [`NetworkEvent`] reports onto the core event queue.
pub(crate) struct CoreNetworkObserver {
    events: Arc<EventQueue>,
    dropped: Arc<AtomicU64>,
}

impl CoreNetworkObserver {
    pub(crate) fn new(events: Arc<EventQueue>, dropped: Arc<AtomicU64>) -> Self {
        Self { events, dropped }
    }
}

impl NetworkObserver for CoreNetworkObserver {
    fn on_event(&self, event: NetworkEvent) {
        let pending = match event {
            NetworkEvent::State {
                state,
                peer_count,
                transports,
                reason,
            } => {
                let mut entries = vec![
                    (Value::Text("state".to_owned()), Value::Text(state.as_str().to_owned())),
                    (
                        Value::Text("peer_count".to_owned()),
                        Value::Unsigned(u64::try_from(peer_count).unwrap_or(u64::MAX)),
                    ),
                    (
                        Value::Text("transports".to_owned()),
                        Value::Array(transports.into_iter().map(Value::Text).collect()),
                    ),
                ];
                if let Some(reason) = reason {
                    entries.push((
                        Value::Text("reason".to_owned()),
                        Value::Text(nexus_observability::text(&reason)),
                    ));
                }
                PendingEvent::durable("network.state", Value::Map(entries))
            }
            NetworkEvent::Peer {
                peer_id,
                event,
                direction,
                transport,
                reason,
            } => {
                let mut entries = vec![
                    (Value::Text("peer_id".to_owned()), Value::Text(peer_id)),
                    (Value::Text("event".to_owned()), Value::Text(event.as_str().to_owned())),
                ];
                if let Some(direction) = direction {
                    entries.push((Value::Text("direction".to_owned()), Value::Text(direction)));
                }
                if let Some(transport) = transport {
                    entries.push((Value::Text("transport".to_owned()), Value::Text(transport)));
                }
                if let Some(reason) = reason {
                    entries.push((
                        Value::Text("reason".to_owned()),
                        Value::Text(nexus_observability::text(&reason)),
                    ));
                }
                PendingEvent::durable("network.peer_event", Value::Map(entries))
            }
        };
        if self.events.try_push(pending).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Renders the aggregate status returned by `network.status`.
pub(crate) fn status_value(status: &NetworkStatus) -> Value {
    Value::Map(vec![
        (
            Value::Text("state".to_owned()),
            Value::Text(status.state.as_str().to_owned()),
        ),
        (Value::Text("peer_id".to_owned()), Value::Text(status.peer_id.clone())),
        (
            Value::Text("transports".to_owned()),
            Value::Array(status.transports.iter().cloned().map(Value::Text).collect()),
        ),
        (
            Value::Text("listen_addresses".to_owned()),
            Value::Array(status.listen_addresses.iter().cloned().map(Value::Text).collect()),
        ),
        (
            Value::Text("observed_addresses".to_owned()),
            Value::Array(status.observed_addresses.iter().cloned().map(Value::Text).collect()),
        ),
        (
            Value::Text("confirmed_addresses".to_owned()),
            Value::Array(status.confirmed_addresses.iter().cloned().map(Value::Text).collect()),
        ),
        (
            Value::Text("peer_count".to_owned()),
            Value::Unsigned(u64::try_from(status.peer_count).unwrap_or(u64::MAX)),
        ),
        (
            Value::Text("connection_count".to_owned()),
            Value::Unsigned(u64::try_from(status.connection_count).unwrap_or(u64::MAX)),
        ),
        (
            Value::Text("bootstrap_configured".to_owned()),
            Value::Unsigned(u64::try_from(status.bootstrap_configured).unwrap_or(u64::MAX)),
        ),
        (
            Value::Text("bootstrap_connected".to_owned()),
            Value::Unsigned(u64::try_from(status.bootstrap_connected).unwrap_or(u64::MAX)),
        ),
        (
            Value::Text("dial_attempts".to_owned()),
            Value::Unsigned(status.dial_attempts),
        ),
        (
            Value::Text("dial_failures".to_owned()),
            Value::Unsigned(status.dial_failures),
        ),
        (
            Value::Text("requests_served".to_owned()),
            Value::Unsigned(status.requests_served),
        ),
        (
            Value::Text("requests_rejected".to_owned()),
            Value::Unsigned(status.requests_rejected),
        ),
        (Value::Text("dht_mode".to_owned()), Value::Text(status.dht_mode.clone())),
        (
            Value::Text("dht_routing_peers".to_owned()),
            Value::Unsigned(u64::try_from(status.dht_routing_peers).unwrap_or(u64::MAX)),
        ),
        (
            Value::Text("dht_bootstrapped".to_owned()),
            Value::Bool(status.dht_bootstrapped),
        ),
        (
            Value::Text("dht_queries_started".to_owned()),
            Value::Unsigned(status.dht_queries_started),
        ),
        (
            Value::Text("dht_queries_failed".to_owned()),
            Value::Unsigned(status.dht_queries_failed),
        ),
        (
            Value::Text("dht_records_accepted".to_owned()),
            Value::Unsigned(status.dht_records_accepted),
        ),
        (
            Value::Text("dht_records_rejected".to_owned()),
            Value::Unsigned(status.dht_records_rejected),
        ),
        (
            Value::Text("dht_providers_accepted".to_owned()),
            Value::Unsigned(status.dht_providers_accepted),
        ),
        (
            Value::Text("dht_providers_rejected".to_owned()),
            Value::Unsigned(status.dht_providers_rejected),
        ),
        (
            Value::Text("dht_publish_successes".to_owned()),
            Value::Unsigned(status.dht_publish_successes),
        ),
        (
            Value::Text("dht_publish_failures".to_owned()),
            Value::Unsigned(status.dht_publish_failures),
        ),
        (
            Value::Text("last_error".to_owned()),
            status
                .last_error
                .as_ref()
                .map_or(Value::Null, |code| Value::Text(nexus_observability::text(code))),
        ),
    ])
}

/// Renders one observed peer for `network.peers`.
pub(crate) fn peer_value(peer: &PeerSnapshot) -> Value {
    Value::Map(vec![
        (Value::Text("peer_id".to_owned()), Value::Text(peer.peer_id.clone())),
        (
            Value::Text("state".to_owned()),
            Value::Text(peer.state.as_str().to_owned()),
        ),
        (
            Value::Text("direction".to_owned()),
            optional_text(peer.direction.as_deref()),
        ),
        (
            Value::Text("transport".to_owned()),
            optional_text(peer.transport.as_deref()),
        ),
        (
            Value::Text("remote_address".to_owned()),
            optional_text(peer.remote_address.as_deref()),
        ),
        (
            Value::Text("last_seen_ms".to_owned()),
            Value::Unsigned(peer.last_seen_ms),
        ),
        (
            Value::Text("rtt_ms".to_owned()),
            peer.rtt_ms.map_or(Value::Null, Value::Unsigned),
        ),
        (
            Value::Text("protocols".to_owned()),
            Value::Array(peer.protocols.iter().cloned().map(Value::Text).collect()),
        ),
        (
            Value::Text("agent_version".to_owned()),
            optional_text(peer.agent_version.as_deref()),
        ),
        (
            Value::Text("connections".to_owned()),
            Value::Unsigned(u64::from(peer.connections)),
        ),
        (Value::Text("error".to_owned()), optional_text(peer.error.as_deref())),
    ])
}

/// Renders the result of a direct connection request.
pub(crate) fn connected_peer_value(peer: &ConnectedPeer, multiaddr: &str) -> Value {
    Value::Map(vec![
        (Value::Text("peer_id".to_owned()), Value::Text(peer.peer_id.clone())),
        (Value::Text("transport".to_owned()), Value::Text(peer.transport.clone())),
        (
            Value::Text("remote_address".to_owned()),
            Value::Text(peer.remote_address.clone()),
        ),
        (
            Value::Text("established_ms".to_owned()),
            Value::Unsigned(peer.established_ms),
        ),
        (Value::Text("multiaddr".to_owned()), Value::Text(multiaddr.to_owned())),
    ])
}

/// Renders the result of a transport-check exchange.
pub(crate) fn test_reply_value(reply: &TestReply) -> Value {
    Value::Map(vec![
        (Value::Text("peer_id".to_owned()), Value::Text(reply.peer_id.clone())),
        (Value::Text("payload".to_owned()), Value::Bytes(reply.payload.clone())),
        (
            Value::Text("round_trip_ms".to_owned()),
            Value::Unsigned(reply.round_trip_ms),
        ),
    ])
}

/// Maps a typed networking failure to a redacted domain error.
pub(crate) fn domain_error(error: &NetError) -> DomainError {
    let (code, message) = match error {
        NetError::Disabled => (DomainCode::NetworkUnavailable, "Networking is disabled."),
        NetError::InvalidConfig(_) => (DomainCode::InvalidArgument, "The network configuration is invalid."),
        NetError::InvalidAddress => (DomainCode::InvalidArgument, "The network address is invalid."),
        NetError::MissingPeerId => (
            DomainCode::InvalidArgument,
            "The network address must include a /p2p peer ID.",
        ),
        NetError::InvalidPeerId => (DomainCode::InvalidArgument, "The peer ID is invalid."),
        NetError::MessageTooLarge => (
            DomainCode::InvalidArgument,
            "The transport-check message exceeds its size limit.",
        ),
        NetError::PeerKeyLocked => (DomainCode::Locked, "The device network key could not be unlocked."),
        NetError::PeerKeyUnavailable(_) | NetError::PeerKeyInvalid | NetError::Crypto(_) => {
            (DomainCode::Internal, "The device network key could not be read.")
        }
        NetError::NotRunning | NetError::Busy | NetError::Unavailable => {
            (DomainCode::NetworkUnavailable, "Networking is not available.")
        }
        NetError::Timeout => (DomainCode::Timeout, "The network operation timed out."),
        NetError::ConnectionFailed => (DomainCode::NetworkUnavailable, "The peer connection failed."),
        NetError::WrongPeerId => (
            DomainCode::VerificationFailed,
            "The remote peer identity did not match the request.",
        ),
        NetError::UnsupportedProtocol => (
            DomainCode::UnsupportedVersion,
            "The remote peer does not support this protocol.",
        ),
        NetError::PeerNotFound => (DomainCode::NotFound, "The peer is not known."),
        NetError::NotConnected => (DomainCode::NetworkUnavailable, "The peer is not connected."),
        NetError::InvalidRecord => (
            DomainCode::VerificationFailed,
            "The DHT record is not a valid signed record.",
        ),
        NetError::InvalidKey => (DomainCode::InvalidArgument, "The DHT key is invalid."),
        NetError::RecordTooLarge => (DomainCode::InvalidArgument, "The DHT record exceeds its size limit."),
        NetError::Rejected => (DomainCode::NetworkUnavailable, "The remote peer rejected the request."),
        NetError::Io(_) => (DomainCode::IoError, "A local network operation failed."),
        NetError::DriverFailed => (DomainCode::NetworkUnavailable, "The network driver could not start."),
    };
    DomainError::new(code, message).with_details(Value::Map(vec![(
        Value::Text("net_code".to_owned()),
        Value::Text(error.code().to_owned()),
    )]))
}

fn optional_text(value: Option<&str>) -> Value {
    value.map_or(Value::Null, |text| Value::Text(nexus_observability::text(text)))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    use nexus_crypto::Value;
    use nexus_net::{NetError, NetworkEvent, NetworkObserver, NetworkState, NetworkStatus};

    use super::{CoreNetworkObserver, domain_error, status_value};
    use crate::error::DomainCode;
    use crate::event::EventQueue;

    #[test]
    fn state_reports_are_durable_core_events() {
        let events = Arc::new(EventQueue::new());
        let dropped = Arc::new(AtomicU64::new(0));
        let observer = CoreNetworkObserver::new(Arc::clone(&events), dropped);
        observer.on_event(NetworkEvent::State {
            state: NetworkState::Degraded,
            peer_count: 0,
            transports: vec!["quic".to_owned(), "tcp".to_owned()],
            reason: Some("no_peers".to_owned()),
        });

        let event = events
            .next_event(std::time::Duration::from_millis(10))
            .expect("event")
            .expect("queued");
        assert_eq!(event.event_type, "network.state");
        assert_eq!(
            event
                .data
                .as_map()
                .expect("map")
                .iter()
                .find(|(key, _)| key.as_text() == Some("state"))
                .and_then(|(_, value)| value.as_text()),
            Some("degraded")
        );
    }

    #[test]
    fn status_values_never_claim_confirmed_reachability() {
        let status = NetworkStatus::disabled();
        let value = status_value(&status);
        let entries = value.as_map().expect("map");
        let get = |name: &str| {
            entries
                .iter()
                .find(|(key, _)| key.as_text() == Some(name))
                .map(|(_, value)| value)
        };
        assert_eq!(get("state").and_then(Value::as_text), Some("disabled"));
        assert_eq!(get("peer_count").and_then(Value::as_unsigned), Some(0));
        match get("confirmed_addresses") {
            Some(Value::Array(items)) => assert!(items.is_empty()),
            other => panic!("confirmed_addresses: {other:?}"),
        }
    }

    #[test]
    fn net_failures_map_to_stable_domain_codes() {
        assert_eq!(
            domain_error(&NetError::WrongPeerId).code,
            DomainCode::VerificationFailed
        );
        assert_eq!(domain_error(&NetError::Timeout).code, DomainCode::Timeout);
        assert_eq!(domain_error(&NetError::PeerNotFound).code, DomainCode::NotFound);
        assert_eq!(domain_error(&NetError::Disabled).code, DomainCode::NetworkUnavailable);
        assert_eq!(
            domain_error(&NetError::InvalidAddress).code,
            DomainCode::InvalidArgument
        );
        assert_eq!(domain_error(&NetError::PeerKeyLocked).code, DomainCode::Locked);
    }

    #[test]
    fn observer_counts_dropped_events_without_blocking() {
        let events = Arc::new(EventQueue::new());
        events.close();
        let dropped = Arc::new(AtomicU64::new(0));
        let observer = CoreNetworkObserver::new(Arc::clone(&events), Arc::clone(&dropped));
        observer.on_event(NetworkEvent::State {
            state: NetworkState::Online,
            peer_count: 1,
            transports: vec!["quic".to_owned()],
            reason: None,
        });
        assert_eq!(dropped.load(Ordering::Relaxed), 1);
    }
}
