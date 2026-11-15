//! Atomic identity-at-rest storage under the app-private root.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::at_rest::{AT_REST_MAX_FILE_LENGTH, decrypt_identity, encrypt_identity};
use crate::backup;
use crate::error::IdentityError;
use crate::identity::Identity;
use crate::local_key::LocalEncryptionKey;

/// Directory below the app-private root that holds the encrypted identity.
pub const SECRETS_DIRECTORY: &str = "secrets";
/// Exact file name of the encrypted identity.
pub const IDENTITY_FILE_NAME: &str = "identity.enc";

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Encrypted identity storage rooted at the app-private directory.
pub struct IdentityVault {
    root: PathBuf,
}

impl IdentityVault {
    /// Creates a vault rooted at an absolute app-private path.
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, IdentityError> {
        let root = root.into();
        if !root.is_absolute() {
            return Err(IdentityError::UnsafePath);
        }
        Ok(Self { root })
    }

    /// Returns the app-private root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns `secrets/identity.enc` below the root.
    #[must_use]
    pub fn identity_path(&self) -> PathBuf {
        self.root.join(SECRETS_DIRECTORY).join(IDENTITY_FILE_NAME)
    }

    /// Reports whether an encrypted identity file exists.
    pub fn exists(&self) -> Result<bool, IdentityError> {
        let Some(secrets) = self.existing_secrets_directory()? else {
            return Ok(false);
        };
        let metadata = match fs::symlink_metadata(secrets.join(IDENTITY_FILE_NAME)) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        if is_link_or_reparse(&metadata) {
            return Err(IdentityError::SymlinkRejected);
        }
        if !metadata.is_file() {
            return Err(IdentityError::InvalidIdentityFile);
        }
        Ok(true)
    }

    /// Creates and saves a new identity.
    ///
    /// Fails with [`IdentityError::AlreadyExists`] instead of replacing an
    /// existing identity.
    pub fn create(&self, lek: &LocalEncryptionKey) -> Result<Identity, IdentityError> {
        if self.exists()? {
            return Err(IdentityError::AlreadyExists);
        }
        let identity = Identity::create()?;
        self.save(lek, &identity)?;
        Ok(identity)
    }

    /// Loads and decrypts the identity file.
    pub fn load(&self, lek: &LocalEncryptionKey) -> Result<Identity, IdentityError> {
        let Some(secrets) = self.existing_secrets_directory()? else {
            return Err(IdentityError::NotFound);
        };
        let path = secrets.join(IDENTITY_FILE_NAME);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(IdentityError::NotFound);
            }
            Err(error) => return Err(error.into()),
        };
        if is_link_or_reparse(&metadata) {
            return Err(IdentityError::SymlinkRejected);
        }
        if !metadata.is_file() || metadata.len() > AT_REST_MAX_FILE_LENGTH as u64 {
            return Err(IdentityError::InvalidIdentityFile);
        }
        let file = fs::read(path)?;
        decrypt_identity(&file, lek)
    }

    /// Atomically replaces the identity file after encrypting it.
    pub fn save(&self, lek: &LocalEncryptionKey, identity: &Identity) -> Result<(), IdentityError> {
        let secrets = self.prepare_secrets_directory()?;
        let path = secrets.join(IDENTITY_FILE_NAME);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if is_link_or_reparse(&metadata) => {
                return Err(IdentityError::SymlinkRejected);
            }
            Ok(metadata) if !metadata.is_file() => {
                return Err(IdentityError::InvalidIdentityFile);
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }

        let file = encrypt_identity(identity, lek)?;
        atomic_write(&path, &file)
    }

    /// Exports an encrypted backup with the v1 Argon2id parameters.
    pub fn export_backup(&self, identity: &Identity, passphrase: &str) -> Result<Vec<u8>, IdentityError> {
        backup::export_backup(identity, passphrase)
    }

    /// Restores a backup and saves it only after full validation and decryption.
    ///
    /// A wrong passphrase, corrupt file, or unsupported version returns an
    /// error before the live identity file is touched.
    pub fn restore_backup(
        &self,
        lek: &LocalEncryptionKey,
        passphrase: &str,
        backup: &[u8],
    ) -> Result<Identity, IdentityError> {
        let identity = backup::restore_backup(backup, passphrase)?;
        self.save(lek, &identity)?;
        Ok(identity)
    }

    fn existing_secrets_directory(&self) -> Result<Option<PathBuf>, IdentityError> {
        if !self.root.is_absolute() {
            return Err(IdentityError::UnsafePath);
        }
        if !checked_directory(&self.root)? {
            return Ok(None);
        }
        let secrets = self.root.join(SECRETS_DIRECTORY);
        if !checked_directory(&secrets)? {
            return Ok(None);
        }
        Ok(Some(secrets))
    }

    fn prepare_secrets_directory(&self) -> Result<PathBuf, IdentityError> {
        if !self.root.is_absolute() {
            return Err(IdentityError::UnsafePath);
        }
        if !checked_directory(&self.root)? {
            fs::create_dir_all(&self.root)?;
            if !checked_directory(&self.root)? {
                return Err(IdentityError::UnsafePath);
            }
        }

        let secrets = self.root.join(SECRETS_DIRECTORY);
        if !checked_directory(&secrets)? {
            fs::create_dir_all(&secrets)?;
            if !checked_directory(&secrets)? {
                return Err(IdentityError::UnsafePath);
            }
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&secrets, fs::Permissions::from_mode(0o700))?;
        }

        Ok(secrets)
    }
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

