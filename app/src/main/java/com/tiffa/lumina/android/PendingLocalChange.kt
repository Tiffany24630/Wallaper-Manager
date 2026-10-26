package com.tiffa.lumina.android

import org.json.JSONObject
import java.io.File
import java.security.MessageDigest

data class PendingLocalChange(
    val kind: String,
    val recordId: String,
    val expectedRevision: Long,
    val payload: JSONObject,
    val filePath: String?,
    val fileSize: Long?,
    val fileSha256: String?,
    val deleted: Boolean,
    val resolutionId: String,
) {
    fun matches(conflict: SyncConflict): Boolean =
        kind == conflict.kind && recordId == conflict.id && expectedRevision == conflict.serverRevision

    fun verifyContent(): Boolean {
        if (deleted) return filePath == null && fileSize == null && fileSha256 == null
        if (filePath == null || fileSize == null || fileSha256.isNullOrBlank()) return false
        val file = File(filePath)
        if (!file.isFile || file.length() != fileSize) return false
        val digest = MessageDigest.getInstance("SHA-256")
        file.inputStream().use { input ->
            val buffer = ByteArray(8192)
            var read: Int
            while (input.read(buffer).also { read = it } >= 0) if (read > 0) digest.update(buffer, 0, read)
        }
        return digest.digest().joinToString("") { "%02x".format(it) }.equals(fileSha256, true)
    }

    fun toJson(): JSONObject = JSONObject().apply {
        put("kind", kind); put("record_id", recordId); put("expected_revision", expectedRevision)
        put("payload", payload); put("deleted", deleted); put("resolution_id", resolutionId)
        filePath?.let { put("file_path", it) }
        fileSize?.let { put("file_size", it) }
        fileSha256?.let { put("file_sha256", it) }
    }

    companion object {
        fun fromJson(json: JSONObject): PendingLocalChange = PendingLocalChange(
            json.getString("kind"), json.getString("record_id"), json.getLong("expected_revision"),
            json.optJSONObject("payload") ?: JSONObject(), json.optString("file_path").takeIf { it.isNotBlank() },
            if (json.has("file_size")) json.getLong("file_size") else null,
            json.optString("file_sha256").takeIf { it.isNotBlank() }, json.optBoolean("deleted"), json.getString("resolution_id"),
        )
    }
}
