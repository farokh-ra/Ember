@file:Suppress(
    "MagicNumber",
    "LongMethod",
    "CyclomaticComplexMethod",
    "ThrowsCount",
    "SwallowedException",
)

package io.nexus.core.client

/**
 * Canonical CBOR value used by the native envelope codec.
 *
 * The profile is intentionally small: definite lengths only, shortest integer
 * encodings, scalar map keys, and map entries sorted by the bytewise
 * lexicographic order of their canonical key encodings. It matches the Rust
 * `nexus-crypto` codec so Kotlin and the core produce identical bytes.
 */
public sealed interface CborValue {
    public data class Unsigned(
        public val value: ULong,
    ) : CborValue

    public data class Negative(
        public val value: Long,
    ) : CborValue {
        init {
            require(value < 0) { "negative CBOR integers must be below zero" }
        }
    }

    public class Bytes(
        public val value: ByteArray,
    ) : CborValue

    public data class Text(
        public val value: String,
    ) : CborValue

    public data class Array(
        public val values: List<CborValue>,
    ) : CborValue

    public data class Map(
        public val entries: List<Pair<CborValue, CborValue>>,
    ) : CborValue

    public data class Bool(
        public val value: Boolean,
    ) : CborValue

    /** CBOR null. */
    public data object Null : CborValue

    public companion object {
        /** Creates a text-keyed map from string keys and values. */
        public fun textMap(vararg entries: Pair<String, CborValue>): Map =
            Map(entries.map { (key, value) -> Text(key) to value })

        /** Creates an empty text-keyed map. */
        public fun emptyMap(): Map = Map(emptyList())

        /** Creates an unsigned integer from a non-negative [Long]. */
        public fun unsigned(value: Long): Unsigned {
            require(value >= 0) { "value must be non-negative" }
            return Unsigned(value.toULong())
        }
    }
}

/** Allocation and recursion bounds for canonical CBOR. */
public data class CborLimits(
    public val maxDepth: Int,
    public val maxItems: Int,
    public val maxInput: Int,
) {
    public companion object {
        /** Default envelope bounds. */
        public val Default: CborLimits = CborLimits(16, 8_192, 1_048_576)

        /** Response bounds, which allow the largest result. */
        public val Response: CborLimits = CborLimits(16, 8_192, 8_388_608)

        /** Create-time configuration bounds. */
        public val Config: CborLimits = CborLimits(16, 1_024, 65_536)
    }
}

/** Canonical CBOR encoding or decoding failure. */
public class CborException(
    message: String,
) : IllegalArgumentException(message)

/** Encodes [value] as canonical CBOR using [limits]. */
public fun encodeCbor(
    value: CborValue,
    limits: CborLimits = CborLimits.Default,
): ByteArray {
    val encoder = CborEncoder(limits)
    val sink = ByteSink()
    encoder.encodeInto(value, 0, sink)
    if (sink.size > limits.maxInput) {
        throw CborException("CBOR output exceeds ${limits.maxInput} bytes")
    }
    return sink.toByteArray()
}

/**
 * Decodes exactly one canonical CBOR value and rejects trailing or
 * non-canonical bytes.
 */
public fun decodeCbor(
    input: ByteArray,
    limits: CborLimits = CborLimits.Default,
): CborValue {
    if (input.size > limits.maxInput) {
        throw CborException("CBOR input exceeds ${limits.maxInput} bytes")
    }
    val decoder = CborDecoder(input, limits)
    val value = decoder.decodeValue(0)
    if (decoder.position != input.size) {
        throw CborException("trailing bytes after the CBOR value")
    }
    val canonical = encodeCbor(value, limits)
    if (!canonical.contentEquals(input)) {
        throw CborException("CBOR input is not canonical")
    }
    return value
}

/** Returns the value associated with a text key, or null. */
public fun CborValue.mapGet(key: String): CborValue? {
    val map = this as? CborValue.Map ?: return null
    return map.entries
        .firstOrNull { (candidate, _) ->
            candidate is CborValue.Text && candidate.value == key
        }?.second
}

/** Returns this unsigned value as an [Int], or null when it does not fit. */
public fun CborValue.asInt(): Int? =
    when (this) {
        is CborValue.Unsigned -> value.toInt().takeIf { value <= Int.MAX_VALUE.toULong() }
        is CborValue.Negative -> value.toInt().takeIf { value >= Int.MIN_VALUE.toLong() }
        else -> null
    }

/** Returns this unsigned value as a [Long], or null when it does not fit. */
public fun CborValue.asLong(): Long? =
    when (this) {
        is CborValue.Unsigned -> value.toLong().takeIf { value <= Long.MAX_VALUE.toULong() }
        is CborValue.Negative -> value
        else -> null
    }

