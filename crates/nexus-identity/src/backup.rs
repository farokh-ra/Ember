//! Encrypted, user-controlled identity backup format.

use nexus_crypto::{
    AEAD_NONCE_LENGTH, AEAD_TAG_LENGTH, ARGON2_SALT_LENGTH, Argon2idParams, DomainTag, IdentityId,
    MAX_ARGON2_ITERATIONS, MAX_ARGON2_MEMORY_KIB, MAX_ARGON2_PARALLELISM, argon2id_key, decrypt, encrypt,
    normalize_text_bytes, random_array,
};
use zeroize::Zeroizing;

use crate::error::IdentityError;
use crate::identity::Identity;
use crate::payload::{PAYLOAD_LENGTH, decode as decode_payload, encode as encode_payload};

/// Backup magic.
pub const BACKUP_MAGIC: [u8; 8] = *b"NEXBKUP1";
/// Backup format version.
pub const BACKUP_FORMAT_VERSION: u16 = 1;
/// KDF identifier for Argon2id.
pub const BACKUP_KDF_ARGON2ID: u8 = 1;
/// Argon2 version 1.3.
pub const BACKUP_ARGON2_VERSION: u32 = 0x13;
/// v1 default Argon2id memory cost in kibibytes (64 MiB).
pub const BACKUP_DEFAULT_MEMORY_KIB: u32 = 65_536;
/// v1 default Argon2id iterations.
pub const BACKUP_DEFAULT_ITERATIONS: u32 = 3;
/// v1 default Argon2id lanes.
pub const BACKUP_DEFAULT_PARALLELISM: u32 = 1;
/// Minimum passphrase length enforced when creating a backup.
pub const PASSPHRASE_MIN_CHARS: usize = 12;
/// Maximum passphrase length in UTF-8 bytes.
pub const PASSPHRASE_MAX_BYTES: usize = 1_024;
/// Maximum accepted backup file size.
pub const BACKUP_MAX_FILE_LENGTH: usize = 4_096;

const FIXED_HEADER_LENGTH: usize = 8 + 2 + 1 + 1 + 4 + 4 + 4 + 4 + ARGON2_SALT_LENGTH + AEAD_NONCE_LENGTH + 1;
const VERSION_OFFSET: usize = 8;
const KDF_OFFSET: usize = 10;
const FLAGS_OFFSET: usize = 11;
const ARGON_VERSION_OFFSET: usize = 12;
const MEMORY_OFFSET: usize = 16;
const ITERATIONS_OFFSET: usize = 20;
const PARALLELISM_OFFSET: usize = 24;
const SALT_OFFSET: usize = 28;
const NONCE_OFFSET: usize = 44;
const ID_LENGTH_OFFSET: usize = 68;
const ID_OFFSET: usize = 69;

/// Returns the v1 default Argon2id parameters.
#[must_use]
pub const fn v1_params() -> Argon2idParams {
    Argon2idParams::new(
        BACKUP_DEFAULT_MEMORY_KIB,
        BACKUP_DEFAULT_ITERATIONS,
        BACKUP_DEFAULT_PARALLELISM,
    )
}

/// Creates an encrypted backup with the v1 default Argon2id parameters.
pub fn export_backup(identity: &Identity, passphrase: &str) -> Result<Vec<u8>, IdentityError> {
    export_backup_with_params(identity, passphrase, v1_params(), None)
}

/// Creates an encrypted backup with explicit parameters and optional salts.
///
/// Production callers must use [`export_backup`] or pass [`v1_params`]. The
/// explicit form exists for Stage 06 vectors and future Android calibration.
pub fn export_backup_with_params(
    identity: &Identity,
    passphrase: &str,
    params: Argon2idParams,
    fixed_material: Option<([u8; ARGON2_SALT_LENGTH], [u8; AEAD_NONCE_LENGTH])>,
) -> Result<Vec<u8>, IdentityError> {
    params.validate()?;
    let normalized = normalize_passphrase(passphrase, true)?;
    let (salt, nonce) = match fixed_material {
        Some(material) => material,
        None => (random_array()?, random_array()?),
    };
    let identity_id = identity.identity_id().as_str().to_owned();
    let header = build_header(&identity_id, params, &salt, &nonce)?;
    let key = argon2id_key(normalized.as_bytes(), &salt, params)?;
    let payload = encode_payload(identity);
    let mut associated_data = Vec::with_capacity(header.len() + DomainTag::Backup.as_bytes().len());
    associated_data.extend_from_slice(&header);
    associated_data.extend_from_slice(DomainTag::Backup.as_bytes());
    let ciphertext = encrypt(&key, &nonce, &associated_data, &payload)?;

    let mut file = Vec::with_capacity(header.len() + ciphertext.len());
    file.extend_from_slice(&header);
    file.extend_from_slice(&ciphertext);
    if file.len() > BACKUP_MAX_FILE_LENGTH {
        return Err(IdentityError::MalformedBackup);
    }
    Ok(file)
}

