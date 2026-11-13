//! Protocol domain separation constants.
//!
//! Every signature and domain-separated hash uses exactly one of these
//! contexts. Changing a context value is a protocol major-version change.

/// Domain tag for a signed publication body.
pub const PUBLICATION: &[u8] = b"NEXUS-PUB-v1";
/// Domain tag for a signed profile body.
pub const PROFILE: &[u8] = b"NEXUS-PROFILE-v1";
/// Domain tag for a signed tombstone body.
pub const TOMBSTONE: &[u8] = b"NEXUS-TOMBSTONE-v1";
/// Domain tag for a publication ID hash.
pub const PUBLICATION_ID: &[u8] = b"NEXUS-PUB-ID-v1";
/// Domain tag for a chunk-manifest root hash.
pub const MANIFEST: &[u8] = b"NEXUS-MANIFEST-v1";
/// Domain tag for backup authenticated-data construction.
pub const BACKUP: &[u8] = b"NEXUS-BACKUP-v1";
/// Info string for deriving the identity-at-rest key from the LEK.
pub const IDENTITY_KEY: &[u8] = b"NEXUS-IDENTITY-KEY-v1";
/// Domain tag for the publication DHT key.
pub const DHT_PUBLICATION: &[u8] = b"NEXUS-DHT-PUB-v1";
/// Domain tag for the tombstone DHT key.
pub const DHT_TOMBSTONE: &[u8] = b"NEXUS-DHT-TOMB-v1";
/// Domain tag for the profile DHT key.
pub const DHT_PROFILE: &[u8] = b"NEXUS-DHT-PROFILE-v1";

/// A signature or hash context owned by one protocol record type.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DomainTag {
    /// Signed publication record.
    Publication,
    /// Signed profile record.
    Profile,
    /// Signed tombstone record.
    Tombstone,
    /// Publication ID derivation.
    PublicationId,
    /// Chunk-manifest root derivation.
    Manifest,
    /// Backup encryption associated data.
    Backup,
    /// Identity-at-rest key derivation.
    IdentityKey,
    /// Publication DHT key derivation.
    DhtPublication,
    /// Tombstone DHT key derivation.
    DhtTombstone,
    /// Profile DHT key derivation.
    DhtProfile,
}

impl DomainTag {
    /// Returns the exact protocol context bytes.
    #[must_use]
    pub const fn as_bytes(self) -> &'static [u8] {
        match self {
            Self::Publication => PUBLICATION,
            Self::Profile => PROFILE,
            Self::Tombstone => TOMBSTONE,
            Self::PublicationId => PUBLICATION_ID,
            Self::Manifest => MANIFEST,
            Self::Backup => BACKUP,
            Self::IdentityKey => IDENTITY_KEY,
            Self::DhtPublication => DHT_PUBLICATION,
            Self::DhtTombstone => DHT_TOMBSTONE,
            Self::DhtProfile => DHT_PROFILE,
        }
    }

    /// Returns the context as ASCII, for diagnostics and tests.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Publication => "NEXUS-PUB-v1",
            Self::Profile => "NEXUS-PROFILE-v1",
            Self::Tombstone => "NEXUS-TOMBSTONE-v1",
            Self::PublicationId => "NEXUS-PUB-ID-v1",
            Self::Manifest => "NEXUS-MANIFEST-v1",
            Self::Backup => "NEXUS-BACKUP-v1",
            Self::IdentityKey => "NEXUS-IDENTITY-KEY-v1",
            Self::DhtPublication => "NEXUS-DHT-PUB-v1",
            Self::DhtTombstone => "NEXUS-DHT-TOMB-v1",
            Self::DhtProfile => "NEXUS-DHT-PROFILE-v1",
        }
    }
}

/// Builds the exact `domain_tag || body` byte sequence that is signed.
#[must_use]
pub fn signing_message(tag: DomainTag, body: &[u8]) -> Vec<u8> {
    let mut message = Vec::with_capacity(tag.as_bytes().len() + body.len());
    message.extend_from_slice(tag.as_bytes());
    message.extend_from_slice(body);
    message
}

#[cfg(test)]
mod tests {
    use super::{DomainTag, signing_message};

    #[test]
    fn domain_tags_are_distinct_and_exact() {
        assert_eq!(DomainTag::Publication.as_bytes(), b"NEXUS-PUB-v1");
        assert_eq!(DomainTag::Profile.as_str(), "NEXUS-PROFILE-v1");
        assert_eq!(signing_message(DomainTag::Profile, b"body"), b"NEXUS-PROFILE-v1body");
        let tags = [
            DomainTag::Publication,
            DomainTag::Profile,
            DomainTag::Tombstone,
            DomainTag::PublicationId,
            DomainTag::Manifest,
            DomainTag::Backup,
            DomainTag::IdentityKey,
            DomainTag::DhtPublication,
            DomainTag::DhtTombstone,
            DomainTag::DhtProfile,
        ];
        for (index, left) in tags.iter().enumerate() {
            for right in &tags[index + 1..] {
                assert_ne!(left.as_bytes(), right.as_bytes());
            }
        }
    }
}
