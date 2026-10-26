package com.tiffa.lumina.android

import android.content.Context
import java.io.File
import java.security.MessageDigest
import java.util.UUID
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors

class LuminaRepository(context: Context) {
    private val appContext = context.applicationContext
    private val secureSession = SecureSessionStore(appContext)
    private val preferences = appContext.getSharedPreferences("lumina_android", Context.MODE_PRIVATE)
    private val executor: ExecutorService = Executors.newSingleThreadExecutor()
    private val client = ApiClient(preferences.getString("base_url", "http://10.0.2.2:8765") ?: "http://10.0.2.2:8765")
    private val deviceId = preferences.getString("device_id", null) ?: UUID.randomUUID().toString().also { preferences.edit().putString("device_id", it).apply() }
    @Volatile private var pendingStore: PendingLocalChangeStore? = null
    @Volatile var snapshot: SyncSnapshot = SyncSnapshot(emptyList(), emptyList(), emptyList()); private set
    @Volatile var devices: List<Device> = emptyList(); private set

    fun setBaseUrl(url: String) { client.setBaseUrl(url); preferences.edit().putString("base_url", client.baseUrl).apply() }
    fun storedSession(): Session? = secureSession.load().getOrNull()

    fun login(email: String, password: String, register: Boolean, callback: (Result<Unit>) -> Unit) = executor.execute {
        runCatching { val session = client.authenticate(register, email.trim(), password); secureSession.save(session); client.token = session.token; pendingStore = PendingLocalChangeStore(appContext, session.userId); client.registerDevice(deviceId, "Lumina Android"); refreshInternal() }.fold({ callback(Result.success(Unit)) }, { callback(Result.failure(it)) })
    }

    fun restore(callback: (Result<Boolean>) -> Unit) = executor.execute {
        val loaded = secureSession.load()
        if (loaded.isFailure) { clearSession(); return@execute callback(Result.failure(IllegalStateException("No se pudo recuperar la sesión cifrada", loaded.exceptionOrNull()))) }
        val session = loaded.getOrNull() ?: return@execute callback(Result.success(false))
        client.token = session.token
        runCatching { client.session(); pendingStore = PendingLocalChangeStore(appContext, session.userId); client.registerDevice(deviceId, "Lumina Android"); refreshInternal(); true }.fold({ callback(Result.success(it)) }, { if (it is ApiException && it.status in 401..403) clearSession(); callback(Result.failure(it)) })
    }

    fun refresh(callback: (Result<SyncSnapshot>) -> Unit) = executor.execute { runCatching { refreshInternal() }.fold({ callback(Result.success(it)) }, { if (it is ApiException && it.status in 401..403) clearSession(); callback(Result.failure(it)) }) }
    fun acceptRemoteConflict(conflict: SyncConflict, callback: (Result<Unit>) -> Unit) = executor.execute {
        runCatching { requireConfirmed(client.resolveConflict(deviceId, conflict, keepRemote = true), conflict); refreshInternal() }
            .fold({ callback(Result.success(Unit)) }, { if (it is ApiException && it.status in 401..403) clearSession(); callback(Result.failure(it)) })
    }
    fun keepLocalAvailable(conflict: SyncConflict): Boolean = pendingStore?.find(conflict)?.let { pending ->
        pending.verifyContent() && (pending.deleted || pending.filePath == null || File(pending.filePath).isFile)
    } == true
    fun acceptLocalConflict(conflict: SyncConflict, callback: (Result<Unit>) -> Unit) = executor.execute {
        runCatching {
            val store = pendingStore ?: error("No hay una cuenta autenticada")
            val pending = store.find(conflict) ?: error("El cambio local pendiente ya no estÃ¡ disponible")
            require(pending.matches(conflict) && pending.verifyContent()) { "El contenido local pendiente no estÃ¡ disponible o cambiÃ³" }
            val preparedUploadId = if (pending.filePath != null && !pending.deleted) {
                client.prepareWallpaperConflict(deviceId, conflict, File(pending.filePath), pending.payload.optString("name", "wallpaper.png"), "image/${pending.payload.optString("file_type", "png")}", pending.resolutionId)
            } else null
            requireConfirmed(client.resolveConflict(deviceId, conflict, keepRemote = false, localPayload = pending.payload, resolutionId = pending.resolutionId, deleted = pending.deleted, preparedUploadId = preparedUploadId), conflict)
            refreshInternal()
            store.remove(pending)
        }.fold({ callback(Result.success(Unit)) }, { if (it is ApiException && it.status in 401..403) clearSession(); callback(Result.failure(it)) })
    }
    fun revoke(device: Device, callback: (Result<Unit>) -> Unit) = executor.execute { runCatching { client.revokeDevice(device.id); devices = client.devices() }.fold({ callback(Result.success(Unit)) }, { if (it is ApiException && it.status in 401..403) clearSession(); callback(Result.failure(it)) }) }
    fun download(file: RemoteFile, record: RemoteRecord, callback: (Result<File>) -> Unit) = executor.execute {
        val safeName = "${file.kind}_${file.recordId}_${file.sha256.take(12)}.${file.extension.trimStart('.')}".replace(Regex("[^A-Za-z0-9._-]"), "_")
        runCatching { client.download(file, record.revision, File(appContext.filesDir, "remote-resources/$safeName"), deviceId) }.fold({ callback(Result.success(it)) }, { if (it is ApiException && it.status in 401..403) clearSession(); callback(Result.failure(it)) })
    }
    fun publishWallpaper(source: File, name: String, mime: String, callback: (Result<PublishResult>) -> Unit) = executor.execute {
        runCatching {
            require(source.isFile && source.length() > 0) { "El archivo seleccionado no es válido" }
            val hash = sha256(source)
            val pendingHash = preferences.getString("pending_publish_hash", null)
            val knownHash = pendingHash ?: preferences.getString("last_publish_hash", null)
            val knownId = preferences.getString(if (pendingHash == hash) "pending_publish_id" else "last_publish_id", null)
            val snapshot = client.sync(deviceId)
            val reusable = findReusableWallpaper(snapshot, hash, source.length(), knownId)
            val recordId = reusable?.id ?: if (knownHash == hash) knownId ?: UUID.randomUUID().toString() else UUID.randomUUID().toString()
            preferences.edit().putString("pending_publish_hash", hash).putString("pending_publish_id", recordId).apply()
            val current = snapshot.records.firstOrNull { it.id == recordId && it.kind == "wallpaper" }
            val unchanged = current != null && !current.deleted && current.payload.optString("hash").equals(hash, true) && current.payload.optLong("size_bytes", -1L) == source.length()
            val payload = org.json.JSONObject().put("name", name).put("hash", hash).put("size_bytes", source.length()).put("file_type", mime.substringAfter('/', "bin"))
            val existingConflict = snapshot.conflicts.firstOrNull { it.id == recordId && it.kind == "wallpaper" }
            if (existingConflict != null) {
                rememberLocalConflict(source, recordId, mime, payload, existingConflict.serverRevision)
                throw ApiException(409, "La revisiÃ³n remota cambiÃ³; el cambio local quedÃ³ pendiente")
            }
            val result = try {
                client.publishWallpaper(deviceId, recordId, source, name, mime, current?.revision ?: 0L, unchanged)
            } catch (error: ApiException) {
                if (error.status == 409) {
                    val latest = runCatching { client.sync(deviceId) }.getOrNull()
                    val conflict = latest?.conflicts?.firstOrNull { it.id == recordId && it.kind == "wallpaper" }
                    if (conflict != null) rememberLocalConflict(source, recordId, mime, payload, conflict.serverRevision)
                }
                throw error
            }
            preferences.edit().remove("pending_publish_hash").remove("pending_publish_id").putString("last_publish_hash", hash).putString("last_publish_id", recordId).apply()
            refreshInternal()
            result
        }.fold({ callback(Result.success(it)) }, { if (it is ApiException && it.status in 401..403) clearSession(); callback(Result.failure(it)) })
    }