/// Restores an encrypted backup.
///
/// A wrong passphrase, tampered ciphertext, or inconsistent identity metadata
/// returns the single generic [`IdentityError::BackupUnreadable`] error.
pub fn restore_backup(backup: &[u8], passphrase: &str) -> Result<Identity, IdentityError> {
    let header = parse_header(backup)?;
    let normalized = normalize_passphrase(passphrase, false).map_err(|_| IdentityError::BackupUnreadable)?;
    let key = argon2id_key(normalized.as_bytes(), &header.salt, header.params)
        .map_err(|_| IdentityError::BackupUnreadable)?;
    let mut associated_data = Vec::with_capacity(header.header.len() + DomainTag::Backup.as_bytes().len());
    associated_data.extend_from_slice(header.header);
    associated_data.extend_from_slice(DomainTag::Backup.as_bytes());
    let plaintext = decrypt(&key, &header.nonce, &associated_data, &backup[header.header.len()..])
        .map_err(|_| IdentityError::BackupUnreadable)?;
    let payload = decode_payload(&plaintext).map_err(|_| IdentityError::BackupUnreadable)?;
    let identity = Identity::from_seed(*payload.keypair.secret_seed(), payload.created_at_ms);
    if identity.identity_id() != header.identity_id {
        return Err(IdentityError::BackupUnreadable);
    }
    Ok(identity)
}

struct ParsedHeader<'backup> {
    header: &'backup [u8],
    params: Argon2idParams,
    salt: [u8; ARGON2_SALT_LENGTH],
    nonce: [u8; AEAD_NONCE_LENGTH],
    identity_id: IdentityId,
}

fn build_header(
    identity_id: &str,
    params: Argon2idParams,
    salt: &[u8; ARGON2_SALT_LENGTH],
    nonce: &[u8; AEAD_NONCE_LENGTH],
) -> Result<Vec<u8>, IdentityError> {
    let id_bytes = identity_id.as_bytes();
    if id_bytes.len() > 128 {
        return Err(IdentityError::InvalidMetadata);
    }
    let mut header = Vec::with_capacity(FIXED_HEADER_LENGTH + id_bytes.len());
    header.extend_from_slice(&BACKUP_MAGIC);
    header.extend_from_slice(&BACKUP_FORMAT_VERSION.to_le_bytes());
    header.push(BACKUP_KDF_ARGON2ID);
    header.push(0);
    header.extend_from_slice(&BACKUP_ARGON2_VERSION.to_le_bytes());
    header.extend_from_slice(&params.memory_kib.to_le_bytes());
    header.extend_from_slice(&params.iterations.to_le_bytes());
    header.extend_from_slice(&params.parallelism.to_le_bytes());
    header.extend_from_slice(salt);
    header.extend_from_slice(nonce);
    header.push(u8::try_from(id_bytes.len()).map_err(|_| IdentityError::InvalidMetadata)?);
    header.extend_from_slice(id_bytes);
    Ok(header)
}

