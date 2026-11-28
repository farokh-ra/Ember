package io.nexus.core.client

/**
 * Platform-sealed storage for the 32-byte local encryption key (LEK).
 *
 * Implementations must never fall back to plaintext. If the platform secure
 * store is unavailable, [loadOrCreate] throws and the core stays unavailable
 * rather than creating an identity that cannot be protected at rest.
 */
public interface LocalSecretStore {
    /**
     * Returns the existing 32-byte key or creates, seals, and persists a new
     * one. The caller owns the returned array and should zero it after use.
     */
    public fun loadOrCreate(): ByteArray
}
