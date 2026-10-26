package com.tiffa.lumina.android

import android.content.Context
import org.json.JSONArray

class PendingLocalChangeStore(context: Context, ownerId: String) {
    private val prefs = context.applicationContext.getSharedPreferences("lumina_pending_changes", Context.MODE_PRIVATE)
    private val key = "changes_$ownerId"

    @Synchronized fun save(change: PendingLocalChange) {
        val values = all().filterNot { it.kind == change.kind && it.recordId == change.recordId }.toMutableList()
        values += change
        write(values)
    }

    @Synchronized fun find(conflict: SyncConflict): PendingLocalChange? =
        all().firstOrNull { it.matches(conflict) }

    @Synchronized fun find(kind: String, recordId: String, expectedRevision: Long): PendingLocalChange? =
        all().firstOrNull { it.kind == kind && it.recordId == recordId && it.expectedRevision == expectedRevision }

    @Synchronized fun remove(change: PendingLocalChange) {
        write(all().filterNot { it.resolutionId == change.resolutionId })
    }

    private fun all(): List<PendingLocalChange> {
        val raw = prefs.getString(key, null) ?: return emptyList()
        val array = runCatching { JSONArray(raw) }.getOrNull() ?: return emptyList()
        return buildList {
            for (index in 0 until array.length()) {
                runCatching { PendingLocalChange.fromJson(array.getJSONObject(index)) }.onSuccess { add(it) }
            }
        }
    }

    private fun write(values: List<PendingLocalChange>) {
        val array = JSONArray()
        values.forEach { array.put(it.toJson()) }
        prefs.edit().putString(key, array.toString()).apply()
    }
}