/// Returns `true` for an existing directory and `false` when it is absent.
///
/// A link, reparse point, or non-directory is rejected instead of followed.
fn checked_directory(path: &Path) -> Result<bool, IdentityError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if is_link_or_reparse(&metadata) {
                return Err(IdentityError::SymlinkRejected);
            }
            if !metadata.is_dir() {
                return Err(IdentityError::UnsafePath);
            }
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), IdentityError> {
    let parent = path.parent().ok_or(IdentityError::UnsafePath)?;
    let sequence = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temporary = parent.join(format!(".identity.enc.tmp-{}-{sequence}", std::process::id()));

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let result = (|| -> std::io::Result<()> {
        let mut file = options.open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)?;
        #[cfg(unix)]
        {
            if let Ok(directory) = std::fs::File::open(parent) {
                let _ = directory.sync_all();
            }
        }
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(IdentityError::Io)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{IDENTITY_FILE_NAME, IdentityVault};
    use crate::error::IdentityError;
    use crate::local_key::LocalEncryptionKey;

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TempRoot(std::path::PathBuf);

    impl TempRoot {
        fn new() -> Self {
            let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("nexus-identity-unit-{}-{sequence}", std::process::id()));
            fs::create_dir_all(&path).expect("temp root");
            Self(path)
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn create_load_and_restart_are_stable() {
        let root = TempRoot::new();
        let vault = IdentityVault::new(&root.0).expect("vault");
        let lek = LocalEncryptionKey::new([21_u8; 32]);
        let created = vault.create(&lek).expect("create");
        let id = created.identity_id();

        let restarted = IdentityVault::new(&root.0).expect("vault");
        let loaded = restarted.load(&lek).expect("load");
        assert_eq!(loaded.identity_id(), id);
        assert!(root.0.join("secrets").join(IDENTITY_FILE_NAME).is_file());
        assert!(restarted.create(&lek).is_err());
    }

    #[test]
    fn non_directory_secrets_path_is_rejected() {
        let root = TempRoot::new();
        fs::write(root.0.join("secrets"), b"not a directory").expect("write placeholder");
        let vault = IdentityVault::new(&root.0).expect("vault");
        let lek = LocalEncryptionKey::new([22_u8; 32]);
        assert!(matches!(vault.exists(), Err(IdentityError::UnsafePath)));
        assert!(matches!(vault.load(&lek), Err(IdentityError::UnsafePath)));
    }
}
