//! Composed libp2p behaviour: connection limits, identify, ping, Kademlia, and
//! the bounded transport-check request/response protocol.

use std::num::NonZeroUsize;
use std::time::Duration;

use libp2p::identity::Keypair;
use libp2p::kad::store::{MemoryStore, MemoryStoreConfig};
use libp2p::swarm::behaviour::toggle::Toggle;
use libp2p::swarm::{NetworkBehaviour, StreamProtocol};
use libp2p::{Multiaddr, autonat, connection_limits, dcutr, identify, kad, ping, relay, request_response};
use nexus_protocol::DHT_MAX_VALUE_BYTES;

use crate::codec::{FileCodec, IndexCodec, TestCodec, file_protocol, index_protocol, test_protocol};
use crate::config::{DhtMode, NetworkConfig};

/// Protocol version advertised through identify.
pub const IDENTIFY_PROTOCOL_VERSION: &str = "/nexus/0.1.0-stage-12";
/// Agent string advertised through identify.
pub const IDENTIFY_AGENT_VERSION: &str = "ember/0.1.0";
/// Identify exchange interval.
pub const IDENTIFY_INTERVAL: Duration = Duration::from_secs(300);
/// Ping interval once behaviour health checks are established.
pub const PING_INTERVAL: Duration = Duration::from_secs(20);
/// Ping timeout for one round trip.
pub const PING_TIMEOUT: Duration = Duration::from_secs(10);
/// Kademlia protocol ID. It is distinct from the Stage 09 transport-check
/// protocol and from the public IPFS DHT.
pub const KAD_PROTOCOL_NAME: &str = "/nexus/kad/1.0.0";
/// Replication factor used for publication and provider queries.
pub const KAD_REPLICATION_FACTOR: usize = 8;
/// Concurrent requests per Kademlia query.
pub const KAD_QUERY_PARALLELISM: usize = 3;
/// Kademlia frame cap. It leaves headroom over the largest signed envelope for
/// protocol framing and provider metadata.
pub const KAD_MAX_PACKET_BYTES: usize = 80 * 1024;
/// Rebootstrap interval for a node that stays online.
pub const KAD_BOOTSTRAP_INTERVAL: Duration = Duration::from_secs(5 * 60);
/// How long AutoNAT waits for one dial-back probe.
pub const AUTONAT_PROBE_TIMEOUT: Duration = Duration::from_secs(15);
/// Delay before the first AutoNAT probe.
pub const AUTONAT_BOOT_DELAY: Duration = Duration::from_secs(5);
/// Retry interval while AutoNAT reachability is unknown.
pub const AUTONAT_RETRY_INTERVAL: Duration = Duration::from_secs(60);
/// Refresh interval once AutoNAT reachability is established.
pub const AUTONAT_REFRESH_INTERVAL: Duration = Duration::from_secs(10 * 60);

#[derive(NetworkBehaviour)]
pub(crate) struct Behaviour {
    pub(crate) limits: connection_limits::Behaviour,
    pub(crate) identify: identify::Behaviour,
    pub(crate) ping: ping::Behaviour,
    pub(crate) kademlia: kad::Behaviour<MemoryStore>,
    pub(crate) relay_server: relay::Behaviour,
    pub(crate) relay_client: relay::client::Behaviour,
    pub(crate) dcutr: Toggle<dcutr::Behaviour>,
    pub(crate) autonat: Toggle<autonat::v1::Behaviour>,
    pub(crate) test: request_response::Behaviour<TestCodec>,
    pub(crate) file: request_response::Behaviour<FileCodec>,
    pub(crate) index: request_response::Behaviour<IndexCodec>,
}

