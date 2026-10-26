package com.tiffa.lumina.android

import android.graphics.Color
import android.content.Intent
import android.os.Bundle
import android.widget.*
import java.io.File
import java.io.IOException

class MainActivity : android.app.Activity() {
    private lateinit var repository: LuminaRepository
    private lateinit var root: LinearLayout
    private lateinit var status: TextView
    private var publishing = false
    private val resolvingConflicts = mutableSetOf<String>()

    override fun onCreate(state: Bundle?) { super.onCreate(state); repository = LuminaRepository(this); showLoading(); repository.restore { result -> runOnUiThread { result.fold({ if (it) showHome() else showLogin() }, { showLogin(friendlyError(it)) }) } } }
    override fun onDestroy() { repository.shutdown(); super.onDestroy() }
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode != PICK_WALLPAPER || resultCode != RESULT_OK) return
        val uri = data?.data ?: return
        val mime = contentResolver.getType(uri) ?: "image/png"
        if (mime !in setOf("image/png", "image/jpeg", "image/webp", "image/bmp")) { status.text = "Selecciona una imagen PNG, JPEG, WebP o BMP."; return }
        val extension = when (mime) { "image/jpeg" -> "jpg"; "image/webp" -> "webp"; "image/bmp" -> "bmp"; else -> "png" }
        val source = File(filesDir, "publish/${System.currentTimeMillis()}.$extension")
        try {
            source.parentFile?.mkdirs(); contentResolver.openInputStream(uri).use { input -> requireNotNull(input) { "No se pudo leer la imagen seleccionada" }; source.outputStream().use { output -> input.copyTo(output) } }
        } catch (error: Throwable) { source.delete(); status.text = friendlyError(error); return }
        publishing = true; status.text = "Publicando imagen…"
        repository.publishWallpaper(source, "Android ${source.nameWithoutExtension}", mime) { result -> runOnUiThread { publishing = false; result.fold({ source.delete(); showHome() }, { status.text = friendlyError(it) }) } }
    }

    private fun base(title: String) {
        root = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(24, 24, 24, 24) }
        root.addView(TextView(this).apply { text = title; textSize = 24f; setTextColor(Color.BLACK) })
        status = TextView(this).apply { setPadding(0, 12, 0, 12) }; root.addView(status)
        setContentView(ScrollView(this).apply { addView(root) })
    }
    private fun showLoading() { base("Lumina"); status.text = "Cargando sesión…" }

    private fun showLogin(message: String? = null) {
        base("Lumina · Cuenta")
        val url = EditText(this).apply { hint = "Servidor (HTTPS o localhost)"; setText("http://10.0.2.2:8765") }
        val email = EditText(this).apply { hint = "Correo electrónico"; inputType = 33 }
        val password = EditText(this).apply { hint = "Contraseña"; inputType = 129 }
        root.addView(url); root.addView(email); root.addView(password)
        fun action(register: Boolean) { status.text = "Conectando…"; runCatching { repository.setBaseUrl(url.text.toString()) }.fold({ repository.login(email.text.toString(), password.text.toString(), register) { result -> runOnUiThread { result.fold({ showHome() }, { status.text = if (!register && it is ApiException && it.status == 401) "Credenciales no válidas." else friendlyError(it) }) } } }, { status.text = friendlyError(it) }) }
        root.addView(Button(this).apply { text = "Iniciar sesión"; setOnClickListener { action(false) } })
        root.addView(Button(this).apply { text = "Crear cuenta"; setOnClickListener { action(true) } })
        status.text = message ?: "Usa HTTPS para un servidor remoto. HTTP solo está permitido para desarrollo local."
    }

    private fun showHome() {
        base("Lumina · Cuenta")
        root.addView(Button(this).apply { text = "Publicar imagen"; isEnabled = !publishing; setOnClickListener { startActivityForResult(Intent(Intent.ACTION_OPEN_DOCUMENT).apply { type = "image/*"; addCategory(Intent.CATEGORY_OPENABLE) }, PICK_WALLPAPER) } })
        root.addView(Button(this).apply { text = "Actualizar biblioteca"; setOnClickListener { status.text = "Sincronizando…"; repository.refresh { result -> runOnUiThread { result.fold({ showHome() }, { if (it is ApiException && it.status in 401..403) showLogin(friendlyError(it)) else status.text = friendlyError(it) }) } } } })
        root.addView(Button(this).apply { text = "Biblioteca"; setOnClickListener { showLibrary() } })
        root.addView(Button(this).apply { text = "Playlists"; setOnClickListener { showPlaylists() } })
        root.addView(Button(this).apply { text = "Conflictos"; setOnClickListener { showConflicts() } })
        root.addView(Button(this).apply { text = "Dispositivo y estado"; setOnClickListener { showDevices() } })
        root.addView(Button(this).apply { text = "Cerrar sesión"; setOnClickListener { repository.logout { runOnUiThread { showLogin() } } } })
        status.text = "Sincronizado: ${repository.snapshot.records.size} registros, ${repository.snapshot.files.size} archivos"
    }

    private fun showLibrary() {
        base("Biblioteca sincronizada")
        val files = repository.snapshot.files
        repository.snapshot.records.filter { it.kind == "wallpaper" || it.kind == "cursor" }.forEach { record ->
            val file = files.firstOrNull { it.recordId == record.id && it.kind == record.kind }
            addRecord(record, file)
        }
        if (repository.snapshot.records.none { it.kind == "wallpaper" || it.kind == "cursor" }) status.text = "No hay fondos ni cursores sincronizados."
        else if (files.isEmpty()) status.text = "Los registros están sincronizados, pero no hay archivos disponibles."
        addBack()
    }

    private fun addRecord(record: RemoteRecord, file: RemoteFile?) {
        val box = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(0, 12, 0, 12) }
        box.addView(TextView(this).apply { text = "${record.kind} · ${record.id} · revisión ${record.revision}"; textSize = 16f })
        box.addView(TextView(this).apply { text = "${record.payload.optString("name", "Sin nombre")}${file?.let { " · ${it.size} bytes · ${it.sha256.take(12)}…" } ?: " · archivo no disponible"}" })
        file?.let { resource -> box.addView(Button(this).apply { text = "Descargar en almacenamiento de Lumina"; setOnClickListener { status.text = "Descargando…"; repository.download(resource, record) { result -> runOnUiThread { result.fold({ status.text = "Recurso disponible localmente: ${it.name}" }, { if (it is ApiException && it.status in 401..403) showLogin(friendlyError(it)) else status.text = friendlyError(it) }) } } } }) }
        root.addView(box)
    }

    private fun showPlaylists() {
        base("Playlists")
        val playlists = repository.snapshot.records.filter { it.kind == "playlist" }
        playlists.forEach { playlist -> root.addView(TextView(this).apply { text = "${playlist.payload.optString("name", playlist.id)} · ${playlist.payload.optJSONArray("wallpaper_ids")?.length() ?: 0} elementos"; textSize = 17f; setPadding(0, 12, 0, 12) }) }
        if (playlists.isEmpty()) status.text = "No hay playlists sincronizadas."
        addBack()
    }

    private fun showConflicts() {
        base("Conflicts")
        val conflicts = repository.snapshot.conflicts
        conflicts.forEach { conflict ->
            val key = "${conflict.kind}:${conflict.id}"
            val box = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(0, 12, 0, 12) }
            box.addView(TextView(this).apply { text = "Pending · ${conflict.kind} · ${conflict.id} · remote revision ${conflict.serverRevision}" })
            box.addView(Button(this).apply {
                text = "Accept remote state"
                isEnabled = !resolvingConflicts.contains(key)
                setOnClickListener {
                    if (!resolvingConflicts.add(key)) return@setOnClickListener
                    isEnabled = false; status.text = "Confirming remote state..."
                    repository.acceptRemoteConflict(conflict) { result -> runOnUiThread {
                        resolvingConflicts.remove(key)
                        result.fold({ showConflicts() }, { if (it is ApiException && it.status in 401..403) showLogin(friendlyError(it)) else { status.text = friendlyError(it); showConflicts() } })
                    } }
                }
            })
            root.addView(box)
        }
        status.text = if (conflicts.isEmpty()) "No pending conflicts." else "The remote state is accepted only if its revision is still current. Files remain available for separate download."
        addBack()
    }

    private fun showConflictsLegacy() {
        base("Conflictos")
        val conflicts = repository.snapshot.conflicts
        conflicts.forEach { conflict -> root.addView(TextView(this).apply { text = "Pendiente · ${conflict.kind} · ${conflict.id} · revisión remota ${conflict.serverRevision}"; setPadding(0, 12, 0, 12) }) }
        status.text = if (conflicts.isEmpty()) "No hay conflictos pendientes." else "Los conflictos se conservan pendientes. La resolución segura usa el cliente Windows mientras el backend no exponga un comando remoto de resolución."
        addBack()
    }

    private fun showDevices() {
        base("Dispositivo y sincronización")
        status.text = "Última sincronización: ${if (repository.snapshot.records.isEmpty()) "sin datos" else "completada"}"
        repository.devices.forEach { device ->
            val row = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL }
            row.addView(TextView(this).apply { text = "${device.name} (${device.platform})"; layoutParams = LinearLayout.LayoutParams(0, -2, 1f) })
            row.addView(Button(this).apply { text = "Revocar"; setOnClickListener { repository.revoke(device) { runOnUiThread { it.fold({ showDevices() }, { status.text = friendlyError(it) }) } } } })
            root.addView(row)
        }
        addBack()
    }

    private fun addBack() { root.addView(Button(this).apply { text = "Atrás"; setOnClickListener { showHome() } }) }
    private fun friendlyError(error: Throwable): String = when (error) { is ApiException -> if (error.status in 401..403) "La sesión expiró o no está autorizada." else error.message ?: "Error del servidor"; is IOException -> "No se pudo conectar con el servidor. Comprueba la red y vuelve a intentarlo."; else -> "No se pudo completar la operación" }

    companion object { private const val PICK_WALLPAPER = 1001 }
}

