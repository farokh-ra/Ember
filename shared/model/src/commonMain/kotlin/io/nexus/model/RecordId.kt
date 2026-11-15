package io.nexus.model

/** Stable identifier for a locally indexed record. */
@JvmInline
public value class RecordId(
    public val value: String,
) {
    init {
        require(value.isNotBlank()) { "RecordId must not be blank" }
    }

    override fun toString(): String = value
}