    fun logout(callback: () -> Unit) = executor.execute { runCatching { client.logout() }; clearSession(); callback() }
    fun clearSession() { client.token = null; secureSession.clear(); pendingStore = null; snapshot = SyncSnapshot(emptyList(), emptyList(), emptyList()); devices = emptyList() }
    fun shutdown() { executor.shutdownNow() }

    private fun refreshInternal(): SyncSnapshot { val result = client.sync(deviceId); snapshot = result; devices = client.devices(); preferences.edit().putLong("last_sync_at", System.currentTimeMillis()).apply(); return result }

    private fun requireConfirmed(response: org.json.JSONObject, conflict: SyncConflict) {
        require(response.optBoolean("resolved", false)) { "El servidor no confirmÃ³ la resoluciÃ³n" }
        require(response.optString("kind") == conflict.kind && response.optString("record_id") == conflict.id) { "La respuesta no corresponde al conflicto seleccionado" }
    }

    private fun rememberLocalConflict(source: File, recordId: String, mime: String, payload: org.json.JSONObject, expectedRevision: Long) {
        val store = pendingStore ?: error("No hay una cuenta autenticada")
        require(payload.optString("hash").isNotBlank() && payload.optLong("size_bytes", -1L) == source.length()) { "Los metadatos locales no son vÃ¡lidos" }
        val extension = mime.substringAfter('/', "bin").replace(Regex("[^A-Za-z0-9]"), "")
        val hash = payload.getString("hash").lowercase()
        val directory = File(appContext.filesDir, "pending-conflicts").apply { mkdirs() }
        val target = File(directory, "wallpaper_${recordId.replace(Regex("[^A-Za-z0-9._-]"), "_")}_${hash.take(16)}.$extension")
        val temporary = File(directory, ".${target.name}.part")
        if (!target.isFile || target.length() != source.length() || sha256(target) != hash) {
            source.inputStream().use { input -> temporary.outputStream().use { output -> input.copyTo(output) } }
            require(temporary.length() == source.length() && sha256(temporary) == hash) { "No se pudo verificar el contenido local" }
            check(temporary.renameTo(target)) { "No se pudo conservar el contenido local pendiente" }
        }
        val resolutionId = store.find("wallpaper", recordId, expectedRevision)?.resolutionId ?: UUID.randomUUID().toString()
        store.save(PendingLocalChange("wallpaper", recordId, expectedRevision, payload, target.absolutePath, target.length(), hash, false, resolutionId))
    }

    private fun sha256(file: File): String { val digest = MessageDigest.getInstance("SHA-256"); file.inputStream().use { input -> val buffer = ByteArray(8192); var read: Int; while (input.read(buffer).also { read = it } >= 0) if (read > 0) digest.update(buffer, 0, read) }; return digest.digest().joinToString("") { "%02x".format(it) } }
}

internal fun findReusableWallpaper(snapshot: SyncSnapshot, hash: String, size: Long, preferredId: String?): RemoteRecord? {
    val candidates = snapshot.records.asSequence().filter { record ->
        record.kind == "wallpaper" && !record.deleted &&
            record.payload.optString("hash").equals(hash, true) &&
            record.payload.optLong("size_bytes", -1L) == size &&
            snapshot.files.any { file ->
                file.recordId == record.id && file.kind == "wallpaper" &&
                    file.sha256.equals(hash, true) && file.size == size
            }
    }
    return candidates.firstOrNull { it.id == preferredId } ?: candidates.firstOrNull()
}

