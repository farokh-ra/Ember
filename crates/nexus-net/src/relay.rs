//! Persisted community relay list.
//!
//! The list lives in `network/relays.list` as one `/p2p`-terminated
//! multiaddress per line. Relays are optional infrastructure: the network
//! keeps working when this file is empty, and an operator can add or remove a
//! relay at runtime through `network.relay_add` / `network.relay_remove`.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use libp2p::Multiaddr;

use crate::config::{MAX_RELAYS, require_peer_id};
use crate::error::NetError;
use crate::peer_key::NETWORK_DIRECTORY;

/// Exact file name of the persisted relay list.
pub const RELAY_FILE_NAME: &str = "relays.list";
/// Maximum accepted file size.
pub const RELAY_MAX_FILE_LENGTH: u64 = 64 * 1024;
/// Compile-time community relay list environment variable.
///
/// Release builds may bake relay entry points in by setting
/// `EMBER_RELAY_NODES` to one or more `/p2p`-terminated multiaddresses
/// separated by semicolons, commas, or newlines. Runtime configuration and the
/// persisted list are merged with these entries.
pub const RELAY_ENV_VAR: &str = "EMBER_RELAY_NODES";

const COMPILED_RELAYS: Option<&str> = option_env!("EMBER_RELAY_NODES");

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Loads the persisted list, rejecting oversized files and invalid entries.
pub(crate) fn load(data_root: &Path) -> Result<Vec<Multiaddr>, NetError> {
    let path = data_root.join(NETWORK_DIRECTORY).join(RELAY_FILE_NAME);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(NetError::Io(error)),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(NetError::InvalidConfig("relays"));
    }
    if metadata.len() > RELAY_MAX_FILE_LENGTH {
        return Err(NetError::InvalidConfig("relays"));
    }
    let text = fs::read_to_string(&path).map_err(NetError::Io)?;
    parse(&text)
}

/// Loads the compile-time community relay list baked into a release build.
pub(crate) fn compiled() -> Result<Vec<Multiaddr>, NetError> {
    let Some(text) = COMPILED_RELAYS else {
        return Ok(Vec::new());
    };
    let normalized = text.replace([';', ','], "\n");
    parse(&normalized)
}

/// Persists the list atomically.
pub(crate) fn save(data_root: &Path, addresses: &[Multiaddr]) -> Result<(), NetError> {
    if addresses.len() > MAX_RELAYS {
        return Err(NetError::InvalidConfig("relays"));
    }
    let directory = data_root.join(NETWORK_DIRECTORY);
    fs::create_dir_all(&directory).map_err(NetError::Io)?;
    let mut text = String::new();
    for address in addresses {
        require_peer_id(address, true)?;
        text.push_str(&address.to_string());
        text.push('\n');
    }
    if text.len() as u64 > RELAY_MAX_FILE_LENGTH {
        return Err(NetError::InvalidConfig("relays"));
    }

    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temporary = directory.join(format!(".relays-{}-{counter}.tmp", std::process::id()));
    let path = directory.join(RELAY_FILE_NAME);
    let result = (|| -> Result<(), NetError> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(NetError::Io)?;
        file.write_all(text.as_bytes()).map_err(NetError::Io)?;
        file.sync_all().map_err(NetError::Io)?;
        drop(file);
        fs::rename(&temporary, &path).map_err(NetError::Io)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

/// Parses a bounded relay file, deduplicating entries.
pub(crate) fn parse(text: &str) -> Result<Vec<Multiaddr>, NetError> {
    let mut addresses: Vec<Multiaddr> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let address: Multiaddr = line.parse().map_err(|_| NetError::InvalidConfig("relays"))?;
        require_peer_id(&address, true)?;
        if !addresses.contains(&address) {
            if addresses.len() >= MAX_RELAYS {
                return Err(NetError::InvalidConfig("relays"));
            }
            addresses.push(address);
        }
    }
    Ok(addresses)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use libp2p::Multiaddr;
    use libp2p::PeerId;
    use libp2p::identity::Keypair;

    use super::{RELAY_FILE_NAME, load, parse, save};
    use crate::error::NetError;
    use crate::peer_key::NETWORK_DIRECTORY;

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn root(name: &str) -> PathBuf {
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("nexus-net-relay-{name}-{}-{counter}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("root");
        path
    }

    fn address() -> Multiaddr {
        let peer_id = PeerId::from(Keypair::generate_ed25519().public());
        format!("/ip4/127.0.0.1/tcp/4001/p2p/{peer_id}")
            .parse()
            .expect("address")
    }

    #[test]
    fn saves_and_reloads_a_deduplicated_list() {
        let root = root("round-trip");
        let address = address();
        save(&root, std::slice::from_ref(&address)).expect("save");
        assert_eq!(load(&root).expect("load"), vec![address.clone()]);

        let text = format!("{address}\n{address}\n");
        assert_eq!(parse(&text).expect("parse"), vec![address]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn missing_file_is_an_empty_list() {
        let root = root("missing");
        assert!(load(&root).expect("load").is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_addresses_without_a_peer_id() {
        let root = root("no-peer");
        let directory = root.join(NETWORK_DIRECTORY);
        fs::create_dir_all(&directory).expect("directory");
        fs::write(directory.join(RELAY_FILE_NAME), "/ip4/127.0.0.1/tcp/4001\n").expect("write");
        assert!(matches!(load(&root), Err(NetError::MissingPeerId)));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_oversized_files() {
        let root = root("oversized");
        let directory = root.join(NETWORK_DIRECTORY);
        fs::create_dir_all(&directory).expect("directory");
        fs::write(
            directory.join(RELAY_FILE_NAME),
            vec![b'x'; (super::RELAY_MAX_FILE_LENGTH + 1) as usize],
        )
        .expect("write");
        assert!(matches!(load(&root), Err(NetError::InvalidConfig("relays"))));
        let _ = fs::remove_dir_all(root);
    }
}
