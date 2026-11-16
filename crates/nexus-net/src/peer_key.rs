//! Encrypted device peer key that is independent of the user identity.
//!
//! The peer key identifies this installation on the network. It is stored in
//! `network/peer_key.enc`, encrypted under the host-supplied local encryption
//! key. Restoring a user identity backup never reads or replaces this file.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use libp2p::PeerId;
use libp2p::identity::Keypair;
use nexus_crypto::{AEAD_NONCE_LENGTH, AEAD_TAG_LENGTH, DomainTag, decrypt, encrypt, hkdf_sha256_32, random_array};
use nexus_identity::LocalEncryptionKey;
use zeroize::Zeroizing;

use crate::error::NetError;

/// Directory below the app-private root that holds networking secrets.
pub const NETWORK_DIRECTORY: &str = "network";
/// Exact file name of the encrypted device peer key.
pub const PEER_KEY_FILE_NAME: &str = "peer_key.enc";
/// Peer-key file magic.
pub const PEER_KEY_MAGIC: [u8; 8] = *b"NEXPEER1";
/// Peer-key format version.
pub const PEER_KEY_FORMAT_VERSION: u16 = 1;
/// KDF identifier for HKDF-SHA256.
pub const PEER_KEY_KDF_HKDF_SHA256: u8 = 1;
/// Salt length.
pub const PEER_KEY_SALT_LENGTH: usize = 32;
/// Header length.
pub const PEER_KEY_HEADER_LENGTH: usize = 8 + 2 + 1 + 1 + PEER_KEY_SALT_LENGTH + AEAD_NONCE_LENGTH;
/// Seed length for the Ed25519 device key.
pub const PEER_KEY_SEED_LENGTH: usize = 32;
/// Maximum accepted peer-key file size.
pub const PEER_KEY_MAX_FILE_LENGTH: usize = PEER_KEY_HEADER_LENGTH + PEER_KEY_SEED_LENGTH + AEAD_TAG_LENGTH;

const VERSION_OFFSET: usize = 8;
const KDF_OFFSET: usize = 10;
const FLAGS_OFFSET: usize = 11;
const SALT_OFFSET: usize = 12;
const NONCE_OFFSET: usize = 44;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// An unlocked device peer key.
pub struct PeerKey {
    keypair: Keypair,
    path: PathBuf,
}

impl PeerKey {
    /// Loads the device peer key, creating it on first use.
    pub fn load_or_create(data_root: &Path, lek: &LocalEncryptionKey) -> Result<Self, NetError> {
        if !data_root.is_absolute() {
            return Err(NetError::InvalidConfig("data_root"));
        }
        let directory = data_root.join(NETWORK_DIRECTORY);
        prepare_directory(data_root)?;
        prepare_directory(&directory)?;
        let path = directory.join(PEER_KEY_FILE_NAME);
        match read_bounded(&path)? {
            Some(file) => Self::from_file(file, path, lek),
            None => {
                let key = Self::generate(path.clone(), lek)?;
                Ok(key)
            }
        }
    }

    /// Loads an existing device peer key without creating one.
    pub fn load(data_root: &Path, lek: &LocalEncryptionKey) -> Result<Self, NetError> {
        if !data_root.is_absolute() {
            return Err(NetError::InvalidConfig("data_root"));
        }
        let path = data_root.join(NETWORK_DIRECTORY).join(PEER_KEY_FILE_NAME);
        let file = read_bounded(&path)?.ok_or(NetError::PeerKeyLocked)?;
        Self::from_file(file, path, lek)
    }

    /// Returns the peer ID derived from the device key.
    #[must_use]
    pub fn peer_id(&self) -> PeerId {
        self.keypair.public().to_peer_id()
    }

    /// Returns the encrypted peer-key file path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn keypair(&self) -> &Keypair {
        &self.keypair
    }

    fn generate(path: PathBuf, lek: &LocalEncryptionKey) -> Result<Self, NetError> {
        let seed = Zeroizing::new(random_array::<PEER_KEY_SEED_LENGTH>()?);
        let salt = random_array::<PEER_KEY_SALT_LENGTH>()?;
        let nonce = random_array::<AEAD_NONCE_LENGTH>()?;
        let file = encrypt_seed(&seed, lek, &salt, &nonce)?;
        atomic_write(&path, &file)?;
        let mut seed_copy = Zeroizing::new(*seed);
        let keypair = Keypair::ed25519_from_bytes(&mut seed_copy[..]).map_err(|_| NetError::PeerKeyInvalid)?;
        Ok(Self { keypair, path })
    }

    fn from_file(file: Vec<u8>, path: PathBuf, lek: &LocalEncryptionKey) -> Result<Self, NetError> {
        let mut seed = decrypt_seed(&file, lek)?;
        let keypair = Keypair::ed25519_from_bytes(&mut seed[..]).map_err(|_| NetError::PeerKeyInvalid)?;
        Ok(Self { keypair, path })
    }
}

