package com.tiffa.lumina.android

import android.content.Context
import java.io.File
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
    @Volatile var snapshot: SyncSnapshot = SyncSnapshot(emptyList(), emptyList(), emptyList()); private set
    @Volatile var devices: List<Device> = emptyList(); private set

    fun setBaseUrl(url: String) { client.setBaseUrl(url); preferences.edit().putString("base_url", client.baseUrl).apply() }
    fun storedSession(): Session? = secureSession.load().getOrNull()

    fun login(email: String, password: String, register: Boolean, callback: (Result<Unit>) -> Unit) = executor.execute {
        runCatching { val session = client.authenticate(register, email.trim(), password); secureSession.save(session); client.token = session.token; client.registerDevice(deviceId, "Lumina Android"); refreshInternal() }.fold({ callback(Result.success(Unit)) }, { callback(Result.failure(it)) })
    }

    fun restore(callback: (Result<Boolean>) -> Unit) = executor.execute {
        val loaded = secureSession.load()
        if (loaded.isFailure) { clearSession(); return@execute callback(Result.failure(IllegalStateException("No se pudo recuperar la sesión cifrada", loaded.exceptionOrNull()))) }
        val session = loaded.getOrNull() ?: return@execute callback(Result.success(false))
        client.token = session.token
        runCatching { client.session(); client.registerDevice(deviceId, "Lumina Android"); refreshInternal(); true }.fold({ callback(Result.success(it)) }, { if (it is ApiException && it.status in 401..403) clearSession(); callback(Result.failure(it)) })
    }

    fun refresh(callback: (Result<SyncSnapshot>) -> Unit) = executor.execute { runCatching { refreshInternal() }.fold({ callback(Result.success(it)) }, { if (it is ApiException && it.status in 401..403) clearSession(); callback(Result.failure(it)) }) }
    fun revoke(device: Device, callback: (Result<Unit>) -> Unit) = executor.execute { runCatching { client.revokeDevice(device.id); devices = client.devices() }.fold({ callback(Result.success(Unit)) }, { if (it is ApiException && it.status in 401..403) clearSession(); callback(Result.failure(it)) }) }
    fun download(file: RemoteFile, record: RemoteRecord, callback: (Result<File>) -> Unit) = executor.execute {
        val safeName = "${file.kind}_${file.recordId}_${file.sha256.take(12)}.${file.extension.trimStart('.')}".replace(Regex("[^A-Za-z0-9._-]"), "_")
        runCatching { client.download(file, record.revision, File(appContext.filesDir, "remote-resources/$safeName"), deviceId) }.fold({ callback(Result.success(it)) }, { if (it is ApiException && it.status in 401..403) clearSession(); callback(Result.failure(it)) })
    }

    fun logout(callback: () -> Unit) = executor.execute { runCatching { client.logout() }; clearSession(); callback() }
    fun clearSession() { client.token = null; secureSession.clear(); snapshot = SyncSnapshot(emptyList(), emptyList(), emptyList()); devices = emptyList() }
    fun shutdown() { executor.shutdownNow() }

    private fun refreshInternal(): SyncSnapshot { val result = client.sync(deviceId); snapshot = result; devices = client.devices(); preferences.edit().putLong("last_sync_at", System.currentTimeMillis()).apply(); return result }
}