private class CborEncoder(
    private val limits: CborLimits,
) {
    private var items: Int = 0

    fun encodeInto(
        value: CborValue,
        depth: Int,
        sink: ByteSink,
    ) {
        if (depth > limits.maxDepth) {
            throw CborException("CBOR nesting exceeds ${limits.maxDepth} levels")
        }
        items += 1
        if (items > limits.maxItems) {
            throw CborException("CBOR item count exceeds ${limits.maxItems}")
        }

        when (value) {
            is CborValue.Unsigned -> {
                sink.writeHeader(0, value.value)
            }

            is CborValue.Negative -> {
                val encoded = 0uL - (value.value + 1L).toULong()
                sink.writeHeader(1, encoded)
            }

            is CborValue.Bytes -> {
                sink.writeHeader(2, value.value.size.toULong())
                sink.write(value.value)
            }

            is CborValue.Text -> {
                val bytes = value.value.encodeToByteArray()
                sink.writeHeader(3, bytes.size.toULong())
                sink.write(bytes)
            }

            is CborValue.Array -> {
                sink.writeHeader(4, value.values.size.toULong())
                for (item in value.values) {
                    encodeInto(item, depth + 1, sink)
                }
            }

            is CborValue.Map -> {
                encodeMap(value, depth, sink)
            }

            is CborValue.Bool -> {
                sink.writeByte(if (value.value) 0xf5 else 0xf4)
            }

            CborValue.Null -> {
                sink.writeByte(0xf6)
            }
        }

        if (sink.size > limits.maxInput) {
            throw CborException("CBOR output exceeds ${limits.maxInput} bytes")
        }
    }

    private fun encodeMap(
        value: CborValue.Map,
        depth: Int,
        sink: ByteSink,
    ) {
        val encoded = ArrayList<Pair<ByteArray, ByteArray>>(value.entries.size)
        for ((key, item) in value.entries) {
            if (key is CborValue.Array || key is CborValue.Map) {
                throw CborException("CBOR map keys must be scalar values")
            }
            val keySink = ByteSink()
            encodeInto(key, depth + 1, keySink)
            val valueSink = ByteSink()
            encodeInto(item, depth + 1, valueSink)
            encoded.add(keySink.toByteArray() to valueSink.toByteArray())
        }
        encoded.sortWith { left, right -> compareBytes(left.first, right.first) }
        for (index in 1 until encoded.size) {
            if (encoded[index - 1].first.contentEquals(encoded[index].first)) {
                throw CborException("duplicate CBOR map key")
            }
        }
        sink.writeHeader(5, encoded.size.toULong())
        for ((key, item) in encoded) {
            sink.write(key)
            sink.write(item)
        }
    }
}

