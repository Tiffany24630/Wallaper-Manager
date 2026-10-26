package com.tiffa.lumina.android

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import java.io.BufferedInputStream
import java.io.File
import java.net.ServerSocket
import java.net.Socket
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.security.MessageDigest
import java.util.concurrent.Executors

class ApiClientIntegrationTest {
    private lateinit var server: LocalHttpServer
    private lateinit var client: ApiClient
    private lateinit var tempDir: File

    @Before fun setUp() {
        server = LocalHttpServer()
        server.start()
        client = ApiClient("http://127.0.0.1:${server.port}")
        tempDir = Files.createTempDirectory("lumina-android-test").toFile()
    }

    @After fun tearDown() { server.close(); tempDir.deleteRecursively() }

    @Test fun authenticatesReadsSessionAndLogsOutWithRealHttpRequests() {
        val seen = mutableListOf<String>()
        server.handler = { request ->
            seen += "${request.method} ${request.path} ${request.headers["authorization"] ?: ""}"
            when (request.path) {
                "/v1/auth/login" -> { assertTrue(request.body.contains("user@example.test")); Response(200, "{\"token\":\"test-token\",\"expires_at\":9999999999,\"user\":{\"id\":\"u1\",\"email\":\"user@example.test\"}}") }
                "/v1/auth/session" -> Response(200, "{\"authenticated\":true,\"user\":{\"id\":\"u1\",\"email\":\"user@example.test\"}}")
                "/v1/auth/logout" -> Response(200, "")
                else -> Response(404, "{\"error\":\"not found\"}")
            }
        }
        val session = client.authenticate(false, "user@example.test", "password")
        client.token = session.token
        assertEquals("u1", session.userId)
        assertEquals("u1", client.session().getJSONObject("user").getString("id"))
        client.logout()
        assertEquals(listOf("POST /v1/auth/login ", "GET /v1/auth/session Bearer test-token", "POST /v1/auth/logout Bearer test-token"), seen)
    }

    @Test fun devicesAndSyncUseRealBackendShapesAndMethods() {
        client.token = "sync-token"
        val requests = mutableListOf<String>()
        server.handler = { request ->
            requests += "${request.method} ${request.path}"
            when (request.path) {
                "/v1/devices" -> if (request.method == "POST") { assertTrue(request.body.contains("\"platform\":\"android\"")); Response(200, "{\"device_id\":\"d1\"}") } else Response(200, "{\"devices\":[{\"id\":\"d1\",\"name\":\"Android\",\"platform\":\"android\",\"last_sync_at\":null,\"revoked_at\":null}]}")
                "/v1/sync" -> { assertTrue(request.body.contains("\"device_id\":\"d1\"")); assertTrue(request.body.contains("\"changes\":[]")); Response(200, "{\"records\":[{\"record_id\":\"r1\",\"kind\":\"wallpaper\",\"payload\":{\"name\":\"Demo\"},\"revision\":3,\"deleted\":0}],\"files\":[],\"conflicts\":[]}") }
                else -> Response(404, "{\"error\":\"not found\"}")
            }
        }
        client.registerDevice("d1", "Android")
        assertEquals("Android", client.devices().single().name)
        val snapshot = client.sync("d1")
        assertEquals(3, snapshot.records.single().revision)
        assertEquals(listOf("POST /v1/devices", "GET /v1/devices", "POST /v1/sync"), requests)
    }

