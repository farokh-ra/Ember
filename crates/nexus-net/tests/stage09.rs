//! Stage 09 loopback integration tests: transports, authentication, reconnection.
//!
//! Every test binds loopback listeners only. A test never claims public
//! reachability and never depends on an external service.

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use libp2p::identity::Keypair;
use libp2p::{Multiaddr, PeerId};
use nexus_identity::LocalEncryptionKey;
use nexus_net::{BOOTSTRAP_FILE_NAME, NetError, Network, NetworkConfig, NetworkEvent, NetworkObserver, PeerEventKind};

static COUNTER: AtomicU64 = AtomicU64::new(0);
const LEK: [u8; 32] = [9_u8; 32];

fn temp_root(name: &str) -> PathBuf {
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("nexus-net-stage09-{name}-{}-{counter}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("root");
    path
}

fn config(root: &Path) -> NetworkConfig {
    let mut config = NetworkConfig::new(root);
    config.listen = vec![
        "/ip4/127.0.0.1/udp/0/quic-v1".parse().expect("quic listen"),
        "/ip4/127.0.0.1/tcp/0".parse().expect("tcp listen"),
    ];
    config.connect_timeout = Duration::from_secs(5);
    config.request_timeout = Duration::from_secs(5);
    config.reconnect_base_delay = Duration::from_millis(250);
    config.reconnect_max_delay = Duration::from_secs(2);
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

fn random_peer_id() -> PeerId {
    Keypair::generate_ed25519().public().to_peer_id()
}

#[derive(Default)]
struct RecordingObserver {
    events: Mutex<Vec<NetworkEvent>>,
}

impl NetworkObserver for RecordingObserver {
    fn on_event(&self, event: NetworkEvent) {
        self.events.lock().expect("observer lock").push(event);
    }
}

impl RecordingObserver {
    fn saw_connected(&self) -> bool {
        self.events.lock().expect("observer lock").iter().any(|event| {
            matches!(
                event,
                NetworkEvent::Peer {
                    event: PeerEventKind::Connected,
                    ..
                }
            )
        })
    }

    fn states(&self) -> Vec<nexus_net::NetworkState> {
        self.events
            .lock()
            .expect("observer lock")
            .iter()
            .filter_map(|event| match event {
                NetworkEvent::State { state, .. } => Some(*state),
                _ => None,
            })
            .collect()
    }
}

#[test]
fn two_nodes_exchange_a_test_message_over_quic_and_tcp() {
    let root_a = temp_root("a");
    let root_b = temp_root("b");
    let observer_a = Arc::new(RecordingObserver::default());
    let observer_b = Arc::new(RecordingObserver::default());

    let a = Network::start(config(&root_a), &local_key(), observer_a.clone()).expect("start a");
    let b = Network::start(config(&root_b), &local_key(), observer_b.clone()).expect("start b");

    let quic = a.connect(&dial_address(&b, "/quic-v1")).expect("quic connect");
    assert_eq!(quic.transport, "quic");
    assert_eq!(quic.peer_id, b.peer_id());
    assert!(quic.remote_address.contains("/quic-v1"));
    assert!(quic.remote_address.contains("/udp/"));

    let reply = a.send_test(b.peer_id(), b"ember stage 09 quic").expect("quic test");
    assert_eq!(reply.peer_id, b.peer_id());
    assert_eq!(reply.payload, b"ember stage 09 quic");

    let tcp = a.connect(&dial_address(&b, "/tcp/")).unwrap_or_else(|error| {
        panic!("tcp connect: {error:?} last_error={:?}", a.status().last_error);
    });
    assert_eq!(tcp.transport, "tcp");
    assert_eq!(tcp.peer_id, b.peer_id());

    let reply = a.send_test(b.peer_id(), b"ember stage 09 tcp").expect("tcp test");
    assert_eq!(reply.peer_id, b.peer_id());
    assert_eq!(reply.payload, b"ember stage 09 tcp");

    assert!(observer_a.saw_connected(), "a observed a peer connection");
    assert!(observer_b.saw_connected(), "b observed a peer connection");
    assert!(
        observer_a.states().contains(&nexus_net::NetworkState::Online),
        "a reached online"
    );
    assert_eq!(a.status().peer_count, 1);
    assert_eq!(b.status().peer_count, 1);
    assert_eq!(a.status().requests_served, 0);
    assert!(b.status().requests_served >= 2, "b served two requests");
    assert_eq!(b.status().requests_rejected, 0);
    assert!(b.status().confirmed_addresses.is_empty());
    assert!(b.status().observed_addresses.len() <= 16);
}

#[test]
fn a_mismatched_peer_identity_is_rejected() {
    let root_a = temp_root("wrong-a");
    let root_b = temp_root("wrong-b");
    let a = Network::start(config(&root_a), &local_key(), Arc::new(())).expect("start a");
    let b = Network::start(config(&root_b), &local_key(), Arc::new(())).expect("start b");

    let socket = listen_address(&b, "/tcp/");
    let target = format!("{socket}/p2p/{}", random_peer_id());
    let error = a.connect(&target).expect_err("mismatched identity must fail");
    assert!(
        matches!(error, NetError::WrongPeerId),
        "expected WrongPeerId, got {error:?}"
    );
    assert_ne!(b.peer_id(), target.rsplit('/').next().unwrap_or_default());
    assert!(a.peers().iter().all(|peer| peer.peer_id != b.peer_id()));
}

#[test]
fn unreachable_peers_fail_without_stopping_the_node() {
    let root_a = temp_root("unreachable-a");
    let root_b = temp_root("unreachable-b");
    let a = Network::start(config(&root_a), &local_key(), Arc::new(())).expect("start a");
    let b = Network::start(config(&root_b), &local_key(), Arc::new(())).expect("start b");

    let reservation = TcpListener::bind("127.0.0.1:0").expect("reserve port");
    let port = reservation.local_addr().expect("port").port();
    drop(reservation);

    let target = format!("/ip4/127.0.0.1/tcp/{port}/p2p/{}", random_peer_id());
    let error = a.connect(&target).expect_err("closed port must fail");
    assert!(
        matches!(error, NetError::ConnectionFailed | NetError::Timeout),
        "expected a bounded connection failure, got {error:?}"
    );

    let reply = a
        .connect(&dial_address(&b, "/tcp/"))
        .and_then(|_| a.send_test(b.peer_id(), b"still usable"))
        .expect("node remains usable");
    assert_eq!(reply.payload, b"still usable");
}

#[test]
fn reconnect_backoff_recovers_after_a_peer_restarts() {
    let root_a = temp_root("reconnect-a");
    let root_b = temp_root("reconnect-b");

    let first = Network::start(config(&root_b), &local_key(), Arc::new(())).expect("first b");
    let address = listen_address(&first, "/tcp/");
    let peer_id = first.peer_id().to_owned();
    let port = address.rsplit('/').next().expect("port").to_owned();
    drop(first);

    let mut a_config = config(&root_a);
    a_config.bootstrap = vec![format!("{address}/p2p/{peer_id}").parse().expect("bootstrap")];
    let a = Network::start(a_config, &local_key(), Arc::new(())).expect("start a");

    let mut b_config = config(&root_b);
    b_config.listen = vec![format!("/ip4/127.0.0.1/tcp/{port}").parse().expect("rebind")];
    let b = Network::start(b_config, &local_key(), Arc::new(())).expect("restart b");
    assert_eq!(b.peer_id(), peer_id);

    wait_until("bootstrap reconnect", || a.status().peer_count == 1);
    let reply = a.send_test(&peer_id, b"reconnected").expect("reconnected test");
    assert_eq!(reply.payload, b"reconnected");
    assert_eq!(a.status().bootstrap_configured, 1);
    assert_eq!(a.status().bootstrap_connected, 1);
}

#[test]
fn a_network_change_resets_backoff_and_reconnects_promptly() {
    let root_a = temp_root("network-change-a");
    let root_b = temp_root("network-change-b");

    let first = Network::start(config(&root_b), &local_key(), Arc::new(())).expect("first b");
    let address = listen_address(&first, "/tcp/");
    let peer_id = first.peer_id().to_owned();
    let port = address.rsplit('/').next().expect("port").to_owned();
    first.shutdown();

    let mut a_config = config(&root_a);
    a_config.bootstrap = vec![format!("{address}/p2p/{peer_id}").parse().expect("bootstrap")];
    a_config.reconnect_base_delay = Duration::from_millis(200);
    a_config.reconnect_max_delay = Duration::from_secs(30);
    a_config.connect_timeout = Duration::from_millis(500);
    let a = Network::start(a_config, &local_key(), Arc::new(())).expect("start a");

    // Five failed dials leave the next bootstrap retry several seconds away.
    wait_until("five failed bootstrap dials", || a.status().dial_failures >= 5);

    let mut b_config = config(&root_b);
    b_config.listen = vec![format!("/ip4/127.0.0.1/tcp/{port}").parse().expect("rebind")];
    let b = Network::start(b_config, &local_key(), Arc::new(())).expect("restart b");
    assert_eq!(b.peer_id(), peer_id);

    // A Wi-Fi switch or sleep/wake resets the backoff; the next tick re-dials.
    a.notify_network_change().expect("network change");
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline && a.status().peer_count == 0 {
        std::thread::sleep(Duration::from_millis(25));
    }
    assert_eq!(a.status().peer_count, 1, "a reset backoff reconnects promptly");
    let reply = a
        .send_test(&peer_id, b"after a network change")
        .expect("test after a network change");
    assert_eq!(reply.payload, b"after a network change");
}

#[test]
fn bootstrap_peers_persist_across_restarts() {
    let root_a = temp_root("bootstrap-a");
    let root_b = temp_root("bootstrap-b");
    let b = Network::start(config(&root_b), &local_key(), Arc::new(())).expect("start b");
    let address = dial_address(&b, "/tcp/");

    let a = Network::start(config(&root_a), &local_key(), Arc::new(())).expect("start a");
    assert!(a.bootstrap_list().is_empty());
    a.bootstrap_add(&address).expect("add bootstrap");
    assert_eq!(a.bootstrap_list(), vec![address.clone()]);
    a.shutdown();

    let a = Network::start(config(&root_a), &local_key(), Arc::new(())).expect("restart a");
    assert_eq!(a.bootstrap_list(), vec![address.clone()]);
    assert_eq!(a.status().bootstrap_configured, 1);
    assert!(root_a.join("network").join(BOOTSTRAP_FILE_NAME).is_file());

    a.bootstrap_remove(&address).expect("remove bootstrap");
    assert!(a.bootstrap_list().is_empty());
    let error = a.bootstrap_remove(&address).expect_err("missing entry");
    assert!(matches!(error, NetError::PeerNotFound));
}

#[test]
fn start_and_stop_is_repeatable() {
    let root = temp_root("cycles");
    let key = local_key();
    let mut config = config(&root);
    config.listen = vec!["/ip4/127.0.0.1/tcp/0".parse().expect("tcp listen")];

    let mut peer_id = None;
    for cycle in 0..100 {
        let network = Network::start(config.clone(), &key, Arc::new(())).unwrap_or_else(|error| {
            panic!("start cycle {cycle} failed: {error:?}");
        });
        match &peer_id {
            Some(expected) => assert_eq!(network.peer_id(), expected),
            None => peer_id = Some(network.peer_id().to_owned()),
        }
        assert!(network.status().listen_addresses.len() <= nexus_net::MAX_LISTEN_ADDRESSES);
        network.shutdown();
    }
}

#[test]
fn network_config_rejects_invalid_addresses_and_bounds() {
    let root = temp_root("config");
    let mut invalid = config(&root);
    invalid.bootstrap = vec!["/ip4/127.0.0.1/tcp/4001".parse().expect("address")];
    assert!(matches!(
        Network::start(invalid, &local_key(), Arc::new(())),
        Err(NetError::MissingPeerId)
    ));

    let relative = NetworkConfig::new(PathBuf::from("relative"));
    assert!(matches!(
        Network::start(relative, &local_key(), Arc::new(())),
        Err(NetError::InvalidConfig(_))
    ));

    let mut unparsable: Multiaddr = "/ip4/127.0.0.1/tcp/1".parse().expect("address");
    unparsable.push(libp2p::multiaddr::Protocol::Tcp(2));
    let mut config = config(&root);
    config.bootstrap = vec![unparsable];
    assert!(matches!(
        Network::start(config, &local_key(), Arc::new(())),
        Err(NetError::MissingPeerId)
    ));
}
