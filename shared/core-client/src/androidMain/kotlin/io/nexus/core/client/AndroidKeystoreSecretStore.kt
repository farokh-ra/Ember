package io.nexus.core.client

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.security.GeneralSecurityException
import java.security.KeyStore
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

private const val SECRET_BYTES = 32
private const val KEY_ALIAS = "ember-local-secret"
private const val PREFERENCES = "ember-core-secrets"
private const val PREFERENCE_KEY = "local_secret"
private const val GCM_TAG_BITS = 128
private const val GCM_IV_BYTES = 12
private const val BYTE_MASK = 0xFF

/**
 * Android Keystore-backed local secret store.
 *
 * The 32-byte key is wrapped by a non-exportable AES-GCM key held by the
 * Android Keystore. The ciphertext lives in app-private preferences. A missing
 * or invalidated Keystore key is an error, not a reason to silently replace an
 * identity that may already exist.
 */
public class AndroidKeystoreSecretStore(
    context: Context,
) : LocalSecretStore {
    private val appContext = context.applicationContext
    private val preferences = appContext.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)

    override fun loadOrCreate(): ByteArray =
        synchronized(this) {
            val stored = preferences.getString(PREFERENCE_KEY, null)
            if (stored != null) {
                return@synchronized decrypt(Base64.decode(stored, Base64.NO_WRAP))
            }
            val secret = ByteArray(SECRET_BYTES).also { SecureRandom().nextBytes(it) }
            val encoded = Base64.encodeToString(encrypt(secret), Base64.NO_WRAP)
            check(preferences.edit().putString(PREFERENCE_KEY, encoded).commit()) {
                "could not persist the local secret"
            }
            secret
        }

    private fun encrypt(plaintext: ByteArray): ByteArray {
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, key())
        val iv = cipher.iv
        check(iv.size == GCM_IV_BYTES) { "unexpected GCM IV length" }
        return byteArrayOf(iv.size.toByte()) + iv + cipher.doFinal(plaintext)
    }

    private fun decrypt(stored: ByteArray): ByteArray {
        require(stored.size > GCM_IV_BYTES) { "local secret file is truncated" }
        val ivLength = stored[0].toInt() and BYTE_MASK
        require(ivLength in 1..GCM_IV_BYTES) { "local secret file has an invalid IV" }
        val iv = stored.copyOfRange(1, 1 + ivLength)
        val ciphertext = stored.copyOfRange(1 + ivLength, stored.size)
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(GCM_TAG_BITS, iv))
        val plaintext =
            try {
                cipher.doFinal(ciphertext)
            } catch (_: GeneralSecurityException) {
                error("the local secret could not be unsealed")
            }
        require(plaintext.size == SECRET_BYTES) { "local secret has an unexpected length" }
        return plaintext
    }

    private fun key(): SecretKey {
        val keyStore = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        val existing = keyStore.getKey(KEY_ALIAS, null) as? SecretKey
        if (existing != null) {
            return existing
        }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        generator.init(
            KeyGenParameterSpec
                .Builder(KEY_ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setRandomizedEncryptionRequired(true)
                .build(),
        )
        return generator.generateKey()
    }
}