    @Test fun publishesWallpaperMetadataThenChunksAndConfirmsTheFile() {
        client.token = "publish-token"
        val bytes = "synthetic-image".toByteArray(StandardCharsets.UTF_8)
        val source = File(tempDir, "source.png").apply { writeBytes(bytes) }
        val hash = sha256(bytes)
        val requests = mutableListOf<String>()
        server.handler = { request ->
            requests += "${request.method} ${request.path}"
            when {
                request.path == "/v1/sync" && requests.count { it == "POST /v1/sync" } == 1 -> {
                    assertTrue(request.body.contains("\"kind\":\"wallpaper\""))
                    assertTrue(request.body.contains("\"base_revision\":0"))
                    Response(200, "{\"records\":[{\"record_id\":\"android-wallpaper\",\"kind\":\"wallpaper\",\"payload\":{\"name\":\"source.png\",\"hash\":\"$hash\",\"size_bytes\":${bytes.size}},\"revision\":1,\"deleted\":0}],\"files\":[],\"conflicts\":[]}")
                }
                request.path == "/v1/files/upload/init" -> {
                    assertTrue(request.body.contains("\"record_id\":\"android-wallpaper\""))
                    assertTrue(request.body.contains("\"sha256\":\"$hash\""))
                    Response(201, "{\"upload_id\":\"upload-1\",\"offset\":0,\"chunk_size\":1024}")
                }
                request.path == "/v1/files/upload/upload-1" && request.method == "PUT" -> {
                    assertEquals("0", request.headers["x-upload-offset"])
                    Response(200, "{\"upload_id\":\"upload-1\",\"offset\":${bytes.size},\"complete\":true}")
                }
                request.path == "/v1/files/upload/upload-1/complete" -> Response(201, "{\"file_id\":\"file-1\",\"sha256\":\"$hash\"}")
                request.path == "/v1/sync" -> Response(200, "{\"records\":[{\"record_id\":\"android-wallpaper\",\"kind\":\"wallpaper\",\"payload\":{\"name\":\"source.png\",\"hash\":\"$hash\",\"size_bytes\":${bytes.size}},\"revision\":1,\"deleted\":0}],\"files\":[{\"id\":\"file-1\",\"record_id\":\"android-wallpaper\",\"kind\":\"wallpaper\",\"original_name\":\"source.png\",\"mime\":\"image/png\",\"extension\":\"png\",\"size_bytes\":${bytes.size},\"sha256\":\"$hash\"}],\"conflicts\":[]}")
                else -> Response(404, "{\"error\":\"not found\"}")
            }
        }
        val result = client.publishWallpaper("android-device", "android-wallpaper", source, "source.png", "image/png", 0)
        assertEquals("android-wallpaper", result.recordId)
        assertEquals(1, result.revision)
        assertEquals("file-1", result.fileId)
        assertEquals(hash, result.sha256)
        assertEquals(listOf("POST /v1/sync", "POST /v1/files/upload/init", "PUT /v1/files/upload/upload-1", "POST /v1/files/upload/upload-1/complete", "POST /v1/sync"), requests)
    }

    @Test fun publicationConflictIsObservableBeforeAnyUpload() {
        client.token = "publish-token"
        val source = File(tempDir, "source.png").apply { writeText("local-image") }
        var uploadStarted = false
        server.handler = { request ->
            if (request.path == "/v1/sync") Response(200, "{\"records\":[],\"files\":[],\"conflicts\":[{\"id\":\"android-wallpaper\",\"kind\":\"wallpaper\",\"server_revision\":4,\"server_payload\":{\"hash\":\"remote\"}}]}")
            else { uploadStarted = true; Response(500, "{\"error\":\"upload must not start\"}") }
        }
        try { client.publishWallpaper("android-device", "android-wallpaper", source, "source.png", "image/png", 3); throw AssertionError("conflict was accepted") } catch (error: ApiException) { assertEquals(409, error.status) }
        assertFalse(uploadStarted)
        assertEquals("local-image", source.readText())
    }

    @Test fun retryWithTheSameRemoteHashDoesNotCreateAnotherUpload() {
        client.token = "publish-token"
        val bytes = "same-image".toByteArray(StandardCharsets.UTF_8)
        val source = File(tempDir, "same.png").apply { writeBytes(bytes) }
        val hash = sha256(bytes)
        val requests = mutableListOf<String>()
        server.handler = { request ->
            requests += "${request.method} ${request.path}"
            when {
                request.path == "/v1/sync" -> Response(200, "{\"records\":[{\"record_id\":\"same-wallpaper\",\"kind\":\"wallpaper\",\"payload\":{\"hash\":\"$hash\",\"size_bytes\":${bytes.size}},\"revision\":2,\"deleted\":0}],\"files\":[{\"id\":\"file-same\",\"record_id\":\"same-wallpaper\",\"kind\":\"wallpaper\",\"original_name\":\"same.png\",\"mime\":\"image/png\",\"extension\":\"png\",\"size_bytes\":${bytes.size},\"sha256\":\"$hash\"}],\"conflicts\":[]}")
                request.path == "/v1/files/upload/init" -> Response(200, "{\"file_id\":\"file-same\",\"already_exists\":true,\"size\":${bytes.size},\"sha256\":\"$hash\"}")
                else -> Response(500, "{\"error\":\"unexpected upload\"}")
            }
        }
        val result = client.publishWallpaper("android-device", "same-wallpaper", source, "same.png", "image/png", 2, metadataAlreadyCurrent = true)
        assertEquals("file-same", result.fileId)
        assertEquals(listOf("POST /v1/sync", "POST /v1/files/upload/init", "POST /v1/sync"), requests)
    }