impl Behaviour {
    pub(crate) fn new(key: &Keypair, config: &NetworkConfig, relay_client: relay::client::Behaviour) -> Self {
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

        let local_peer_id = key.public().to_peer_id();
        let store = MemoryStore::with_config(
            local_peer_id,
            MemoryStoreConfig {
                max_records: config.dht_store_max_records,
                // `MemoryStore` rejects values whose length is greater than or
                // equal to this field, so keep one byte of headroom over the
                // signed-envelope cap.
                max_value_bytes: DHT_MAX_VALUE_BYTES + 1,
                max_providers_per_key: config.dht_store_max_providers,
                max_provided_keys: config.dht_store_max_provided_keys,
            },
        );
        let mut kademlia_config = kad::Config::new(StreamProtocol::new(KAD_PROTOCOL_NAME));
        kademlia_config
            .set_query_timeout(config.dht_query_timeout)
            .set_record_filtering(kad::StoreInserts::FilterBoth)
            .set_record_ttl(Some(config.dht_record_ttl))
            .set_provider_record_ttl(Some(config.dht_record_ttl))
            .set_replication_factor(nonzero(KAD_REPLICATION_FACTOR))
            .set_parallelism(nonzero(KAD_QUERY_PARALLELISM))
            .set_replication_interval(Some(config.dht_republish_interval))
            .set_publication_interval(Some(config.dht_republish_interval))
            .set_provider_publication_interval(Some(config.dht_republish_interval))
            .set_periodic_bootstrap_interval(Some(KAD_BOOTSTRAP_INTERVAL))
            .set_max_packet_size(KAD_MAX_PACKET_BYTES);
        let mut kademlia = kad::Behaviour::with_config(local_peer_id, store, kademlia_config);
        kademlia.set_mode(Some(match config.dht_mode {
            DhtMode::Client => kad::Mode::Client,
            DhtMode::Server => kad::Mode::Server,
        }));

        let request_config = request_response::Config::default()
            .with_request_timeout(config.request_timeout)
            .with_max_concurrent_streams(config.max_concurrent_streams);
        let test = request_response::Behaviour::with_codec(
            TestCodec,
            [(test_protocol(), request_response::ProtocolSupport::Full)],
            request_config.clone(),
        );
        let file = request_response::Behaviour::with_codec(
            FileCodec,
            [(file_protocol(), request_response::ProtocolSupport::Full)],
            request_config.clone(),
        );
        let index = request_response::Behaviour::with_codec(
            IndexCodec,
            [(index_protocol(), request_response::ProtocolSupport::Full)],
            request_config,
        );

        let relay_config = relay::Config {
            max_reservations: config.relay_max_reservations,
            max_circuits: config.relay_max_circuits,
            max_circuits_per_peer: config.relay_max_circuits_per_peer,
            max_circuit_duration: config.relay_max_circuit_duration,
            max_circuit_bytes: config.relay_max_circuit_bytes,
            ..Default::default()
        };
        let mut relay_server = relay::Behaviour::new(local_peer_id, relay_config);
        relay_server.set_status(Some(if config.relay_server_enabled {
            relay::Status::Enable
        } else {
            relay::Status::Disable
        }));

        let dcutr = Toggle::from(config.dcutr_enabled.then(|| dcutr::Behaviour::new(local_peer_id)));

        let autonat = Toggle::from(config.autonat_enabled.then(|| {
            let autonat_config = autonat::v1::Config {
                timeout: AUTONAT_PROBE_TIMEOUT,
                boot_delay: AUTONAT_BOOT_DELAY,
                retry_interval: AUTONAT_RETRY_INTERVAL,
                refresh_interval: AUTONAT_REFRESH_INTERVAL,
                use_connected: true,
                ..Default::default()
            };
            let mut behaviour = autonat::v1::Behaviour::new(local_peer_id, autonat_config);
            for address in &config.relays {
                if let Ok((peer_id, plain)) = crate::config::split_peer_address(address) {
                    behaviour.add_server(peer_id, Some(strip_peer_suffix(plain)));
                }
            }
            behaviour
        }));

        Self {
            limits,
            identify,
            ping,
            kademlia,
            relay_server,
            relay_client,
            dcutr,
            autonat,
            test,
            file,
            index,
        }
    }
}

fn strip_peer_suffix(mut address: Multiaddr) -> Multiaddr {
    if matches!(address.iter().last(), Some(libp2p::multiaddr::Protocol::P2p(_))) {
        address.pop();
    }
    address
}

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or(NonZeroUsize::MIN)
}

#[cfg(test)]
mod tests {
    use super::{IDENTIFY_PROTOCOL_VERSION, KAD_PROTOCOL_NAME};

    #[test]
    fn protocol_names_are_versioned_and_private() {
        assert_eq!(IDENTIFY_PROTOCOL_VERSION, "/nexus/0.1.0-stage-12");
        assert_eq!(KAD_PROTOCOL_NAME, "/nexus/kad/1.0.0");
        assert!(!KAD_PROTOCOL_NAME.contains("ipfs"));
    }
}
