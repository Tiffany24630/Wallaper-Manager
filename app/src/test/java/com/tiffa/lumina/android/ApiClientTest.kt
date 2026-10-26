package com.tiffa.lumina.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

class ApiClientTest {
    @Test fun normalizesLocalDevelopmentUrl() {
        assertEquals("http://10.0.2.2:8765", ApiClient.normalizeBaseUrl("http://10.0.2.2:8765/"))
        assertEquals("https://sync.example.test", ApiClient.normalizeBaseUrl("https://sync.example.test/"))
    }

    @Test fun rejectsCleartextRemoteUrl() {
        assertThrows(IllegalStateException::class.java) { ApiClient.normalizeBaseUrl("http://sync.example.test") }
    }

    @Test fun rejectsEmbeddedCredentialsAndQueryParameters() {
        assertThrows(IllegalArgumentException::class.java) { ApiClient.normalizeBaseUrl("https://user:secret@sync.example.test") }
        assertThrows(IllegalArgumentException::class.java) { ApiClient.normalizeBaseUrl("https://sync.example.test/?token=secret") }
    }
}