    @Test fun reusableWallpaperIdentityRequiresTheTypedFileAssociation() {
        val hash = sha256("same-image".toByteArray(StandardCharsets.UTF_8))
        val snapshot = SyncSnapshot(
            records = listOf(
                RemoteRecord("without-file", "wallpaper", 1, org.json.JSONObject().put("hash", hash).put("size_bytes", 10), false),
                RemoteRecord("with-file", "wallpaper", 2, org.json.JSONObject().put("hash", hash).put("size_bytes", 10), false),
                RemoteRecord("same-id-different-kind", "cursor", 3, org.json.JSONObject().put("hash", hash).put("size_bytes", 10), false),
            ),
            files = listOf(RemoteFile("file-1", "with-file", "wallpaper", "same.png", "image/png", "png", 10, hash)),
            conflicts = emptyList(),
        )
        assertEquals("with-file", findReusableWallpaper(snapshot, hash, 10, "without-file")?.id)
    }

    @Test fun conditionalDownloadVerifiesHeadersHashAndPreservesExistingFileOnFailure() {
        client.token = "download-token"
        val bytes = "fixture-content".toByteArray(StandardCharsets.UTF_8)
        val hash = sha256(bytes)
        val metadata = RemoteFile("f1", "r1", "wallpaper", "demo.bin", "application/octet-stream", "bin", bytes.size.toLong(), hash)
        val record = RemoteRecord("r1", "wallpaper", 7, org.json.JSONObject(), false)
        server.handler = { request ->
            assertEquals("GET", request.method)
            assertEquals("Bearer download-token", request.headers["authorization"])
            assertEquals("android-test-device", request.headers["x-device-id"])
            assertEquals("wallpaper", request.headers["x-sync-kind"])
            assertEquals("r1", request.headers["x-sync-record-id"])
            assertEquals("7", request.headers["x-sync-revision"])
            Response(200, bytes)
        }
        val output = File(tempDir, "resource.bin")
        output.writeText("old-content")
        assertEquals(output, client.download(metadata, record.revision, output, "android-test-device"))
        assertEquals("fixture-content", output.readText())
        assertFalse(File(tempDir, "resource.bin.part").exists())

        server.handler = { Response(200, "wrong-content".toByteArray(StandardCharsets.UTF_8)) }
        val old = output.readBytes()
        try { client.download(metadata.copy(size = 13), record.revision, output, "android-test-device"); throw AssertionError("hash failure was accepted") } catch (_: ApiException) { }
        assertTrue(output.readBytes().contentEquals(old))
        assertFalse(File(tempDir, "resource.bin.part").exists())
    }

    @Test fun truncatedResponsePreservesPreviousFileAndAllowsRetry() {
        client.token = "download-token"
        val bytes = "complete-fixture".toByteArray(StandardCharsets.UTF_8)
        val metadata = RemoteFile("f2", "r2", "wallpaper", "demo.bin", "application/octet-stream", "bin", bytes.size.toLong(), sha256(bytes))
        val output = File(tempDir, "resource.bin")
        output.writeText("previous-valid-content")
        server.handler = { Response(200, bytes, closeAfterBytes = 4) }

        try {
            client.download(metadata, 1, output, "android-test-device")
            throw AssertionError("truncated response was accepted")
        } catch (_: Exception) { }

        assertEquals("previous-valid-content", output.readText())
        assertFalse(File(tempDir, "resource.bin.part").exists())

        server.handler = { Response(200, bytes) }
        assertEquals(output, client.download(metadata, 1, output, "android-test-device"))
        assertTrue(output.readBytes().contentEquals(bytes))
        assertFalse(File(tempDir, "resource.bin.part").exists())
    }

