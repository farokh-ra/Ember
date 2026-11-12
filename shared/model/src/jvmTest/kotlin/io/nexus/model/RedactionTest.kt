package io.nexus.model

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class RedactionTest {
    @Test
    fun sensitiveFieldNamesAreDetected() {
        assertTrue(Redaction.isSensitiveField("apiKey"))
        assertTrue(Redaction.isSensitiveField("access_token"))
        assertTrue(Redaction.isSensitiveField("privateKeyPath"))
        assertTrue(Redaction.isSensitiveField("mnemonic"))
        assertFalse(Redaction.isSensitiveField("publicKey"))
        assertFalse(Redaction.isSensitiveField("recordId"))
    }

    @Test
    fun fieldValuesAreRedacted() {
        assertEquals("token=[REDACTED]", Redaction.field("token", "abc123"))
        assertEquals("peerId=peer-1", Redaction.field("peerId", "peer-1"))
    }

    @Test
    fun freeFormTextIsRedacted() {
        val input = "password=hunter2 Authorization: Bearer abc.def.ghi api_key=secret-value"
        val redacted = Redaction.text(input)

        assertFalse(redacted.contains("hunter2"))
        assertFalse(redacted.contains("abc.def.ghi"))
        assertFalse(redacted.contains("secret-value"))
    }
}
