@file:Suppress("LongMethod")

package io.nexus.core.client

import io.nexus.model.CoreState
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collect
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import java.io.ByteArrayOutputStream
import java.nio.file.Files
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue

private const val TEST_SECRET_BYTES = 32
private const val IMPORT_BYTES = 2_500_000
private const val TERMINAL_TIMEOUT_MS = 15_000L
private val TERMINAL_EVENTS = setOf("test.completed", "test.failed", "test.cancelled")

/**
 * Runs the shared Stage 08 contract against one native adapter.
 *
 * The C ABI, the Windows JNA adapter, and the JNI adapter all execute this
 * sequence. Every assertion is adapter-independent.
 */
internal fun runNativeContract(
    label: String,
    bridge: NativeBridge,
) = runBlocking {
    assertEquals(NativeCoreClient.ABI_VERSION, bridge.abiVersion, "$label ABI version")
    assertTrue(bridge.version.isNotBlank(), "$label library version")

    verifyAbiMismatchIsRejectedBeforeCreate(label, bridge)
    verifyUnknownHandles(bridge)

    val root = Files.createTempDirectory("nexus-$label-").toAbsolutePath()
    val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    val secret = ByteArray(TEST_SECRET_BYTES) { (it + 1).toByte() }
    val client =
        try {
            NativeCoreClient.create(
                bridge = bridge,
                config =
                    NativeConfig(
                        dataRoot = root.toString(),
                        appVersion = "stage-08",
                        locale = "en-US",
                        enableTestMethods = true,
                    ),
                localSecret = secret,
                scope = scope,
                dispatcher = Dispatchers.Default,
            )
        } catch (error: Throwable) {
            scope.cancel()
            root.toFile().deleteRecursively()
            throw error
        }

    try {
        assertTrue(secret.all { it == 0.toByte() }, "$label must zero the caller's secret")

        client.start()
        client.start()
        awaitState(client, CoreState.READY)

        val echo = "contract-$label"
        val echoed =
            client.invoke(
                NativeCoreClient.TEST_ECHO_METHOD_ID,
                CborValue.textMap("payload" to CborValue.Text(echo)),
            )
        assertEquals(echo, (echoed.mapGet("payload") as CborValue.Text).value, "$label echo")

        val failure =
            assertFailsWith<NexusException.Domain>("$label domain error") {
                client.invoke(
                    NativeCoreClient.TEST_FAULT_METHOD_ID,
                    CborValue.textMap("fault" to CborValue.Text("not_found")),
                )
            }
        assertEquals("not_found", failure.code, "$label domain code")
        assertFalse(failure.retryable, "$label retry policy")

        val panic =
            assertFailsWith<NexusException.Status>("$label panic containment") {
                client.invoke(
                    NativeCoreClient.TEST_FAULT_METHOD_ID,
                    CborValue.textMap("fault" to CborValue.Text("panic")),
                )
            }
        assertEquals(NativeStatus.INTERNAL, panic.status, "$label panic status")
        val afterPanic =
            client.invoke(
                NativeCoreClient.TEST_ECHO_METHOD_ID,
                CborValue.textMap("payload" to CborValue.Text("after-panic")),
            )
        assertEquals("after-panic", (afterPanic.mapGet("payload") as CborValue.Text).value, "$label after panic")

        val events = mutableListOf<NativeEvent>()
        val collector =
            scope.launch {
                client.nativeEvents.collect { event -> synchronized(events) { events.add(event) } }
            }
        try {
            verifyAsyncProgress(client, events, label)
            verifyCancellation(client, events, label)
        } finally {
            collector.cancel()
        }

        verifyStreaming(client, label)

        client.stop()
        client.start()
        awaitState(client, CoreState.READY)
        val restarted =
            client.invoke(
                NativeCoreClient.TEST_ECHO_METHOD_ID,
                CborValue.textMap("payload" to CborValue.Text("restarted")),
            )
        assertEquals("restarted", (restarted.mapGet("payload") as CborValue.Text).value, "$label restart")
    } finally {
        client.close()
        scope.cancel()
        root.toFile().deleteRecursively()
    }
}

private fun verifyAbiMismatchIsRejectedBeforeCreate(
    label: String,
    bridge: NativeBridge,
) {
    val wrongAbi =
        object : NativeBridge by bridge {
            override val abiVersion: Int = NativeCoreClient.ABI_VERSION + 1
        }
    val root = Files.createTempDirectory("nexus-$label-abi-").resolve("must-not-exist")
    val secret = ByteArray(TEST_SECRET_BYTES) { 9 }
    val mismatch =
        assertFailsWith<NexusException.AbiMismatch>("$label ABI mismatch") {
            NativeCoreClient.create(
                bridge = wrongAbi,
                config = NativeConfig(dataRoot = root.toString(), enableTestMethods = true),
                localSecret = secret,
            )
        }
    assertEquals(NativeCoreClient.ABI_VERSION, mismatch.expected, "$label expected ABI")
    assertEquals(NativeCoreClient.ABI_VERSION + 1, mismatch.actual, "$label observed ABI")
    assertTrue(secret.all { it == 0.toByte() }, "$label mismatch must zero the secret")
    assertFalse(Files.exists(root), "$label mismatch must not create the data root")
}

