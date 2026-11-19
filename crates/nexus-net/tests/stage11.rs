//! Stage 11 loopback integration tests: relay reservations, direct fallback,
//! explicit no-path failures, and persisted community relay lists.
//!
//! Every node listens on loopback only. Relay servers are ordinary peers with
//! the relay server behaviour enabled; no test depends on a public service.

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use libp2p::PeerId;
use libp2p::identity::Keypair;
use nexus_identity::LocalEncryptionKey;
use nexus_net::{
    DEFAULT_RELAY_MAX_CIRCUIT_BYTES, DEFAULT_RELAY_MAX_CIRCUIT_DURATION, NETWORK_DIRECTORY, NetError, Network,
    NetworkConfig, RELAY_FILE_NAME,
};

static COUNTER: AtomicU64 = AtomicU64::new(0);
const LEK: [u8; 32] = [11_u8; 32];

fn temp_root(name: &str) -> PathBuf {
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("nexus-net-stage11-{name}-{}-{counter}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("root");
    path
}

fn config(root: &Path, relay_server: bool) -> NetworkConfig {
    let mut config = NetworkConfig::new(root);
    config.listen = vec!["/ip4/127.0.0.1/tcp/0".parse().expect("tcp listen")];
    config.connect_timeout = Duration::from_secs(3);
    config.request_timeout = Duration::from_secs(3);
    config.relay_reservation_timeout = Duration::from_secs(3);
    config.reconnect_base_delay = Duration::from_millis(250);
    config.reconnect_max_delay = Duration::from_secs(2);
    config.relay_server_enabled = relay_server;
    // Keep the path deterministic: this suite measures relay fallback, not
    // whether a loopback DCUtR upgrade happens to win a race.
    config.dcutr_enabled = false;
    config.autonat_enabled = false;
    config
}

fn local_key() -> LocalEncryptionKey {
    LocalEncryptionKey::new(LEK)
}

fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if condition() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("timed out waiting for {what}");
}

fn listen_address(network: &Network, needle: &str) -> String {
    let mut found = None;
    wait_until(needle, || {
        found = network
            .status()
            .listen_addresses
            .into_iter()
            .find(|address| address.contains(needle));
        found.is_some()
    });
    found.expect("listen address")
}

fn dial_address(network: &Network, needle: &str) -> String {
    let address = listen_address(network, needle);
    if address.ends_with(&format!("/p2p/{}", network.peer_id())) {
        address
    } else {
        format!("{address}/p2p/{}", network.peer_id())
    }
}

fn relay_address(network: &Network) -> String {
    dial_address(network, "/tcp/")
}

fn closed_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve port");
    let port = listener.local_addr().expect("local address").port();
    drop(listener);
    port
}

fn broken_address(peer_id: &str) -> String {
    format!("/ip4/127.0.0.1/tcp/{}/p2p/{peer_id}", closed_port())
}

fn random_peer_id() -> PeerId {
    Keypair::generate_ed25519().public().to_peer_id()
}

#[test]
fn relay_reservation_reports_negotiated_caps() {
    let relay_root = temp_root("reservation-relay");
    let client_root = temp_root("reservation-client");
    let relay = Network::start(config(&relay_root, true), &local_key(), Arc::new(())).expect("start relay");
    let relay_address = relay_address(&relay);

    let mut client_config = config(&client_root, false);
    client_config.relays = vec![relay_address.parse().expect("relay address")];
    let client = Network::start(client_config, &local_key(), Arc::new(())).expect("start client");

    wait_until("relay reservation", || {
        let status = client.status();
        status.relays_connected == 1 && status.relay_reservations == 1
    });
    wait_until("relay server reservation", || {
        relay.status().relay_server_reservations == 1
    });

    let status = client.status();
    assert_eq!(status.relays_configured, 1);
    assert!(status.relay_client_enabled);
    assert_eq!(status.relay_data_limit_bytes, Some(DEFAULT_RELAY_MAX_CIRCUIT_BYTES));
    assert_eq!(
        status.relay_duration_limit_secs,
        Some(DEFAULT_RELAY_MAX_CIRCUIT_DURATION.as_secs())
    );
    assert_eq!(client.relay_list(), vec![relay_address]);

    let server = relay.status();
    assert!(server.relay_server_enabled);
    assert_eq!(
        server.relay_server_max_circuit_bytes,
        Some(DEFAULT_RELAY_MAX_CIRCUIT_BYTES)
    );
    assert_eq!(
        server.relay_server_max_circuit_duration_secs,
        Some(DEFAULT_RELAY_MAX_CIRCUIT_DURATION.as_secs())
    );
}

#[test]
fn direct_connections_are_preferred_when_reachable() {
    let relay_root = temp_root("direct-relay");
    let initiator_root = temp_root("direct-initiator");
    let target_root = temp_root("direct-target");
    let relay = Network::start(config(&relay_root, true), &local_key(), Arc::new(())).expect("start relay");
    let relay_address = relay_address(&relay);

    let mut initiator_config = config(&initiator_root, false);
    initiator_config.relays = vec![relay_address.parse().expect("relay address")];
    let initiator = Network::start(initiator_config, &local_key(), Arc::new(())).expect("start initiator");

    let mut target_config = config(&target_root, false);
    target_config.relays = vec![relay_address.parse().expect("relay address")];
    let target = Network::start(target_config, &local_key(), Arc::new(())).expect("start target");

    wait_until("initiator reservation", || initiator.status().relay_reservations == 1);
    wait_until("target reservation", || target.status().relay_reservations == 1);

    let connected = initiator
        .connect(&dial_address(&target, "/tcp/"))
        .expect("direct connect");
    assert_eq!(connected.peer_id, target.peer_id());
    assert_eq!(connected.path, "direct");
    assert!(connected.relay_peer_id.is_none());
    assert_eq!(initiator.status().fallback_attempts, 0);
    assert!(initiator.status().direct_connections >= 1);
    assert_eq!(initiator.status().connectivity(), "direct");
}

