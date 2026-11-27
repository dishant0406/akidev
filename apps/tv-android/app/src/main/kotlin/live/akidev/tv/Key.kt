package live.akidev.tv

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.io.File
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * The viewer's TorBox key. It is kept encrypted with a key that lives in the Android Keystore, in the app's private storage,
 * and goes nowhere but to TorBox. A debug build also reads `files/torbox.key` once (pushed with adb `run-as`) so a test can set it.
 */
object Key {
    private const val ALIAS = "akidev_torbox"
    private const val TRANSFORM = "AES/GCM/NoPadding"

    private fun prefs(context: Context) = context.getSharedPreferences("akidev", Context.MODE_PRIVATE)

    /** The saved key, or "" when there is none (or it cannot be read: the viewer types it again). */
    fun get(context: Context): String {
        if (BuildConfig.DEBUG) importTestKey(context)
        val saved = prefs(context).getString("torbox_key", null) ?: return ""
        return try {
            val bytes = Base64.decode(saved, Base64.NO_WRAP)
            val cipher = Cipher.getInstance(TRANSFORM)
            cipher.init(Cipher.DECRYPT_MODE, secret(), GCMParameterSpec(128, bytes.copyOfRange(0, 12)))
            String(cipher.doFinal(bytes, 12, bytes.size - 12))
        } catch (e: Exception) {
            ""
        }
    }

    /** False when this TV cannot keep the key safely. */
    fun set(context: Context, key: String): Boolean = try {
        val cipher = Cipher.getInstance(TRANSFORM)
        cipher.init(Cipher.ENCRYPT_MODE, secret())
        val bytes = cipher.iv + cipher.doFinal(key.trim().toByteArray())
        prefs(context).edit().putString("torbox_key", Base64.encodeToString(bytes, Base64.NO_WRAP)).apply()
        true
    } catch (e: Exception) {
        false
    }

    /** Forgets the saved key (the encryption key in the Keystore stays; it opens nothing without it). */
    fun clear(context: Context) = prefs(context).edit().remove("torbox_key").apply()

    private fun secret(): SecretKey {
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (store.getKey(ALIAS, null) as? SecretKey)?.let { return it }
        val spec = KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .build()
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore").apply { init(spec) }.generateKey()
    }

    private fun importTestKey(context: Context) {
        val file = File(context.filesDir, "torbox.key")
        if (file.exists() && set(context, file.readText())) file.delete()
    }
}
