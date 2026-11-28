package io.nexus.core.client

import com.sun.jna.Library
import com.sun.jna.Memory
import com.sun.jna.Native
import com.sun.jna.Pointer
import com.sun.jna.Structure
import com.sun.jna.ptr.PointerByReference
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.StandardCopyOption
import java.security.SecureRandom

private const val SECRET_BYTES = 32
private const val CRYPTPROTECT_UI_FORBIDDEN = 0x1
private const val FILE_MAGIC = "EMB1"

/** DPAPI-backed local secret store used by the Windows desktop shell. */
public class DpapiSecretStore(
    dataRoot: String,
) : LocalSecretStore {
    private val directory: Path = Path.of(dataRoot)
    private val secretFile: Path = directory.resolve("local-secret.bin")

    override fun loadOrCreate(): ByteArray {
        require(isWindows()) { "DPAPI is only available on Windows" }
        Files.createDirectories(directory)
        if (Files.isRegularFile(secretFile)) {
            return unseal(Files.readAllBytes(secretFile))
        }
        val secret = ByteArray(SECRET_BYTES).also { SecureRandom().nextBytes(it) }
        writeAtomically(secretFile, FILE_MAGIC.toByteArray() + seal(secret))
        return secret
    }

    private fun seal(plaintext: ByteArray): ByteArray {
        val input = Memory(plaintext.size.toLong())
        input.write(0, plaintext, 0, plaintext.size)
        val inputBlob =
            DataBlob().apply {
                cbData = plaintext.size
                pbData = input
            }
        val outputBlob = DataBlob()
        val ok =
            crypt32.CryptProtectData(
                inputBlob,
                null,
                null,
                null,
                null,
                CRYPTPROTECT_UI_FORBIDDEN,
                outputBlob,
            )
        if (!ok) {
            error("DPAPI could not protect the local secret")
        }
        outputBlob.read()
        return copyAndFree(outputBlob)
    }

    private fun unseal(file: ByteArray): ByteArray {
        require(file.size > FILE_MAGIC.length) { "local secret file is truncated" }
        val magic = file.copyOfRange(0, FILE_MAGIC.length).toString(Charsets.US_ASCII)
        require(magic == FILE_MAGIC) { "local secret file has an unknown format" }
        val sealed = file.copyOfRange(FILE_MAGIC.length, file.size)
        val input = Memory(sealed.size.toLong())
        input.write(0, sealed, 0, sealed.size)
        val inputBlob =
            DataBlob().apply {
                cbData = sealed.size
                pbData = input
            }
        val outputBlob = DataBlob()
        val description = PointerByReference()
        val ok =
            crypt32.CryptUnprotectData(
                inputBlob,
                description,
                null,
                null,
                null,
                CRYPTPROTECT_UI_FORBIDDEN,
                outputBlob,
            )
        if (!ok) {
            error("DPAPI could not unseal the local secret")
        }
        outputBlob.read()
        val plaintext = copyAndFree(outputBlob)
        require(plaintext.size == SECRET_BYTES) { "local secret has an unexpected length" }
        return plaintext
    }

    private fun copyAndFree(blob: DataBlob): ByteArray {
        val pointer = blob.pbData ?: return ByteArray(0)
        return try {
            pointer.getByteArray(0, blob.cbData)
        } finally {
            crypt32.LocalFree(pointer)
        }
    }

    private fun writeAtomically(
        target: Path,
        bytes: ByteArray,
    ) {
        val temporary = Files.createTempFile(directory, "local-secret", ".tmp")
        try {
            Files.write(temporary, bytes)
            Files.move(
                temporary,
                target,
                StandardCopyOption.ATOMIC_MOVE,
                StandardCopyOption.REPLACE_EXISTING,
            )
        } finally {
            Files.deleteIfExists(temporary)
        }
    }

    private fun isWindows(): Boolean = System.getProperty("os.name").startsWith("Windows", ignoreCase = true)

    @Structure.FieldOrder("cbData", "pbData")
    public open class DataBlob : Structure {
        @JvmField
        public var cbData: Int = 0

        @JvmField
        public var pbData: Pointer? = null

        public constructor() : super()

        public constructor(pointer: Pointer?) : super(pointer)
    }

    @Suppress("FunctionNaming", "ktlint:standard:function-naming", "LongParameterList")
    private interface Crypt32 : Library {
        fun CryptProtectData(
            dataIn: DataBlob,
            description: Pointer?,
            entropy: DataBlob?,
            reserved: Pointer?,
            prompt: Pointer?,
            flags: Int,
            dataOut: DataBlob,
        ): Boolean

        fun CryptUnprotectData(
            dataIn: DataBlob,
            description: PointerByReference?,
            entropy: DataBlob?,
            reserved: Pointer?,
            prompt: Pointer?,
            flags: Int,
            dataOut: DataBlob,
        ): Boolean

        fun LocalFree(memory: Pointer): Pointer
    }

    private companion object {
        private val crypt32: Crypt32 by lazy { Native.load("crypt32", Crypt32::class.java) }
    }
}