    @Test fun interruptedResponsePreservesPreviousFileAndCleansStaging() {
        client.token = "download-token"
        val bytes = "network-fixture".toByteArray(StandardCharsets.UTF_8)
        val metadata = RemoteFile("f3", "r3", "cursor", "demo.cur", "application/octet-stream", "cur", bytes.size.toLong(), sha256(bytes))
        val output = File(tempDir, "cursor.cur")
        output.writeText("previous-cursor")
        server.handler = { Response(200, bytes, closeAfterBytes = 1) }

        try {
            client.download(metadata, 2, output, "android-test-device")
            throw AssertionError("interrupted response was accepted")
        } catch (_: Exception) { }

        assertEquals("previous-cursor", output.readText())
        assertFalse(File(tempDir, "cursor.cur.part").exists())
    }

    @Test fun httpErrorsAndInvalidJsonRemainObservable() {
        client.token = "token"
        server.handler = { Response(409, "{\"error\":\"revision changed\"}") }
        try { client.session(); throw AssertionError("HTTP error was accepted") } catch (error: ApiException) { assertEquals(409, error.status); assertEquals("revision changed", error.message) }
        server.handler = { Response(401, "{\"error\":\"Sesi\u00f3n no v\u00e1lida\"}") }
        try { client.session(); throw AssertionError("401 was accepted") } catch (error: ApiException) { assertEquals(401, error.status) }
        server.handler = { Response(403, "{\"error\":\"Dispositivo revocado\"}") }
        try { client.session(); throw AssertionError("403 was accepted") } catch (error: ApiException) { assertEquals(403, error.status) }
        server.handler = { Response(200, "not-json") }
        try { client.session(); throw AssertionError("invalid JSON was accepted") } catch (_: Exception) { }
    }

    @Test fun resolvesTypedConflictWithExpectedRevisionAndIsolatesActions() {
        client.token = "resolve-token"
        val requests = mutableListOf<Request>()
        server.handler = { request ->
            requests += request
            if (request.path == "/v1/sync/resolve") {
                assertEquals("POST", request.method)
                assertEquals("Bearer resolve-token", request.headers["authorization"])
                Response(200, "{\"resolved\":true,\"idempotent\":false,\"kind\":\"wallpaper\",\"record_id\":\"shared-id\",\"revision\":8,\"deleted\":false,\"payload\":{}}")
            } else Response(404, "{\"error\":\"unexpected\"}")
        }
        val conflict = SyncConflict("shared-id", "wallpaper", 7, org.json.JSONObject())
        val response = client.resolveConflict("android-device", conflict, keepRemote = true, resolutionId = "resolution-1")
        assertTrue(response.getBoolean("resolved"))
        val body = requests.single().body
        assertTrue(body.contains("\"kind\":\"wallpaper\""))
        assertTrue(body.contains("\"record_id\":\"shared-id\""))
        assertTrue(body.contains("\"expected_revision\":7"))
        assertTrue(body.contains("\"action\":\"keep_remote\""))
        assertTrue(body.contains("\"resolution_id\":\"resolution-1\""))

        server.handler = { request ->
            assertTrue(request.body.contains("\"action\":\"keep_local\""))
            assertTrue(request.body.contains("\"kind\":\"cursor\""))
            Response(409, "{\"error\":\"revision changed\"}")
        }
        try {
            client.resolveConflict("android-device", SyncConflict("shared-id", "cursor", 7, org.json.JSONObject()), keepRemote = false,
                localPayload = org.json.JSONObject().put("name", "local"), resolutionId = "resolution-2")
            throw AssertionError("stale resolution was accepted")
        } catch (error: ApiException) {
            assertEquals(409, error.status)
        }
    }

    @Test fun unavailableServerRemainsAConnectionError() {
        val unusedPort = java.net.ServerSocket(0).use { it.localPort }
        try { ApiClient("http://127.0.0.1:$unusedPort").session(); throw AssertionError("offline server was accepted") } catch (_: java.io.IOException) { }
    }

