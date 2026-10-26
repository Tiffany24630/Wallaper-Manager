package com.tiffa.lumina.android

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import org.json.JSONObject
import java.nio.charset.StandardCharsets
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

class SecureSessionStore(context: Context) {
    private val prefs = context.getSharedPreferences("lumina_secure_session", Context.MODE_PRIVATE)
    private val alias = "com.tiffa.lumina.android.session"

    private fun key(): SecretKey {
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (store.getKey(alias, null) as? SecretKey)?.let { return it }
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore").apply {
            init(KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM).setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE).build())
        }.generateKey()
    }

    fun save(session: Session) {
        val cipher = Cipher.getInstance("AES/GCM/NoPadding").apply { init(Cipher.ENCRYPT_MODE, key()) }
        val plain = JSONObject().put("token", session.token).put("expires_at", session.expiresAt)
            .put("user_id", session.userId).put("email", session.email).toString().toByteArray(StandardCharsets.UTF_8)
        prefs.edit().putString("value", Base64.encodeToString(cipher.iv, Base64.NO_WRAP) + ":" + Base64.encodeToString(cipher.doFinal(plain), Base64.NO_WRAP)).apply()
    }

    fun load(): Result<Session?> = runCatching {
        val parts = prefs.getString("value", null)?.split(":") ?: return@runCatching null
        require(parts.size == 2) { "Sesión cifrada inválida" }
        val cipher = Cipher.getInstance("AES/GCM/NoPadding").apply { init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, Base64.decode(parts[0], Base64.DEFAULT))) }
        val json = JSONObject(String(cipher.doFinal(Base64.decode(parts[1], Base64.DEFAULT)), StandardCharsets.UTF_8))
        Session(json.getString("token"), json.getLong("expires_at"), json.getString("user_id"), json.getString("email"))
    }

    fun clear() { prefs.edit().clear().apply() }
}