impl std::fmt::Debug for PeerKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PeerKey")
            .field("peer_id", &self.peer_id().to_string())
            .field("path", &self.path)
            .finish()
    }
}

fn encrypt_seed(
    seed: &[u8; PEER_KEY_SEED_LENGTH],
    lek: &LocalEncryptionKey,
    salt: &[u8; PEER_KEY_SALT_LENGTH],
    nonce: &[u8; AEAD_NONCE_LENGTH],
) -> Result<Vec<u8>, NetError> {
    let mut header = Vec::with_capacity(PEER_KEY_HEADER_LENGTH);
    header.extend_from_slice(&PEER_KEY_MAGIC);
    header.extend_from_slice(&PEER_KEY_FORMAT_VERSION.to_le_bytes());
    header.push(PEER_KEY_KDF_HKDF_SHA256);
    header.push(0);
    header.extend_from_slice(salt);
    header.extend_from_slice(nonce);
    debug_assert_eq!(header.len(), PEER_KEY_HEADER_LENGTH);

    let key = hkdf_sha256_32(lek.expose(), salt, DomainTag::PeerKey.as_bytes())?;
    let ciphertext = encrypt(&key, nonce, &header, seed)?;
    let mut file = Vec::with_capacity(PEER_KEY_MAX_FILE_LENGTH);
    file.extend_from_slice(&header);
    file.extend_from_slice(&ciphertext);
    if file.len() != PEER_KEY_MAX_FILE_LENGTH {
        return Err(NetError::PeerKeyInvalid);
    }
    Ok(file)
}

fn decrypt_seed(file: &[u8], lek: &LocalEncryptionKey) -> Result<Zeroizing<[u8; PEER_KEY_SEED_LENGTH]>, NetError> {
    if file.len() != PEER_KEY_MAX_FILE_LENGTH {
        return Err(NetError::PeerKeyInvalid);
    }
    if file[..8] != PEER_KEY_MAGIC {
        return Err(NetError::PeerKeyInvalid);
    }
    let version = u16::from_le_bytes([file[VERSION_OFFSET], file[VERSION_OFFSET + 1]]);
    if version != PEER_KEY_FORMAT_VERSION || file[KDF_OFFSET] != PEER_KEY_KDF_HKDF_SHA256 || file[FLAGS_OFFSET] != 0 {
        return Err(NetError::PeerKeyInvalid);
    }
    let header = &file[..PEER_KEY_HEADER_LENGTH];
    let salt: [u8; PEER_KEY_SALT_LENGTH] = header[SALT_OFFSET..SALT_OFFSET + PEER_KEY_SALT_LENGTH]
        .try_into()
        .map_err(|_| NetError::PeerKeyInvalid)?;
    let nonce: [u8; AEAD_NONCE_LENGTH] = header[NONCE_OFFSET..NONCE_OFFSET + AEAD_NONCE_LENGTH]
        .try_into()
        .map_err(|_| NetError::PeerKeyInvalid)?;
    let key = hkdf_sha256_32(lek.expose(), &salt, DomainTag::PeerKey.as_bytes())?;
    let plaintext =
        decrypt(&key, &nonce, header, &file[PEER_KEY_HEADER_LENGTH..]).map_err(|_| NetError::PeerKeyLocked)?;
    let seed: [u8; PEER_KEY_SEED_LENGTH] = plaintext.as_slice().try_into().map_err(|_| NetError::PeerKeyInvalid)?;
    Ok(Zeroizing::new(seed))
}

fn prepare_directory(path: &Path) -> Result<(), NetError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if is_link_or_reparse(&metadata) => Err(NetError::PeerKeyUnavailable(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "network directory is a link",
        ))),
        Ok(metadata) if !metadata.is_dir() => Err(NetError::PeerKeyUnavailable(std::io::Error::new(
            std::io::ErrorKind::NotADirectory,
            "network path is not a directory",
        ))),
        Ok(_) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(NetError::PeerKeyUnavailable)?;
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(path).map_err(NetError::PeerKeyUnavailable)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(NetError::PeerKeyUnavailable)?;
            }
            Ok(())
        }
        Err(error) => Err(NetError::PeerKeyUnavailable(error)),
    }
}