#[test]
fn a_failed_direct_dial_falls_back_to_a_relay() {
    let relay_root = temp_root("fallback-relay");
    let initiator_root = temp_root("fallback-initiator");
    let target_root = temp_root("fallback-target");
    let relay = Network::start(config(&relay_root, true), &local_key(), Arc::new(())).expect("start relay");
    let relay_address = relay_address(&relay);

    let mut initiator_config = config(&initiator_root, false);
    initiator_config.relays = vec![relay_address.parse().expect("relay address")];
    let initiator = Network::start(initiator_config, &local_key(), Arc::new(())).expect("start initiator");

    let mut target_config = config(&target_root, false);
    target_config.relays = vec![relay_address.parse().expect("relay address")];
    let target = Network::start(target_config, &local_key(), Arc::new(())).expect("start target");

    wait_until("initiator reservation", || {
        let status = initiator.status();
        status.relays_connected == 1 && status.relay_reservations == 1
    });
    wait_until("target reservation", || target.status().relay_reservations == 1);

    // The direct address is deliberately a closed port, so only the relayed
    // circuit behind the target's reservation can succeed.
    let connected = initiator
        .connect(&broken_address(target.peer_id()))
        .expect("relay fallback");
    assert_eq!(connected.peer_id, target.peer_id());
    assert_eq!(connected.path, "relay");
    assert_eq!(connected.relay_peer_id.as_deref(), Some(relay.peer_id()));

    let status = initiator.status();
    assert!(status.fallback_attempts >= 1);
    assert!(status.fallback_successes >= 1);
    assert!(status.relay_circuits_established >= 1);
    assert_eq!(status.connectivity(), "relay");

    let reply = initiator
        .send_test(target.peer_id(), b"ember stage 11 relay")
        .expect("relayed test message");
    assert_eq!(reply.peer_id, target.peer_id());
    assert_eq!(reply.payload, b"ember stage 11 relay");

    wait_until("relay server circuit", || {
        relay.status().relay_server_circuits_accepted >= 1
    });
    assert!(relay.status().relay_server_max_circuit_bytes.is_some());
}

#[test]
fn an_unreachable_peer_reports_no_viable_path() {
    let relay_root = temp_root("nopath-relay");
    let initiator_root = temp_root("nopath-initiator");
    let relay = Network::start(config(&relay_root, true), &local_key(), Arc::new(())).expect("start relay");
    let relay_address = relay_address(&relay);

    let mut initiator_config = config(&initiator_root, false);
    initiator_config.relays = vec![relay_address.parse().expect("relay address")];
    let initiator = Network::start(initiator_config, &local_key(), Arc::new(())).expect("start initiator");

    wait_until("initiator reservation", || {
        let status = initiator.status();
        status.relays_connected == 1 && status.relay_reservations == 1
    });

    let target = random_peer_id();
    let error = initiator
        .connect(&broken_address(&target.to_string()))
        .expect_err("an unavailable peer must fail");
    assert!(
        matches!(error, NetError::NoViablePath),
        "expected NoViablePath, got {error:?}"
    );
    assert_eq!(initiator.status().last_error.as_deref(), Some("no_viable_path"));
    assert!(initiator.status().fallback_attempts >= 1);
}

#[test]
fn relay_list_persists_across_restarts() {
    let root = temp_root("persist");
    let relay_root = temp_root("persist-relay");
    let relay = Network::start(config(&relay_root, true), &local_key(), Arc::new(())).expect("start relay");
    let relay_address = relay_address(&relay);

    let network = Network::start(config(&root, false), &local_key(), Arc::new(())).expect("start");
    assert!(network.relay_list().is_empty());
    network.relay_add(&relay_address).expect("add relay");
    assert_eq!(network.relay_list(), vec![relay_address.clone()]);
    network.shutdown();
    drop(network);

    let path = root.join(NETWORK_DIRECTORY).join(RELAY_FILE_NAME);
    assert!(path.is_file(), "relay list file exists");
    let text = std::fs::read_to_string(&path).expect("read relay list");
    assert_eq!(text.trim(), relay_address);

    let restarted = Network::start(config(&root, false), &local_key(), Arc::new(())).expect("restart");
    assert_eq!(restarted.relay_list(), vec![relay_address.clone()]);
    restarted.relay_remove(&relay_address).expect("remove relay");
    assert!(restarted.relay_list().is_empty());
    let text = std::fs::read_to_string(&path).expect("read empty relay list");
    assert!(text.is_empty());

    assert!(matches!(
        restarted.relay_add("/ip4/127.0.0.1/tcp/4001"),
        Err(NetError::MissingPeerId)
    ));
}
