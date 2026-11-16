//! Composed libp2p behaviour: connection limits, identify, ping, and the
//! bounded transport-check request/response protocol.

use std::time::Duration;

use libp2p::identity::Keypair;
use libp2p::swarm::NetworkBehaviour;
use libp2p::{connection_limits, identify, ping, request_response};

use crate::codec::{TestCodec, test_protocol};
use crate::config::NetworkConfig;

/// Protocol version advertised through identify.
pub const IDENTIFY_PROTOCOL_VERSION: &str = "/nexus/0.1.0-stage-09";
/// Agent string advertised through identify.
pub const IDENTIFY_AGENT_VERSION: &str = "ember/0.1.0";
/// Identify exchange interval.
pub const IDENTIFY_INTERVAL: Duration = Duration::from_secs(300);
/// Ping interval once behaviour health checks are established.
pub const PING_INTERVAL: Duration = Duration::from_secs(20);
/// Ping timeout for one round trip.
pub const PING_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(NetworkBehaviour)]
pub(crate) struct Behaviour {
    pub(crate) limits: connection_limits::Behaviour,
    pub(crate) identify: identify::Behaviour,
    pub(crate) ping: ping::Behaviour,
    pub(crate) test: request_response::Behaviour<TestCodec>,
}

impl Behaviour {
    pub(crate) fn new(key: &Keypair, config: &NetworkConfig) -> Self {
        let limits = connection_limits::Behaviour::new(
            connection_limits::ConnectionLimits::default()
                .with_max_established(Some(config.max_established))
                .with_max_established_per_peer(Some(config.max_established_per_peer))
                .with_max_pending_incoming(Some(config.max_pending_incoming)),
        );

        let identify = identify::Behaviour::new(
            identify::Config::new(IDENTIFY_PROTOCOL_VERSION.to_owned(), key.public())
                .with_agent_version(IDENTIFY_AGENT_VERSION.to_owned())
                .with_interval(IDENTIFY_INTERVAL)
                .with_push_listen_addr_updates(true),
        );

        let ping = ping::Behaviour::new(
            ping::Config::default()
                .with_interval(PING_INTERVAL)
                .with_timeout(PING_TIMEOUT),
        );

        let test = request_response::Behaviour::with_codec(
            TestCodec,
            [(test_protocol(), request_response::ProtocolSupport::Full)],
            request_response::Config::default()
                .with_request_timeout(config.request_timeout)
                .with_max_concurrent_streams(config.max_concurrent_streams),
        );

        Self {
            limits,
            identify,
            ping,
            test,
        }
    }
}
