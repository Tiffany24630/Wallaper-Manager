package com.tiffa.lumina.android

import org.json.JSONObject

data class Session(val token: String, val expiresAt: Long, val userId: String, val email: String)
data class Device(val id: String, val name: String, val platform: String, val lastSyncAt: Long?, val revokedAt: Long?)
data class RemoteRecord(val id: String, val kind: String, val revision: Long, val payload: JSONObject, val deleted: Boolean)
data class RemoteFile(val id: String, val recordId: String, val kind: String, val name: String, val mime: String, val extension: String, val size: Long, val sha256: String)
data class SyncConflict(val id: String, val kind: String, val serverRevision: Long, val serverPayload: JSONObject)
data class SyncSnapshot(val records: List<RemoteRecord>, val files: List<RemoteFile>, val conflicts: List<SyncConflict>)
data class SyncChange(val id: String, val kind: String, val baseRevision: Long, val payload: JSONObject, val deleted: Boolean = false)
data class PublishResult(val recordId: String, val revision: Long, val fileId: String, val sha256: String)

class ApiException(val status: Int, override val message: String) : Exception(message)

