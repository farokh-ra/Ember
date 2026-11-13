//! XChaCha20-Poly1305 authenticated encryption helpers.

use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use zeroize::Zeroizing;

use crate::error::CryptoError;

/// Length of an XChaCha20-Poly1305 key.
pub const AEAD_KEY_LENGTH: usize = 32;
/// Length of an XChaCha20-Poly1305 nonce.
pub const AEAD_NONCE_LENGTH: usize = 24;
/// Length of the Poly1305 authentication tag.
pub const AEAD_TAG_LENGTH: usize = 16;

/// Encrypts and authenticates a plaintext with associated data.
pub fn encrypt(
    key: &[u8; AEAD_KEY_LENGTH],
    nonce: &[u8; AEAD_NONCE_LENGTH],
    associated_data: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|_| CryptoError::InvalidKeyMaterial)?;
    let nonce = XNonce::try_from(&nonce[..]).map_err(|_| CryptoError::InvalidKeyMaterial)?;
    cipher
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext,
                aad: associated_data,
            },
        )
        .map_err(|_| CryptoError::Encryption)
}

/// Authenticates and decrypts a ciphertext with associated data.
///
/// The returned plaintext is zeroized on drop.
pub fn decrypt(
    key: &[u8; AEAD_KEY_LENGTH],
    nonce: &[u8; AEAD_NONCE_LENGTH],
    associated_data: &[u8],
    ciphertext: &[u8],
) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|_| CryptoError::InvalidKeyMaterial)?;
    let nonce = XNonce::try_from(&nonce[..]).map_err(|_| CryptoError::InvalidKeyMaterial)?;
    let plaintext = cipher
        .decrypt(
            &nonce,
            Payload {
                msg: ciphertext,
                aad: associated_data,
            },
        )
        .map_err(|_| CryptoError::Decryption)?;
    Ok(Zeroizing::new(plaintext))
}

#[cfg(test)]
mod tests {
    use super::{decrypt, encrypt};

    #[test]
    fn round_trips_and_authenticates_context() {
        let key = [3_u8; 32];
        let nonce = [4_u8; 24];
        let ciphertext = encrypt(&key, &nonce, b"header", b"secret payload").expect("encrypt");
        assert_ne!(ciphertext, b"secret payload");
        let plaintext = decrypt(&key, &nonce, b"header", &ciphertext).expect("decrypt");
        assert_eq!(&plaintext[..], b"secret payload");

        assert!(decrypt(&key, &nonce, b"other header", &ciphertext).is_err());
        let mut changed = ciphertext.clone();
        changed[0] ^= 0x01;
        assert!(decrypt(&key, &nonce, b"header", &changed).is_err());
        assert!(decrypt(&[5_u8; 32], &nonce, b"header", &ciphertext).is_err());
    }
}
