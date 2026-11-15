package io.nexus.core.client

/** Create-time configuration encoded as the ABI's canonical CBOR map. */
public data class NativeConfig(
    /** Absolute app-private root that owns the database, store, and secrets. */
    public val dataRoot: String,
    /** Host application version shown in diagnostics. */
    public val appVersion: String = "0.0.0",
    /** BCP-47-style locale tag. */
    public val locale: String = "en",
    /** Minimum log level: trace, debug, info, warn, or error. */
    public val logLevel: String = "info",
    /** Whether networking starts with the core. */
    public val networkEnabled: Boolean = false,
    /** Managed storage quota in bytes. */
    public val quotaBytes: ULong = DEFAULT_QUOTA_BYTES,
    /** Whether `test.*` debug methods are exposed. */
    public val enableTestMethods: Boolean = false,
) {
    /** Encodes the configuration using the canonical CBOR profile. */
    public fun encode(): ByteArray =
        encodeCbor(
            CborValue.textMap(
                "abi_version" to CborValue.Unsigned(NativeCoreClient.ABI_VERSION.toULong()),
                "data_root" to CborValue.Text(dataRoot),
                "app_version" to CborValue.Text(appVersion),
                "locale" to CborValue.Text(locale),
                "log_level" to CborValue.Text(logLevel),
                "network_enabled" to CborValue.Bool(networkEnabled),
                "quota_bytes" to CborValue.Unsigned(quotaBytes),
                "enable_test_methods" to CborValue.Bool(enableTestMethods),
            ),
            CborLimits.Config,
        )

    public companion object {
        /** Default managed quota (10 GiB), matching the Rust core. */
        public const val DEFAULT_QUOTA_BYTES: ULong = 10_737_418_240uL
    }
}
