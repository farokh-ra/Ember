package io.nexus.core.client

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

class CborTest {
    @Test
    fun encodesTheRustIntegerAndScalarVectors() {
        assertContentEquals(byteArrayOf(0x00), encodeCbor(CborValue.Unsigned(0u)))
        assertContentEquals(byteArrayOf(0x18, 0x18), encodeCbor(CborValue.Unsigned(24u)))
        assertContentEquals(byteArrayOf(0x19, 0x03, 0xe8.toByte()), encodeCbor(CborValue.Unsigned(1_000u)))
        assertContentEquals(byteArrayOf(0x1a, 0x00, 0x0f, 0x42, 0x40), encodeCbor(CborValue.Unsigned(1_000_000u)))
        assertContentEquals(
            byteArrayOf(0x1b, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00),
            encodeCbor(CborValue.Unsigned(4_294_967_296u)),
        )
        assertContentEquals(byteArrayOf(0x20), encodeCbor(CborValue.Negative(-1)))
        assertContentEquals(byteArrayOf(0x38, 0x63), encodeCbor(CborValue.Negative(-100)))
        assertContentEquals(
            "IETF".encodeToByteArray().let { byteArrayOf(0x64) + it },
            encodeCbor(CborValue.Text("IETF")),
        )
        assertContentEquals(byteArrayOf(0x44, 1, 2, 3, 4), encodeCbor(CborValue.Bytes(byteArrayOf(1, 2, 3, 4))))
        assertContentEquals(
            byteArrayOf(0x83.toByte(), 0x01, 0x02, 0x03),
            encodeCbor(
                CborValue.Array(
                    listOf(
                        CborValue.Unsigned(1u),
                        CborValue.Unsigned(2u),
                        CborValue.Unsigned(3u),
                    ),
                ),
            ),
        )
    }

    @Test
    fun sortsMapKeysByEncodedBytes() {
        val value =
            CborValue.textMap(
                "b" to CborValue.Unsigned(2u),
                "a" to CborValue.Unsigned(1u),
            )
        assertContentEquals(
            byteArrayOf(0xa2.toByte(), 0x61, 'a'.code.toByte(), 0x01, 0x61, 'b'.code.toByte(), 0x02),
            encodeCbor(value),
        )

        val numeric =
            CborValue.Map(
                listOf(
                    CborValue.Unsigned(10u) to CborValue.Bool(true),
                    CborValue.Unsigned(1u) to CborValue.Null,
                ),
            )
        assertContentEquals(
            byteArrayOf(0xa2.toByte(), 0x01, 0xf6.toByte(), 0x0a, 0xf5.toByte()),
            encodeCbor(numeric),
        )
    }

    @Test
    fun decodesCanonicalValuesAndRejectsNonCanonicalInput() {
        val bytes = byteArrayOf(0xa2.toByte(), 0x61, 'a'.code.toByte(), 0x01, 0x61, 'b'.code.toByte(), 0x02)
        val decoded = decodeCbor(bytes)
        assertEquals(1uL, (decoded.mapGet("a") as CborValue.Unsigned).value)
        assertEquals(2uL, (decoded.mapGet("b") as CborValue.Unsigned).value)

        assertFailsWith<CborException> {
            decodeCbor(byteArrayOf(0x18, 0x17))
        }
        assertFailsWith<CborException> {
            decodeCbor(byteArrayOf(0xa2.toByte(), 0x61, 'b'.code.toByte(), 0x02, 0x61, 'a'.code.toByte(), 0x01))
        }
        assertFailsWith<CborException> {
            decodeCbor(byteArrayOf(0x01, 0x02))
        }
    }

    @Test
    fun configEncodesTheDocumentedTextKeys() {
        val config =
            NativeConfig(
                dataRoot = "C:\\nexus-test",
                appVersion = "0.1.0",
                locale = "en-US",
                logLevel = "debug",
                networkEnabled = false,
                quotaBytes = 10uL * 1024uL * 1024uL * 1024uL,
                enableTestMethods = true,
            )
        val decoded = decodeCbor(config.encode(), CborLimits.Config)
        assertEquals(1uL, (decoded.mapGet("abi_version") as CborValue.Unsigned).value)
        assertEquals("C:\\nexus-test", (decoded.mapGet("data_root") as CborValue.Text).value)
        assertEquals("0.1.0", (decoded.mapGet("app_version") as CborValue.Text).value)
        assertEquals("en-US", (decoded.mapGet("locale") as CborValue.Text).value)
        assertEquals("debug", (decoded.mapGet("log_level") as CborValue.Text).value)
        assertEquals(false, (decoded.mapGet("network_enabled") as CborValue.Bool).value)
        assertEquals(true, (decoded.mapGet("enable_test_methods") as CborValue.Bool).value)
        assertTrue((decoded.mapGet("quota_bytes") as CborValue.Unsigned).value > 0uL)
    }
}
