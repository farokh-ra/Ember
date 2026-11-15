package io.nexus.model

/**
 * Pure redaction helpers for debug output.
 *
 * Logging code must call these helpers before emitting values that may contain
 * credentials. See `docs/LOGGING_POLICY.md`.
 */
public object Redaction {
    /** Replacement text used for any sensitive value. */
    public const val REDACTED: String = "[REDACTED]"

    private val sensitiveTokens: Set<String> =
        setOf(
            "password",
            "passphrase",
            "secret",
            "token",
            "apikey",
            "privatekey",
            "signingkey",
            "seed",
            "mnemonic",
            "authorization",
            "cookie",
            "credential",
            "pin",
            "keystore",
        )

    private val bearerPattern: Regex = Regex("""(?i)\bBearer\s+[A-Za-z0-9._~+/=-]+""")
    private val assignmentPattern: Regex =
        Regex(
            """(?i)\b(password|passphrase|secret|token|api[_-]?key|private[_-]?key|seed|mnemonic)""" +
                """\s*[:=]\s*("[^"]*"|'[^']*'|\S+)""",
        )

    /** Returns true when a field name conventionally carries secret material. */
    public fun isSensitiveField(name: String): Boolean {
        val tokens =
            name
                .replace(Regex("([a-z0-9])([A-Z])"), "$1 $2")
                .split(Regex("[^A-Za-z0-9]+"))
                .filter { it.isNotEmpty() }
                .map { it.lowercase() }

        return tokens.any { it in sensitiveTokens } ||
            tokens.zipWithNext().any { (first, second) -> "$first$second" in sensitiveTokens }
    }

    /** Formats a field, replacing its value when the field is sensitive. */
    public fun field(
        name: String,
        value: String,
    ): String = if (isSensitiveField(name)) "$name=$REDACTED" else "$name=$value"

    /** Redacts common credential shapes embedded in free-form debug text. */
    public fun text(input: String): String =
        input
            .replace(bearerPattern, "Bearer $REDACTED")
            .replace(assignmentPattern) { match ->
                "${match.groupValues[1]}=$REDACTED"
            }
}
