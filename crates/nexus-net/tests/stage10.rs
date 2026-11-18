//! Stage 10 loopback integration tests: bootstrap entry points, DHT records,
//! provider discovery, and churn.
//!
//! Every node listens on loopback only. Bootstrap peers are ordinary
//! authenticated peers; no test depends on a central service.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use nexus_crypto::{IdentityId, KeyPair};
use nexus_identity::LocalEncryptionKey;
use nexus_net::{DhtMode, NetError, Network, NetworkConfig};
use nexus_protocol::{DhtRecord, ProfileV1, SignedProfile};

static COUNTER: AtomicU64 = AtomicU64::new(0);
const LEK: [u8; 32] = [10_u8; 32];

fn temp_root(name: &str) -> PathBuf {
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("nexus-net-stage10-{name}-{}-{counter}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("root");
    path
}

fn config(root: &Path, server: bool) -> NetworkConfig {
    let mut config = NetworkConfig::new(root);
    config.listen = vec!["/ip4/127.0.0.1/tcp/0".parse().expect("tcp listen")];
    config.connect_timeout = Duration::from_secs(5);
    config.request_timeout = Duration::from_secs(5);
    config.dht_query_timeout = Duration::from_secs(5);
    config.reconnect_base_delay = Duration::from_millis(250);
    config.reconnect_max_delay = Duration::from_secs(2);
    config.dht_mode = if server { DhtMode::Server } else { DhtMode::Client };
    config
}

fn local_key() -> LocalEncryptionKey {
    LocalEncryptionKey::new(LEK)
}

fn start(root: &Path, server: bool) -> Network {
    Network::start(config(root, server), &local_key(), Arc::new(())).expect("start network")
}

fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if condition() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("timed out waiting for {what}");
}

fn listen_address(network: &Network) -> String {
    let mut found = None;
    wait_until("listen address", || {
        found = network
            .status()
            .listen_addresses
            .into_iter()
            .find(|address| address.contains("/tcp/"));
        found.is_some()
    });
    found.expect("listen address")
}

fn dial_address(network: &Network) -> String {
    let address = listen_address(network);
    if address.ends_with(&format!("/p2p/{}", network.peer_id())) {
        address
    } else {
        format!("{address}/p2p/{}", network.peer_id())
    }
}

fn signed_profile(seed: u8) -> DhtRecord {
    let keypair = KeyPair::from_seed([seed; 32]);
    let identity_id = IdentityId::from_public_key(&keypair.public_key());
    let profile = ProfileV1::new(
        identity_id,
        "Stage Ten",
        "signed DHT record",
        1,
        1_700_000_000_000 + u64::from(seed),
    );
    let signed = SignedProfile::sign(&profile, &keypair).expect("sign profile");
    DhtRecord::parse(signed.raw()).expect("parse record")
}