private class CborDecoder(
    private val input: ByteArray,
    private val limits: CborLimits,
) {
    var position: Int = 0
        private set
    private var items: Int = 0

    fun decodeValue(depth: Int): CborValue {
        if (depth > limits.maxDepth) {
            throw CborException("CBOR nesting exceeds ${limits.maxDepth} levels")
        }
        items += 1
        if (items > limits.maxItems) {
            throw CborException("CBOR item count exceeds ${limits.maxItems}")
        }

        val initial = readByte()
        val major = initial ushr 5
        val additional = initial and 0x1f
        return when (major) {
            0 -> {
                CborValue.Unsigned(readLength(additional))
            }

            1 -> {
                val encoded = readLength(additional)
                if (encoded > Long.MAX_VALUE.toULong()) {
                    throw CborException("negative CBOR integer is out of range")
                }
                CborValue.Negative(-1L - encoded.toLong())
            }

            2 -> {
                CborValue.Bytes(readExact(readLength(additional)))
            }

            3 -> {
                val bytes = readExact(readLength(additional))
                val text =
                    try {
                        bytes.decodeToString(throwOnInvalidSequence = true)
                    } catch (error: IllegalArgumentException) {
                        throw CborException("invalid UTF-8 text: ${error.message}")
                    }
                CborValue.Text(text)
            }

            4 -> {
                val length = readLength(additional)
                if (length > remaining().toULong()) {
                    throw CborException("unexpected end of CBOR array")
                }
                val values = ArrayList<CborValue>(length.toInt())
                repeat(length.toInt()) {
                    values.add(decodeValue(depth + 1))
                }
                CborValue.Array(values)
            }

            5 -> {
                val length = readLength(additional)
                if (length > (remaining() / 2).toULong()) {
                    throw CborException("unexpected end of CBOR map")
                }
                val entries = ArrayList<Pair<CborValue, CborValue>>(length.toInt())
                val seen = HashSet<ByteArrayKey>()
                repeat(length.toInt()) {
                    val key = decodeValue(depth + 1)
                    if (key is CborValue.Array || key is CborValue.Map) {
                        throw CborException("CBOR map keys must be scalar values")
                    }
                    val keyBytes = encodeCbor(key, limits)
                    if (!seen.add(ByteArrayKey(keyBytes))) {
                        throw CborException("duplicate CBOR map key")
                    }
                    entries.add(key to decodeValue(depth + 1))
                }
                CborValue.Map(entries)
            }

            6 -> {
                throw CborException("CBOR tags are not supported")
            }

            7 -> {
                when (additional) {
                    20 -> CborValue.Bool(false)
                    21 -> CborValue.Bool(true)
                    22 -> CborValue.Null
                    23 -> throw CborException("CBOR undefined is not supported")
                    in 24..27 -> throw CborException("CBOR floats and simple values are not supported")
                    31 -> throw CborException("indefinite lengths are not supported")
                    else -> throw CborException("CBOR simple value is not supported")
                }
            }

            else -> {
                throw CborException("reserved CBOR major type")
            }
        }
    }

    private fun readLength(additional: Int): ULong =
        when (additional) {
            in 0..23 -> {
                additional.toULong()
            }

            24 -> {
                val value = readByte().toULong()
                if (value < 24uL) {
                    throw CborException("non-shortest CBOR integer")
                }
                value
            }

            25 -> {
                val value = ((readByte().toULong() shl 8) or readByte().toULong())
                if (value <= 0xffuL) {
                    throw CborException("non-shortest CBOR integer")
                }
                value
            }

            26 -> {
                val value = (
                    (readByte().toULong() shl 24) or
                        (readByte().toULong() shl 16) or
                        (readByte().toULong() shl 8) or
                        readByte().toULong()
                )
                if (value <= 0xffffuL) {
                    throw CborException("non-shortest CBOR integer")
                }
                value
            }

            27 -> {
                var value = 0uL
                repeat(8) {
                    value = (value shl 8) or readByte().toULong()
                }
                if (value <= 0xffff_ffffuL) {
                    throw CborException("non-shortest CBOR integer")
                }
                value
            }

            28, 29, 30 -> {
                throw CborException("reserved CBOR additional information")
            }

            else -> {
                throw CborException("indefinite lengths are not supported")
            }
        }

    private fun readByte(): Int {
        if (position >= input.size) {
            throw CborException("unexpected end of CBOR input")
        }
        return input[position++].toInt() and 0xff
    }

    private fun readExact(length: ULong): ByteArray {
        if (length > Int.MAX_VALUE.toULong()) {
            throw CborException("CBOR item is too large")
        }
        val count = length.toInt()
        val end = position + count
        if (end < position || end > input.size) {
            throw CborException("unexpected end of CBOR input")
        }
        val bytes = input.copyOfRange(position, end)
        position = end
        return bytes
    }

    private fun remaining(): Int = input.size - position
}

private class ByteArrayKey(
    private val bytes: ByteArray,
) {
    override fun equals(other: Any?): Boolean = other is ByteArrayKey && bytes.contentEquals(other.bytes)

    override fun hashCode(): Int = bytes.contentHashCode()
}

private fun compareBytes(
    left: ByteArray,
    right: ByteArray,
): Int {
    val shared = minOf(left.size, right.size)
    for (index in 0 until shared) {
        val difference = (left[index].toInt() and 0xff) - (right[index].toInt() and 0xff)
        if (difference != 0) {
            return difference
        }
    }
    return left.size - right.size
}

private class ByteSink(
    initialCapacity: Int = 64,
) {
    private var buffer = ByteArray(initialCapacity)
    var size: Int = 0
        private set

    fun writeByte(value: Int) {
        ensure(1)
        buffer[size] = value.toByte()
        size += 1
    }

    fun write(bytes: ByteArray) {
        ensure(bytes.size)
        bytes.copyInto(buffer, size)
        size += bytes.size
    }

    fun writeHeader(
        major: Int,
        length: ULong,
    ) {
        val prefix = major shl 5
        when {
            length < 24uL -> {
                writeByte(prefix or length.toInt())
            }

            length <= 0xffuL -> {
                writeByte(prefix or 24)
                writeByte(length.toInt())
            }

            length <= 0xffffuL -> {
                writeByte(prefix or 25)
                writeByte(length.toInt() ushr 8)
                writeByte(length.toInt())
            }

            length <= 0xffff_ffffuL -> {
                writeByte(prefix or 26)
                writeByte((length shr 24).toInt())
                writeByte((length shr 16).toInt())
                writeByte((length shr 8).toInt())
                writeByte(length.toInt())
            }

            else -> {
                writeByte(prefix or 27)
                for (shift in 56 downTo 0 step 8) {
                    writeByte((length shr shift).toInt())
                }
            }
        }
    }

    fun toByteArray(): ByteArray = buffer.copyOf(size)

    private fun ensure(additional: Int) {
        if (size + additional <= buffer.size) {
            return
        }
        var capacity = buffer.size * 2
        while (capacity < size + additional) {
            capacity *= 2
        }
        buffer = buffer.copyOf(capacity)
    }
}
