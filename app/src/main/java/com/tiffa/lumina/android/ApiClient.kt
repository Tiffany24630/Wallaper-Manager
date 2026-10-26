package com.tiffa.lumina.android

import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.io.FileOutputStream
import java.io.FileInputStream
import java.net.HttpURLConnection
import java.net.URL
import java.net.URLEncoder
import java.security.MessageDigest
import java.util.Locale

class ApiClient(baseUrl: String) {
    var baseUrl: String = normalizeBaseUrl(baseUrl); private set
    var token: String? = null

    fun setBaseUrl(value: String) { baseUrl = normalizeBaseUrl(value) }

    private fun request(method: String, path: String, body: JSONObject? = null, headers: Map<String, String> = emptyMap()): String {
        val connection = URL(baseUrl + path).openConnection() as HttpURLConnection
        connection.requestMethod = method; connection.connectTimeout = 15_000; connection.readTimeout = 30_000; connection.instanceFollowRedirects = false
        connection.setRequestProperty("Accept", "application/json")
        token?.let { connection.setRequestProperty("Authorization", "Bearer $it") }
        headers.forEach { (k, v) -> connection.setRequestProperty(k, v) }
        if (body != null) { connection.doOutput = true; connection.setRequestProperty("Content-Type", "application/json"); connection.outputStream.use { it.write(body.toString().toByteArray()) } }
        val status = connection.responseCode
        val stream = if (status in 200..299) connection.inputStream else connection.errorStream
        val text = stream?.bufferedReader()?.use { it.readText() }.orEmpty()
        if (status !in 200..299) throw ApiException(status, runCatching { JSONObject(text).optString("error", "Error HTTP $status") }.getOrDefault("Error HTTP $status"))
        return text
    }

    fun authenticate(register: Boolean, email: String, password: String): Session {
        val json = JSONObject(request(if (register) "POST" else "POST", if (register) "/v1/auth/register" else "/v1/auth/login", JSONObject().put("email", email).put("password", password)))
        val user = json.getJSONObject("user"); return Session(json.getString("token"), json.getLong("expires_at"), user.getString("id"), user.getString("email"))
    }
    fun session(): JSONObject = JSONObject(request("GET", "/v1/auth/session"))
    fun logout() { request("POST", "/v1/auth/logout") }
    fun registerDevice(id: String, name: String) { request("POST", "/v1/devices", JSONObject().put("id", id).put("name", name).put("platform", "android")) }
    fun devices(): List<Device> = JSONArray(JSONObject(request("GET", "/v1/devices")).optJSONArray("devices")?.toString() ?: "[]").toList { Device(it.getString("id"), it.getString("name"), it.getString("platform"), it.optNullableLong("last_sync_at"), it.optNullableLong("revoked_at")) }
    fun revokeDevice(id: String) { request("DELETE", "/v1/devices/${encodePathSegment(id)}") }

    fun sync(deviceId: String, changes: List<SyncChange> = emptyList()): SyncSnapshot {
        val encodedChanges = JSONArray().apply { changes.forEach { change -> put(JSONObject().apply { put("id", change.id); put("kind", change.kind); put("base_revision", change.baseRevision); put("payload", change.payload); if (change.deleted) put("deleted", true) }) } }
        val json = JSONObject(request("POST", "/v1/sync", JSONObject().put("device_id", deviceId).put("changes", encodedChanges)))
        val records = json.optJSONArray("records").toList { RemoteRecord(it.getString("record_id"), it.getString("kind"), it.getLong("revision"), it.optJSONObject("payload") ?: JSONObject(), it.optInt("deleted") != 0) }
        val files = json.optJSONArray("files").toList { RemoteFile(it.getString("id"), it.getString("record_id"), it.getString("kind"), it.getString("original_name"), it.optString("mime"), it.optString("extension"), it.getLong("size_bytes"), it.getString("sha256")) }
        val conflicts = json.optJSONArray("conflicts").toList { SyncConflict(it.getString("id"), it.getString("kind"), it.getLong("server_revision"), it.optJSONObject("server_payload") ?: JSONObject()) }
        return SyncSnapshot(records, files, conflicts)
    }