    @Test fun pendingLocalChangeSeparatesLocalContentFromRemoteAndDetectsMutation() {
        val local = File(tempDir, "local.png").apply { writeBytes("local-bytes".toByteArray(StandardCharsets.UTF_8)) }
        val hash = sha256(local.readBytes())
        val pending = PendingLocalChange(
            kind = "wallpaper", recordId = "same-id", expectedRevision = 4,
            payload = org.json.JSONObject().put("name", "Local").put("hash", hash).put("size_bytes", local.length()),
            filePath = local.absolutePath, fileSize = local.length(), fileSha256 = hash,
            deleted = false, resolutionId = "stable-resolution",
        )
        val conflict = SyncConflict("same-id", "wallpaper", 4, org.json.JSONObject().put("name", "Remote"))
        assertTrue(pending.matches(conflict))
        assertTrue(pending.verifyContent())
        assertEquals("Remote", conflict.serverPayload.getString("name"))
        assertEquals("Local", pending.payload.getString("name"))
        assertEquals("stable-resolution", PendingLocalChange.fromJson(pending.toJson()).resolutionId)

        local.appendText("changed")
        assertFalse(pending.verifyContent())
        assertTrue(PendingLocalChange("playlist", "same-id", 4, org.json.JSONObject(), null, null, null, true, "tombstone").verifyContent())
    }

    private fun sha256(bytes: ByteArray): String = MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }
}

private data class Request(val method: String, val path: String, val headers: Map<String, String>, val body: String)
private data class Response(val status: Int, val body: ByteArray, val closeAfterBytes: Int? = null) {
    constructor(status: Int, body: String) : this(status, body.toByteArray(StandardCharsets.UTF_8))
}

private class LocalHttpServer {
    private val socket = ServerSocket(0, 10, java.net.InetAddress.getByName("127.0.0.1"))
    private val executor = Executors.newCachedThreadPool()
    @Volatile private var running = true
    @Volatile var handler: (Request) -> Response = { Response(500, "{}") }
    val port: Int get() = socket.localPort

    fun start() { executor.execute { while (running) runCatching { socket.accept() }.onSuccess { connection -> executor.execute { serve(connection) } } } }
    fun close() { running = false; socket.close(); executor.shutdownNow() }

    private fun serve(connection: Socket) {
        connection.use { socket ->
            val input = BufferedInputStream(socket.getInputStream())
            val requestLine = input.readLineAscii() ?: return
            val requestParts = requestLine.split(" ")
            val headers = linkedMapOf<String, String>()
            while (true) { val line = input.readLineAscii() ?: return; if (line.isEmpty()) break; val separator = line.indexOf(':'); if (separator > 0) headers[line.substring(0, separator).lowercase()] = line.substring(separator + 1).trim() }
            val length = headers["content-length"]?.toIntOrNull() ?: 0
            val body = ByteArray(length); input.readFully(body)
            val response = handler(Request(requestParts[0], requestParts[1].substringBefore('?'), headers, String(body, StandardCharsets.UTF_8)))
            val output = socket.getOutputStream(); output.write("HTTP/1.1 ${response.status} OK\r\nContent-Length: ${response.body.size}\r\nConnection: close\r\n\r\n".toByteArray(StandardCharsets.UTF_8)); val count = response.closeAfterBytes?.coerceIn(0, response.body.size) ?: response.body.size; output.write(response.body, 0, count); output.flush()
        }
    }
}

private fun BufferedInputStream.readLineAscii(): String? {
    val bytes = ArrayList<Byte>(); var previous = -1
    while (true) { val current = read(); if (current < 0) return if (bytes.isEmpty()) null else String(bytes.toByteArray(), StandardCharsets.ISO_8859_1); if (current == 10 && previous == 13) { bytes.removeAt(bytes.lastIndex); return String(bytes.toByteArray(), StandardCharsets.ISO_8859_1) }; bytes.add(current.toByte()); previous = current }
}

private fun BufferedInputStream.readFully(target: ByteArray) { var offset = 0; while (offset < target.size) { val count = read(target, offset, target.size - offset); if (count < 0) return; offset += count } }
