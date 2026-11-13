//! Domain-separated SHA-256 hashing.

use sha2::{Digest, Sha256};

use crate::domain::DomainTag;

/// Returns the SHA-256 digest of the supplied bytes.
#[must_use]
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

/// Returns SHA-256 over `domain_tag || bytes`.
#[must_use]
pub fn domain_sha256(tag: DomainTag, bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(tag.as_bytes());
    hasher.update(bytes);
    hasher.finalize().into()
}

/// Encodes bytes as lowercase hexadecimal.
#[must_use]
pub fn to_hex(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::{domain_sha256, sha256, to_hex};
    use crate::domain::DomainTag;

    #[test]
    fn sha256_matches_known_vectors() {
        assert_eq!(
            to_hex(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            to_hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn domain_hashing_is_separated() {
        assert_ne!(
            domain_sha256(DomainTag::Publication, b"same"),
            domain_sha256(DomainTag::Profile, b"same")
        );
    }

    #[test]
    fn committed_publication_id_vector_is_exact() {
        assert_eq!(
            to_hex(&domain_sha256(DomainTag::PublicationId, b"nexus-stage-06")),
            "c5109ddb870d0325411cee8a934308fde01b3995fbeb1d67f2db5b5fdac625f1"
        );
    }
}