private fun verifyUnknownHandles(bridge: NativeBridge) {
    assertEquals(NativeStatus.INVALID_HANDLE, bridge.start(0xDEAD_BEEF), "unknown start")
    assertEquals(NativeStatus.INVALID_HANDLE, bridge.stop(0xDEAD_BEEF), "unknown stop")
    assertEquals(NativeStatus.INVALID_HANDLE, bridge.destroy(0xDEAD_BEEF), "unknown destroy")
    assertEquals(
        NativeStatus.INVALID_HANDLE,
        bridge.invoke(0xDEAD_BEEF, NativeCoreClient.SYSTEM_INFO_METHOD_ID, ByteArray(0)).status,
        "unknown invoke",
    )
}

private suspend fun verifyAsyncProgress(
    client: NativeCoreClient,
    events: MutableList<NativeEvent>,
    label: String,
) {
    val accepted =
        client.invoke(
            NativeCoreClient.TEST_FAULT_METHOD_ID,
            CborValue.textMap(
                "fault" to CborValue.Text("async_progress"),
                "steps" to CborValue.Unsigned(4u),
                "delay_ms" to CborValue.Unsigned(5u),
            ),
        )
    val opId = (accepted.mapGet("op_id") as CborValue.Unsigned).value.toLong()
    val terminal = awaitTerminal(events, opId)
    assertEquals("test.completed", terminal.type, "$label async terminal")

    val related = synchronized(events) { events.filter { it.opId == opId } }
    assertTrue(related.any { it.type == "test.progress" }, "$label async progress")
    assertTrue(
        related.count { it.type == "test.progress" } in 1..4,
        "$label coalesced progress count",
    )
    assertTrue(related.zipWithNext().all { (left, right) -> left.sequence < right.sequence }, "$label event order")
}

private suspend fun verifyCancellation(
    client: NativeCoreClient,
    events: MutableList<NativeEvent>,
    label: String,
) {
    val accepted =
        client.invoke(
            NativeCoreClient.TEST_FAULT_METHOD_ID,
            CborValue.textMap(
                "fault" to CborValue.Text("async"),
                "steps" to CborValue.Unsigned(100u),
                "delay_ms" to CborValue.Unsigned(20u),
            ),
        )
    val opId = (accepted.mapGet("op_id") as CborValue.Unsigned).value.toLong()
    client.cancel(opId)
    val terminal = awaitTerminal(events, opId)
    assertEquals("test.cancelled", terminal.type, "$label cancellation terminal")
    assertTrue(terminal.sequence > 0, "$label cancelled sequence")
}

private fun verifyStreaming(
    client: NativeCoreClient,
    label: String,
) {
    val data = ByteArray(IMPORT_BYTES) { index -> ((index * 31 + 7) % 251).toByte() }
    val importId = client.importBegin("contract-$label.bin", data.size.toLong())
    var offset = 0
    while (offset < data.size) {
        val end = minOf(offset + NativeCoreClient.MAX_STREAM_CHUNK, data.size)
        client.importWrite(importId, data.copyOfRange(offset, end))
        offset = end
    }
    val summary = client.importFinish(importId)
    val contentId = hexToBytes((summary.mapGet("content_id") as CborValue.Text).value)
    assertEquals(NativeCoreClient.CONTENT_ID_BYTES, contentId.size, "$label content ID length")
    assertEquals(
        data.size.toULong(),
        (summary.mapGet("size") as CborValue.Unsigned).value,
        "$label imported size",
    )
    assertTrue(
        (summary.mapGet("manifest_root") as CborValue.Text).value.isNotBlank(),
        "$label manifest root",
    )

    val exportId = client.exportOpen(contentId)
    val output = ByteArrayOutputStream(data.size)
    try {
        while (true) {
            val chunk = client.exportRead(exportId)
            if (chunk.isEmpty()) {
                break
            }
            assertTrue(chunk.size <= NativeCoreClient.MAX_STREAM_CHUNK, "$label export chunk bound")
            output.write(chunk)
        }
    } finally {
        client.exportClose(exportId)
    }
    assertContentEquals(data, output.toByteArray(), "$label export round trip")
}

private suspend fun awaitState(
    client: NativeCoreClient,
    expected: CoreState,
) {
    withTimeout(TERMINAL_TIMEOUT_MS) {
        while (client.snapshot().state != expected) {
            delay(10)
        }
    }
}

private suspend fun awaitTerminal(
    events: MutableList<NativeEvent>,
    opId: Long,
): NativeEvent =
    withTimeout(TERMINAL_TIMEOUT_MS) {
        var terminal: NativeEvent? = null
        while (terminal == null) {
            terminal =
                synchronized(events) {
                    events.lastOrNull { it.opId == opId && it.type in TERMINAL_EVENTS }
                }
            if (terminal == null) {
                delay(10)
            }
        }
        checkNotNull(terminal)
    }

private fun hexToBytes(value: String): ByteArray {
    require(value.length % 2 == 0) { "hex string must have an even length" }
    return ByteArray(value.length / 2) { index ->
        value.substring(index * 2, index * 2 + 2).toInt(16).toByte()
    }
}
