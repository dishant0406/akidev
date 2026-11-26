package live.akidev.manga.sources

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import uniffi.akimanga.SessionKeeper

/**
 * Keeps each source's session (what its site answered at sign-in) encrypted with a key that lives in the Android Keystore and never
 * leaves it. The password never gets here: the engine gives only the session, and takes it back when a source loads. A session that
 * cannot be read (the key is gone) is simply not there, and the person signs in again.
 */
class SessionKeystore(context: Context) : SessionKeeper {
    private val store = context.getSharedPreferences("sessions", Context.MODE_PRIVATE)

    private fun key(): SecretKey {
        val keys = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (keys.getKey(ALIAS, null) as? SecretKey)?.let { return it }
        val make = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        make.init(
            KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build(),
        )
        return make.generateKey()
    }

    override fun load(source: String): String? = runCatching {
        val sealed = Base64.decode(store.getString(source, null) ?: return null, Base64.NO_WRAP)
        val cipher = Cipher.getInstance(CIPHER).apply { init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, sealed, 0, IV_BYTES)) }
        String(cipher.doFinal(sealed, IV_BYTES, sealed.size - IV_BYTES), Charsets.UTF_8)
    }.getOrNull()

    override fun save(source: String, session: String?) {
        if (session == null) {
            store.edit().remove(source).apply()
            return
        }
        runCatching {
            val cipher = Cipher.getInstance(CIPHER).apply { init(Cipher.ENCRYPT_MODE, key()) }
            val sealed = cipher.iv + cipher.doFinal(session.toByteArray(Charsets.UTF_8))
            store.edit().putString(source, Base64.encodeToString(sealed, Base64.NO_WRAP)).apply()
        }
    }

    private companion object {
        const val ALIAS = "akidev.manga.sessions"
        const val CIPHER = "AES/GCM/NoPadding"
        const val IV_BYTES = 12
    }
}