fn wait_for_record(network: &Network, key: &[u8; 32]) -> nexus_net::DhtValue {
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut last_error = None;
    while Instant::now() < deadline {
        match network.lookup_record(key) {
            Ok(Some(value)) => return value,
            Ok(None) => {}
            Err(error) => last_error = Some(error),
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("timed out waiting for a DHT record: {last_error:?}");
}

fn wait_for_provider(network: &Network, content_id: &[u8; 32], expected: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        match network.find_providers(content_id) {
            Ok(providers) => {
                if providers.iter().any(|peer| peer.peer_id == expected) {
                    return;
                }
            }
            Err(NetError::Timeout) => {}
            Err(error) => panic!("provider lookup failed: {error:?}"),
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("timed out waiting for provider {expected}");
}

#[test]
fn fresh_node_joins_and_exchanges_dht_records() {
    let root_a = temp_root("join-a");
    let root_b = temp_root("join-b");
    let root_c = temp_root("join-c");

    let b = start(&root_b, true);
    let address_b = dial_address(&b);

    let mut c_config = config(&root_c, true);
    c_config.bootstrap = vec![address_b.parse().expect("bootstrap b")];
    let c = Network::start(c_config, &local_key(), Arc::new(())).expect("start c");
    wait_until("c joins b", || c.status().dht_routing_peers >= 1);
    let address_c = dial_address(&c);

    let mut a_config = config(&root_a, false);
    a_config.bootstrap = vec![
        address_b.parse().expect("bootstrap b"),
        address_c.parse().expect("bootstrap c"),
    ];
    let a = Network::start(a_config, &local_key(), Arc::new(())).expect("start a");

    wait_until("a joins the DHT", || {
        let status = a.status();
        status.dht_routing_peers >= 1 && status.dht_bootstrapped
    });

    let discovery = a.discover_peers(None).expect("discovery");
    assert!(
        !discovery.peers.is_empty(),
        "a fresh node must discover peers through a bootstrap path"
    );
    assert!(
        discovery
            .peers
            .iter()
            .any(|peer| peer.peer_id == b.peer_id() || peer.peer_id == c.peer_id()),
        "discovery must include a bootstrap peer: {discovery:?}"
    );
    assert!(discovery.routing_peers >= 1);

    let record = signed_profile(11);
    b.publish_record(record.key(), record.raw()).expect("publish profile");
    let value = wait_for_record(&a, record.key());
    assert_eq!(value.key, record.key().to_vec());
    assert_eq!(value.value, record.raw());

    let content_id = [42_u8; 32];
    b.start_providing(&content_id).expect("start providing");
    wait_for_provider(&a, &content_id, b.peer_id());

    assert!(matches!(
        a.publish_record(&[0_u8; 32], b"not a signed record"),
        Err(NetError::InvalidRecord)
    ));
    assert!(matches!(
        a.publish_record(&[0_u8; 31], record.raw()),
        Err(NetError::InvalidKey)
    ));
    assert!(matches!(a.start_providing(&[0_u8; 31]), Err(NetError::InvalidKey)));

    let status = a.status();
    assert_eq!(status.dht_mode, "client");
    wait_until("provider announcements are validated", || {
        let status_b = b.status();
        let status_c = c.status();
        status_b.dht_providers_accepted
            + status_c.dht_providers_accepted
            + status_b.dht_providers_rejected
            + status_c.dht_providers_rejected
            >= 1
    });

    a.shutdown();
    b.shutdown();
    c.shutdown();
}

#[test]
fn losing_one_bootstrap_peer_keeps_the_network_reachable() {
    let root_a = temp_root("loss-a");
    let root_b = temp_root("loss-b");
    let root_c = temp_root("loss-c");

    let b = start(&root_b, true);
    let address_b = dial_address(&b);

    let mut c_config = config(&root_c, true);
    c_config.bootstrap = vec![address_b.parse().expect("bootstrap b")];
    let c = Network::start(c_config, &local_key(), Arc::new(())).expect("start c");
    wait_until("c joins b", || c.status().dht_routing_peers >= 1);
    let address_c = dial_address(&c);

    let mut a_config = config(&root_a, false);
    a_config.bootstrap = vec![
        address_b.parse().expect("bootstrap b"),
        address_c.parse().expect("bootstrap c"),
    ];
    let a = Network::start(a_config, &local_key(), Arc::new(())).expect("start a");
    wait_until("a joins both bootstrap peers", || a.status().dht_routing_peers >= 2);

    // Entry points are not authorities: losing one leaves the other in place.
    b.shutdown();

    let deadline = Instant::now() + Duration::from_secs(30);
    let mut last = None;
    while Instant::now() < deadline {
        match a.discover_peers(None) {
            Ok(discovery) if !discovery.peers.is_empty() => {
                assert!(
                    discovery.peers.iter().any(|peer| peer.peer_id == c.peer_id()),
                    "the surviving bootstrap peer must answer discovery: {discovery:?}"
                );
                a.shutdown();
                c.shutdown();
                return;
            }
            Ok(discovery) => last = Some(format!("empty discovery: {discovery:?}")),
            Err(error) => last = Some(format!("discovery failed: {error:?}")),
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    panic!("discovery did not survive one bootstrap loss: {last:?}");
}

#[test]
fn failed_publish_is_counted_once() {
    let root = temp_root("publish-failure");
    let network = start(&root, false);
    let record = signed_profile(12);

    // With no bootstrap peer and an empty routing table, the put-record query
    // completes immediately with a quorum failure.
    assert!(matches!(
        network.publish_record(record.key(), record.raw()),
        Err(NetError::Rejected)
    ));

    let status = network.status();
    assert_eq!(status.dht_publish_failures, 1);
    assert_eq!(status.dht_publish_successes, 0);
    assert_eq!(status.dht_queries_failed, 1);

    network.shutdown();
}

#[test]
fn node_churn_does_not_disable_discovery() {
    let root_c = temp_root("churn-c");
    let c = start(&root_c, true);
    let address_c = dial_address(&c);

    for cycle in 0..3 {
        let root = temp_root(&format!("churn-{cycle}"));
        let mut config = config(&root, false);
        config.bootstrap = vec![address_c.parse().expect("bootstrap c")];
        let node = Network::start(config, &local_key(), Arc::new(())).expect("start churn node");
        wait_until("churn node joins", || node.status().dht_routing_peers >= 1);
        let discovery = node.discover_peers(None).expect("churn discovery");
        assert!(!discovery.peers.is_empty(), "cycle {cycle}");
        node.shutdown();
    }

    wait_until("server stays online", || c.status().dht_routing_peers >= 1);
    let root_a = temp_root("churn-a");
    let mut a_config = config(&root_a, false);
    a_config.bootstrap = vec![address_c.parse().expect("bootstrap c")];
    let a = Network::start(a_config, &local_key(), Arc::new(())).expect("start a");
    wait_until("a joins after churn", || a.status().dht_routing_peers >= 1);
    let discovery = a.discover_peers(None).expect("discovery after churn");
    assert!(!discovery.peers.is_empty());

    a.shutdown();
    c.shutdown();
}