fn read_bounded(path: &Path) -> Result<Option<Vec<u8>>, NetError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(NetError::PeerKeyUnavailable(error)),
    };
    if is_link_or_reparse(&metadata) || !metadata.is_file() {
        return Err(NetError::PeerKeyInvalid);
    }
    if metadata.len() as usize != PEER_KEY_MAX_FILE_LENGTH {
        return Err(NetError::PeerKeyInvalid);
    }
    let file = fs::read(path).map_err(NetError::PeerKeyUnavailable)?;
    Ok(Some(file))
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), NetError> {
    let directory = path.parent().ok_or(NetError::PeerKeyInvalid)?;
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temporary = directory.join(format!(".peer-key-{}-{counter}.tmp", std::process::id()));

    let result = (|| -> Result<(), NetError> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(NetError::PeerKeyUnavailable)?;
        file.write_all(bytes).map_err(NetError::PeerKeyUnavailable)?;
        file.sync_all().map_err(NetError::PeerKeyUnavailable)?;
        drop(file);
        fs::rename(&temporary, path).map_err(NetError::PeerKeyUnavailable)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use nexus_identity::LocalEncryptionKey;

    use super::{NETWORK_DIRECTORY, PEER_KEY_FILE_NAME, PeerKey};
    use crate::error::NetError;

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn root(name: &str) -> PathBuf {
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("nexus-net-key-{name}-{}-{counter}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("root");
        path
    }

    #[test]
    fn creates_persists_and_reloads_one_device_key() {
        let root = root("persist");
        let lek = LocalEncryptionKey::new([7_u8; 32]);
        let first = PeerKey::load_or_create(&root, &lek).expect("create");
        let peer_id = first.peer_id();
        let path = first.path().to_path_buf();
        assert_eq!(path, root.join(NETWORK_DIRECTORY).join(PEER_KEY_FILE_NAME));
        drop(first);

        let second = PeerKey::load_or_create(&root, &lek).expect("reload");
        assert_eq!(second.peer_id(), peer_id);
        assert_eq!(
            fs::metadata(&path).expect("metadata").len() as usize,
            super::PEER_KEY_MAX_FILE_LENGTH
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_a_wrong_local_secret_without_overwriting() {
        let root = root("wrong-lek");
        let first = PeerKey::load_or_create(&root, &LocalEncryptionKey::new([1_u8; 32])).expect("create");
        let peer_id = first.peer_id();
        let path = first.path().to_path_buf();
        drop(first);

        let error = PeerKey::load_or_create(&root, &LocalEncryptionKey::new([2_u8; 32])).expect_err("locked");
        assert!(matches!(error, NetError::PeerKeyLocked));
        let bytes = fs::read(&path).expect("file");
        assert_eq!(bytes.len(), super::PEER_KEY_MAX_FILE_LENGTH);

        let reopened = PeerKey::load(&root, &LocalEncryptionKey::new([1_u8; 32])).expect("reopen");
        assert_eq!(reopened.peer_id(), peer_id);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_tampering_and_unknown_formats() {
        let root = root("tamper");
        let lek = LocalEncryptionKey::new([3_u8; 32]);
        let key = PeerKey::load_or_create(&root, &lek).expect("create");
        let path = key.path().to_path_buf();
        drop(key);

        let mut bytes = fs::read(&path).expect("read");
        bytes[0] ^= 0x01;
        fs::write(&path, &bytes).expect("write magic");
        assert!(matches!(PeerKey::load(&root, &lek), Err(NetError::PeerKeyInvalid)));

        let mut bytes = fs::read(&path).expect("read");
        bytes[0] ^= 0x01;
        bytes[super::PEER_KEY_HEADER_LENGTH + 3] ^= 0x01;
        fs::write(&path, &bytes).expect("write ciphertext");
        assert!(matches!(PeerKey::load(&root, &lek), Err(NetError::PeerKeyLocked)));

        fs::write(&path, vec![0_u8; super::PEER_KEY_MAX_FILE_LENGTH + 1]).expect("write size");
        assert!(matches!(PeerKey::load(&root, &lek), Err(NetError::PeerKeyInvalid)));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn identity_files_do_not_affect_the_peer_key() {
        let root = root("identity");
        let lek = LocalEncryptionKey::new([4_u8; 32]);
        let first = PeerKey::load_or_create(&root, &lek).expect("create");
        let peer_id = first.peer_id();
        drop(first);

        let secrets = root.join("secrets");
        fs::create_dir_all(&secrets).expect("secrets");
        fs::write(secrets.join("identity.enc"), b"unrelated identity bytes").expect("identity");

        let second = PeerKey::load_or_create(&root, &lek).expect("reload");
        assert_eq!(second.peer_id(), peer_id);
        let _ = fs::remove_dir_all(root);
    }
}