    fun publishWallpaper(deviceId: String, recordId: String, file: File, name: String, mime: String, baseRevision: Long, metadataAlreadyCurrent: Boolean = false): PublishResult {
        require(recordId.isNotBlank() && name.isNotBlank() && file.isFile && file.length() > 0) { "El recurso local no es válido" }
        val digest = sha256(file)
        val payload = JSONObject().put("name", name).put("hash", digest).put("size_bytes", file.length()).put("file_type", mime.substringAfter('/', "bin"))
        val metadata = if (metadataAlreadyCurrent) sync(deviceId) else sync(deviceId, listOf(SyncChange(recordId, "wallpaper", baseRevision, payload)))
        if (metadata.conflicts.any { it.id == recordId && it.kind == "wallpaper" }) throw ApiException(409, "La revisión remota cambió; conserva el recurso local y vuelve a sincronizar")
        val record = metadata.records.firstOrNull { it.id == recordId && it.kind == "wallpaper" && !it.deleted }
            ?: throw ApiException(502, "El servidor no confirmó el registro publicado")
        val upload = uploadFile(deviceId, recordId, file, name, mime, digest)
        val confirmed = sync(deviceId).files.firstOrNull { it.id == upload.first && it.recordId == recordId && it.kind == "wallpaper" && it.sha256.equals(digest, true) }
            ?: throw ApiException(502, "El servidor no confirmó el archivo publicado")
        return PublishResult(recordId, record.revision, confirmed.id, confirmed.sha256)
    }

    private fun uploadFile(deviceId: String, recordId: String, file: File, name: String, mime: String, hash: String): Pair<String, String> {
        val init = JSONObject(request("POST", "/v1/files/upload/init", JSONObject().put("device_id", deviceId).put("record_id", recordId).put("kind", "wallpaper").put("name", name).put("mime", mime).put("size", file.length()).put("sha256", hash)))
        if (init.optBoolean("already_exists")) return init.getString("file_id") to init.getString("sha256")
        val uploadId = init.getString("upload_id"); val chunkSize = init.getLong("chunk_size").toInt(); var offset = init.getLong("offset")
        FileInputStream(file).use { input ->
            var skipped = 0L
            while (skipped < offset) { val count = input.skip(offset - skipped); if (count <= 0) throw ApiException(502, "No se pudo reanudar la carga local"); skipped += count }
            val buffer = ByteArray(chunkSize)
            while (offset < file.length()) {
                val wanted = minOf(buffer.size.toLong(), file.length() - offset).toInt(); var read = 0
                while (read < wanted) { val count = input.read(buffer, read, wanted - read); if (count < 0) throw ApiException(502, "La lectura local se interrumpió"); read += count }
                val response = binaryRequest("PUT", "/v1/files/upload/${encodePathSegment(uploadId)}", buffer.copyOf(wanted), mapOf("Content-Type" to "application/octet-stream", "X-Upload-Offset" to offset.toString(), "X-Upload-Length" to wanted.toString()))
                offset = JSONObject(response).getLong("offset")
            }
        }
        val completed = JSONObject(request("POST", "/v1/files/upload/${encodePathSegment(uploadId)}/complete"))
        return completed.getString("file_id") to completed.getString("sha256")
    }

    private fun binaryRequest(method: String, path: String, bytes: ByteArray, headers: Map<String, String>): String {
        val connection = URL(baseUrl + path).openConnection() as HttpURLConnection
        connection.requestMethod = method; connection.connectTimeout = 15_000; connection.readTimeout = 30_000; connection.instanceFollowRedirects = false; connection.doOutput = true
        token?.let { connection.setRequestProperty("Authorization", "Bearer $it") }; headers.forEach { (key, value) -> connection.setRequestProperty(key, value) }
        connection.outputStream.use { it.write(bytes) }
        val status = connection.responseCode; val stream = if (status in 200..299) connection.inputStream else connection.errorStream
        val text = stream?.bufferedReader()?.use { it.readText() }.orEmpty()
        if (status !in 200..299) throw ApiException(status, runCatching { JSONObject(text).optString("error", "Error HTTP $status") }.getOrDefault("Error HTTP $status"))
        return text
    }

