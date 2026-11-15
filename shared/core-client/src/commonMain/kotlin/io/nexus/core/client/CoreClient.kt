package io.nexus.core.client

import io.nexus.core.api.NexusCore

/**
 * Placeholder for the future FFI-backed core client.
 *
 * Stage 03 only establishes the module boundary. The JNI/JNA bridge replaces
 * the delegate in a later stage without changing the `NexusCore` contract.
 */
public class CoreClient(
    private val delegate: NexusCore,
) : NexusCore by delegate {
    public companion object {
        /** Version of the native ABI expected by this client. */
        public const val ABI_VERSION: Int = 1
    }
}
