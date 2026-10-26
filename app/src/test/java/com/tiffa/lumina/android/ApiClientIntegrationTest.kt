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

    @Test fun unavailableServerRemainsAConnectionError() {
        val unusedPort = java.net.ServerSocket(0).use { it.localPort }
        try { ApiClient("http://127.0.0.1:$unusedPort").session(); throw AssertionError("offline server was accepted") } catch (_: java.io.IOException) { }
    }

    private fun sha256(bytes: ByteArray): String = MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }
}

private data class Request(val method: String, val path: String, val headers: Map<String, String>, val body: String)
private data class Response(val status: Int, val body: ByteArray) {
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
            val output = socket.getOutputStream(); output.write("HTTP/1.1 ${response.status} OK\r\nContent-Length: ${response.body.size}\r\nConnection: close\r\n\r\n".toByteArray(StandardCharsets.UTF_8)); output.write(response.body); output.flush()
        }
    }
}

private fun BufferedInputStream.readLineAscii(): String? {
    val bytes = ArrayList<Byte>(); var previous = -1
    while (true) { val current = read(); if (current < 0) return if (bytes.isEmpty()) null else String(bytes.toByteArray(), StandardCharsets.ISO_8859_1); if (current == 10 && previous == 13) { bytes.removeAt(bytes.lastIndex); return String(bytes.toByteArray(), StandardCharsets.ISO_8859_1) }; bytes.add(current.toByte()); previous = current }
}

private fun BufferedInputStream.readFully(target: ByteArray) { var offset = 0; while (offset < target.size) { val count = read(target, offset, target.size - offset); if (count < 0) return; offset += count } }