fn parse_header(backup: &[u8]) -> Result<ParsedHeader<'_>, IdentityError> {
    if backup.len() > BACKUP_MAX_FILE_LENGTH || backup.len() < FIXED_HEADER_LENGTH {
        return Err(IdentityError::MalformedBackup);
    }
    if backup[..8] != BACKUP_MAGIC {
        return Err(IdentityError::MalformedBackup);
    }
    let version = u16::from_le_bytes([backup[VERSION_OFFSET], backup[VERSION_OFFSET + 1]]);
    if version != BACKUP_FORMAT_VERSION {
        return Err(IdentityError::UnsupportedBackupVersion);
    }
    if backup[KDF_OFFSET] != BACKUP_KDF_ARGON2ID || backup[FLAGS_OFFSET] != 0 {
        return Err(IdentityError::UnsupportedBackupVersion);
    }
    let argon_version = u32::from_le_bytes(
        backup[ARGON_VERSION_OFFSET..ARGON_VERSION_OFFSET + 4]
            .try_into()
            .map_err(|_| IdentityError::MalformedBackup)?,
    );
    if argon_version != BACKUP_ARGON2_VERSION {
        return Err(IdentityError::UnsupportedBackupVersion);
    }
    let params = Argon2idParams::new(
        u32::from_le_bytes(
            backup[MEMORY_OFFSET..MEMORY_OFFSET + 4]
                .try_into()
                .map_err(|_| IdentityError::MalformedBackup)?,
        ),
        u32::from_le_bytes(
            backup[ITERATIONS_OFFSET..ITERATIONS_OFFSET + 4]
                .try_into()
                .map_err(|_| IdentityError::MalformedBackup)?,
        ),
        u32::from_le_bytes(
            backup[PARALLELISM_OFFSET..PARALLELISM_OFFSET + 4]
                .try_into()
                .map_err(|_| IdentityError::MalformedBackup)?,
        ),
    );
    if params.memory_kib > MAX_ARGON2_MEMORY_KIB
        || params.iterations > MAX_ARGON2_ITERATIONS
        || params.parallelism > MAX_ARGON2_PARALLELISM
        || params.validate().is_err()
    {
        return Err(IdentityError::UnsupportedBackupVersion);
    }

    let id_length = usize::from(backup[ID_LENGTH_OFFSET]);
    let header_length = FIXED_HEADER_LENGTH
        .checked_add(id_length)
        .ok_or(IdentityError::MalformedBackup)?;
    if id_length == 0 || header_length > backup.len() {
        return Err(IdentityError::MalformedBackup);
    }
    let identity_id_text =
        std::str::from_utf8(&backup[ID_OFFSET..header_length]).map_err(|_| IdentityError::MalformedBackup)?;
    let identity_id = IdentityId::parse(identity_id_text).map_err(|_| IdentityError::MalformedBackup)?;

    let ciphertext_length = backup.len() - header_length;
    if ciphertext_length != PAYLOAD_LENGTH + AEAD_TAG_LENGTH {
        return Err(IdentityError::MalformedBackup);
    }

    let salt = backup[SALT_OFFSET..SALT_OFFSET + ARGON2_SALT_LENGTH]
        .try_into()
        .map_err(|_| IdentityError::MalformedBackup)?;
    let nonce = backup[NONCE_OFFSET..NONCE_OFFSET + AEAD_NONCE_LENGTH]
        .try_into()
        .map_err(|_| IdentityError::MalformedBackup)?;

    Ok(ParsedHeader {
        header: &backup[..header_length],
        params,
        salt,
        nonce,
        identity_id,
    })
}

fn normalize_passphrase(passphrase: &str, enforce_minimum: bool) -> Result<Zeroizing<String>, IdentityError> {
    let normalized =
        normalize_text_bytes(passphrase, PASSPHRASE_MAX_BYTES).map_err(|_| IdentityError::PassphraseTooLong)?;
    if normalized.is_empty() {
        return Err(IdentityError::EmptyPassphrase);
    }
    if enforce_minimum && normalized.chars().count() < PASSPHRASE_MIN_CHARS {
        return Err(IdentityError::WeakPassphrase {
            min: PASSPHRASE_MIN_CHARS,
        });
    }
    Ok(Zeroizing::new(normalized))
}

#[cfg(test)]
mod tests {
    use super::{BACKUP_FORMAT_VERSION, BACKUP_MAGIC, export_backup_with_params, restore_backup, v1_params};
    use crate::identity::Identity;
    use nexus_crypto::Argon2idParams;

    fn fast_params() -> Argon2idParams {
        Argon2idParams::new(8, 1, 1)
    }

    #[test]
    fn round_trips_with_fixed_header_material() {
        let identity = Identity::from_seed([31_u8; 32], 1_234);
        let backup = export_backup_with_params(
            &identity,
            "correct horse battery staple",
            fast_params(),
            Some(([4_u8; 16], [5_u8; 24])),
        )
        .expect("export");
        assert_eq!(&backup[..8], &BACKUP_MAGIC);
        assert_eq!(u16::from_le_bytes([backup[8], backup[9]]), BACKUP_FORMAT_VERSION);
        assert_eq!(&backup[12..16], &0x13_u32.to_le_bytes());
        assert_eq!(&backup[16..20], &8_u32.to_le_bytes());
        assert_eq!(&backup[20..24], &1_u32.to_le_bytes());
        assert_eq!(&backup[24..28], &1_u32.to_le_bytes());
        assert_eq!(&backup[28..44], &[4_u8; 16]);
        assert_eq!(&backup[44..68], &[5_u8; 24]);

        let restored = restore_backup(&backup, "correct horse battery staple").expect("restore");
        assert_eq!(restored.identity_id(), identity.identity_id());
        assert_eq!(restored.created_at_ms(), 1_234);
    }

    #[test]
    fn wrong_passphrase_is_generic() {
        let identity = Identity::from_seed([32_u8; 32], 1);
        let backup =
            export_backup_with_params(&identity, "correct horse battery staple", fast_params(), None).expect("export");
        let error = restore_backup(&backup, "wrong horse battery staple").expect_err("reject");
        assert_eq!(
            error.to_string(),
            "backup could not be opened; the passphrase or file is invalid"
        );
    }

    #[test]
    fn v1_defaults_are_64_mib_three_iterations_one_lane() {
        let params = v1_params();
        assert_eq!(params.memory_kib, 65_536);
        assert_eq!(params.iterations, 3);
        assert_eq!(params.parallelism, 1);
    }
}
