package io.nexus.core.client

import io.nexus.core.api.CoreAvailability
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class CoreHostTest {
    private val config = NativeConfig(dataRoot = "/tmp/ember-core-host-test", appVersion = "0.1.0-test")

    @Test
    fun aMissingLibraryLeavesTheShellRunning() =
        runBlocking {
            val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
            val host =
                CoreHost(
                    bridgeFactory = { throw UnsatisfiedLinkError("nexus_core") },
                    config = config,
                    secretStore = FakeSecretStore(),
                    scope = scope,
                )
            try {
                host.start()

                assertEquals(CoreAvailability.UNAVAILABLE, host.status.value.availability)
                assertEquals("native library is not available", host.status.value.detail)
                assertFalse(host.isStarted)
            } finally {
                host.close()
                scope.cancel()
            }
        }

    @Test
    fun anAbiMismatchIsReportedBeforeTheCoreOpens() =
        runBlocking {
            val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
            val host =
                CoreHost(
                    bridgeFactory = { FakeBridge(abiVersion = NativeCoreClient.ABI_VERSION + 1) },
                    config = config,
                    secretStore = FakeSecretStore(),
                    scope = scope,
                )
            try {
                host.start()

                assertEquals(CoreAvailability.UNAVAILABLE, host.status.value.availability)
                assertEquals(
                    "ABI mismatch: expected ${NativeCoreClient.ABI_VERSION}, found ${NativeCoreClient.ABI_VERSION + 1}",
                    host.status.value.detail,
                )
                assertFalse(host.isStarted)
            } finally {
                host.close()
                scope.cancel()
            }
        }

    @Test
    fun aReadyEventPublishesTheProtocolVersion() =
        runBlocking {
            val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
            val bridge = FakeBridge(events = listOf(coreStateEvent("ready")))
            val host =
                CoreHost(bridgeFactory = { bridge }, config = config, secretStore = FakeSecretStore(), scope = scope)
            try {
                host.start()

                val ready = withTimeout(5_000) { host.status.first { it.availability == CoreAvailability.READY } }
                assertEquals("1.0", ready.protocolVersion)
                assertTrue(host.isStarted)
            } finally {
                host.close()
                scope.cancel()
            }
        }

    @Test
    fun closeReleasesTheNativeHandle() =
        runBlocking {
            val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
            val bridge = FakeBridge(events = listOf(coreStateEvent("ready")))
            val host =
                CoreHost(bridgeFactory = { bridge }, config = config, secretStore = FakeSecretStore(), scope = scope)
            try {
                host.start()
                withTimeout(5_000) { host.status.first { it.availability == CoreAvailability.READY } }

                host.close()

                assertEquals(CoreAvailability.UNAVAILABLE, host.status.value.availability)
                assertTrue(bridge.destroyed)
                assertFalse(host.isStarted)
            } finally {
                scope.cancel()
            }
        }

    @Test
    fun invokingBeforeStartFailsExplicitly() =
        runBlocking {
            val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
            val host =
                CoreHost(
                    bridgeFactory = { FakeBridge() },
                    config = config,
                    secretStore = FakeSecretStore(),
                    scope = scope,
                )
            try {
                assertFailsWith<IllegalStateException> { host.invoke(NativeCoreClient.SYSTEM_INFO_METHOD_ID) }
                Unit
            } finally {
                host.close()
                scope.cancel()
            }
        }

    private class FakeSecretStore : LocalSecretStore {
        override fun loadOrCreate(): ByteArray = ByteArray(32) { 7 }
    }

    private class FakeBridge(
        private val events: List<ByteArray> = emptyList(),
        override val abiVersion: Int = NativeCoreClient.ABI_VERSION,
    ) : NativeBridge {
        override val version: String = "0.1.0-test"

        @Volatile
        var destroyed: Boolean = false

        private val pending = ArrayDeque(events)

        override fun create(
            config: ByteArray,
            localSecret: ByteArray,
        ): NativeHandleResult {
            assertTrue(localSecret.size == 32)
            return NativeHandleResult(NativeStatus.OK, HANDLE)
        }

        override fun start(handle: Long): NativeStatus = NativeStatus.OK

        override fun stop(handle: Long): NativeStatus = NativeStatus.OK

        override fun destroy(handle: Long): NativeStatus {
            destroyed = true
            return NativeStatus.OK
        }

        override fun invoke(
            handle: Long,
            methodId: Int,
            request: ByteArray,
        ): NativeBytesResult =
            NativeBytesResult(
                NativeStatus.OK,
                encodeCbor(
                    CborValue.textMap(
                        "ok" to CborValue.Bool(true),
                        "result" to CborValue.textMap("protocol_version" to CborValue.Text("1.0")),
                    ),
                    CborLimits.Default,
                ),
            )

        override fun nextEvent(
            handle: Long,
            timeoutMs: Int,
        ): NativeBytesResult {
            val next = pending.removeFirstOrNull() ?: return NativeBytesResult(NativeStatus.QUEUE_CLOSED)
            return NativeBytesResult(NativeStatus.OK, next)
        }

        override fun cancel(
            handle: Long,
            opId: Long,
        ): NativeStatus = NativeStatus.OK

        override fun importBegin(
            handle: Long,
            displayName: ByteArray,
            expectedSize: Long,
        ): NativeHandleResult = NativeHandleResult(NativeStatus.OK, 1)

        override fun importWrite(
            handle: Long,
            importId: Long,
            data: ByteArray,
        ): NativeStatus = NativeStatus.OK

        override fun importFinish(
            handle: Long,
            importId: Long,
        ): NativeBytesResult = NativeBytesResult(NativeStatus.OK)

        override fun importAbort(
            handle: Long,
            importId: Long,
        ): NativeStatus = NativeStatus.OK

        override fun exportOpen(
            handle: Long,
            contentId: ByteArray,
        ): NativeHandleResult = NativeHandleResult(NativeStatus.OK, 1)

        override fun exportRead(
            handle: Long,
            exportId: Long,
            maxLen: Int,
        ): NativeBytesResult = NativeBytesResult(NativeStatus.OK)

        override fun exportClose(
            handle: Long,
            exportId: Long,
        ): NativeStatus = NativeStatus.OK

        private companion object {
            private const val HANDLE: Long = 42
        }
    }

    private companion object {
        private fun coreStateEvent(state: String): ByteArray =
            encodeCbor(
                CborValue.textMap(
                    "seq" to CborValue.Unsigned(1u),
                    "ts" to CborValue.Unsigned(0u),
                    "type" to CborValue.Text("core.state"),
                    "data" to CborValue.textMap("state" to CborValue.Text(state)),
                ),
                CborLimits.Default,
            )
    }
}