    fun download(file: RemoteFile, expectedRevision: Long, output: File, deviceId: String): File {
        output.parentFile?.mkdirs(); val staging = File(output.parentFile, output.name + ".part")
        try {
            val connection = URL(baseUrl + "/v1/files/${encodePathSegment(file.id)}").openConnection() as HttpURLConnection
            connection.requestMethod = "GET"; connection.connectTimeout = 15_000; connection.readTimeout = 60_000; connection.instanceFollowRedirects = false
            token?.let { connection.setRequestProperty("Authorization", "Bearer $it") }
            connection.setRequestProperty("X-Device-ID", deviceId)
            connection.setRequestProperty("X-Sync-Kind", file.kind); connection.setRequestProperty("X-Sync-Record-ID", file.recordId); connection.setRequestProperty("X-Sync-Revision", expectedRevision.toString())
            if (connection.responseCode !in 200..299) throw ApiException(connection.responseCode, "No se pudo descargar el recurso")
            connection.inputStream.use { input -> FileOutputStream(staging).use { outputStream ->
                val buffer = ByteArray(8192); var total = 0L; var read: Int
                while (input.read(buffer).also { read = it } != -1) {
                    total += read
                    if (total > file.size) throw ApiException(422, "La respuesta supera el tamaño anunciado")
                    outputStream.write(buffer, 0, read)
                }
            } }
            if (staging.length() != file.size) throw ApiException(422, "El tamaño descargado no coincide")
            if (sha256(staging) != file.sha256.lowercase()) throw ApiException(422, "El hash descargado no coincide")
            if (output.exists()) {
                if (output.length() == file.size && sha256(output) == file.sha256.lowercase()) { staging.delete(); return output }
                val backup = File(output.parentFile, output.name + ".bak")
                if (backup.exists()) backup.delete()
                if (!output.renameTo(backup)) throw ApiException(500, "No se pudo preparar el reemplazo local")
                if (!staging.renameTo(output)) { backup.renameTo(output); throw ApiException(500, "No se pudo finalizar el recurso descargado") }
                if (!backup.delete()) throw ApiException(500, "El recurso se descargó, pero no se pudo limpiar el respaldo")
            } else if (!staging.renameTo(output)) throw ApiException(500, "No se pudo guardar el recurso descargado")
            return output
        } catch (error: Throwable) { staging.delete(); throw error }
    }

    private fun sha256(file: File): String { val digest = MessageDigest.getInstance("SHA-256"); file.inputStream().use { input -> val buffer = ByteArray(8192); var read: Int; while (input.read(buffer).also { read = it } >= 0) if (read > 0) digest.update(buffer, 0, read) }; return digest.digest().joinToString("") { "%02x".format(it) } }

    companion object {
        fun normalizeBaseUrl(value: String): String {
            val uri = java.net.URI(value.trim().removeSuffix("/"))
            require(uri.scheme == "http" || uri.scheme == "https") { "La URL debe usar HTTP o HTTPS" }
            require(uri.userInfo == null && uri.rawQuery == null && uri.rawFragment == null) { "La URL no puede incluir credenciales, consulta o fragmento" }
            val host = uri.host?.lowercase(Locale.ROOT) ?: error("La URL no tiene host válido")
            if (uri.scheme == "http" && host !in setOf("localhost", "127.0.0.1", "::1", "10.0.2.2")) error("HTTP solo está permitido para desarrollo local")
            return uri.toString().removeSuffix("/")
        }
    }

    private fun encodePathSegment(value: String): String = URLEncoder.encode(value, Charsets.UTF_8.name()).replace("+", "%20")
}

private inline fun <T> JSONArray?.toList(map: (JSONObject) -> T): List<T> { if (this == null) return emptyList(); return (0 until length()).map { map(getJSONObject(it)) } }
private fun JSONObject.optNullableLong(key: String): Long? = if (isNull(key)) null else optLong(key)

