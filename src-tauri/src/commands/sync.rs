use crate::{
    db::get_connection,
    models::sync::{SyncPlaylistConflict, SyncPlaylistState, SyncQueueItem},
};
use rusqlite::params;
use serde_json::Value;
use uuid::Uuid;

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

#[tauri::command]
pub fn mark_playlist_local_change(playlist_id: String, deleted: bool) -> Result<(), String> {
    if playlist_id.trim().is_empty() {
        return Err("Lista no válida".into());
    }
    let conn = get_connection().map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO sync_playlist_state(playlist_id,last_remote_revision,base_payload,local_dirty,deleted_pending,updated_at) VALUES(?1,NULL,NULL,1,?2,?3) ON CONFLICT(playlist_id) DO UPDATE SET local_dirty=1,deleted_pending=?2,updated_at=?3", params![playlist_id, i32::from(deleted), now()]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn list_sync_playlist_states() -> Result<Vec<SyncPlaylistState>, String> {
    let conn = get_connection().map_err(|e| e.to_string())?;
    let mut statement = conn.prepare("SELECT playlist_id,last_remote_revision,base_payload,local_dirty,deleted_pending FROM sync_playlist_state").map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(SyncPlaylistState {
                playlist_id: row.get(0)?,
                last_remote_revision: row.get(1)?,
                base_payload: row.get(2)?,
                local_dirty: row.get::<_, i32>(3)? != 0,
                deleted_pending: row.get::<_, i32>(4)? != 0,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn mark_playlist_synced(
    playlist_id: String,
    revision: i64,
    payload: Value,
) -> Result<(), String> {
    let conn = get_connection().map_err(|e| e.to_string())?;
    let payload = serde_json::to_string(&payload).map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO sync_playlist_state(playlist_id,last_remote_revision,base_payload,local_dirty,deleted_pending,updated_at) VALUES(?1,?2,?3,0,0,?4) ON CONFLICT(playlist_id) DO UPDATE SET last_remote_revision=?2,base_payload=?3,local_dirty=0,deleted_pending=0,updated_at=?4", params![playlist_id, revision, payload, now()]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn record_playlist_conflict(
    playlist_id: String,
    local_payload: Value,
    remote_payload: Value,
    base_revision: Option<i64>,
    remote_revision: i64,
) -> Result<String, String> {
    let conn = get_connection().map_err(|e| e.to_string())?;
    let local_payload = serde_json::to_string(&local_payload).map_err(|e| e.to_string())?;
    let remote_payload = serde_json::to_string(&remote_payload).map_err(|e| e.to_string())?;
    if let Ok(existing) = conn.query_row("SELECT id FROM sync_playlist_conflicts WHERE playlist_id=?1 AND local_payload=?2 AND remote_payload=?3 AND resolved=0 LIMIT 1", params![playlist_id, local_payload, remote_payload], |row| row.get::<_, String>(0)) { return Ok(existing); }
    let id = Uuid::new_v4().to_string();
    conn.execute("INSERT INTO sync_playlist_conflicts(id,playlist_id,local_payload,remote_payload,base_revision,remote_revision,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)", params![id, playlist_id, local_payload, remote_payload, base_revision, remote_revision, now()]).map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub fn list_playlist_conflicts() -> Result<Vec<SyncPlaylistConflict>, String> {
    let conn = get_connection().map_err(|e| e.to_string())?;
    let mut statement = conn.prepare("SELECT id,playlist_id,local_payload,remote_payload,base_revision,remote_revision FROM sync_playlist_conflicts WHERE resolved=0 ORDER BY created_at").map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(SyncPlaylistConflict {
                id: row.get(0)?,
                playlist_id: row.get(1)?,
                local_payload: row.get(2)?,
                remote_payload: row.get(3)?,
                base_revision: row.get(4)?,
                remote_revision: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn record_sync_tombstone(entity_id: String, kind: String) -> Result<(), String> {
    if entity_id.trim().is_empty() || !["wallpaper", "cursor", "playlist"].contains(&kind.as_str())
    {
        return Err("Tombstone de sincronización no válido".into());
    }
    let conn = get_connection().map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO sync_tombstones(entity_id, kind, created_at) VALUES(?1, ?2, ?3) ON CONFLICT(entity_id) DO UPDATE SET kind=excluded.kind", params![entity_id, kind, now()]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn list_sync_tombstones() -> Result<Vec<(String, String)>, String> {
    let conn = get_connection().map_err(|e| e.to_string())?;
    let mut statement = conn
        .prepare("SELECT entity_id, kind FROM sync_tombstones ORDER BY created_at")
        .map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn enqueue_sync_operation(
    operation_type: String,
    entity_id: Option<String>,
    revision: Option<i64>,
) -> Result<String, String> {
    if operation_type.trim().is_empty() || operation_type.len() > 64 {
        return Err("Tipo de operación de sincronización no válido".into());
    }
    let timestamp = now();
    let conn = get_connection().map_err(|e| e.to_string())?;
    if let Some(existing) = conn
        .query_row(
            "SELECT id FROM sync_queue WHERE operation_type=?1 AND entity_id IS ?2 AND state IN ('pending','retry','processing') ORDER BY created_at LIMIT 1",
            params![operation_type.trim(), entity_id],
            |row| row.get::<_, String>(0),
        )
        .ok()
    {
        conn.execute("UPDATE sync_queue SET revision=?2, updated_at=?3 WHERE id=?1", params![existing, revision, timestamp]).map_err(|e| e.to_string())?;
        return Ok(existing);
    }
    let id = Uuid::new_v4().to_string();
    conn.execute("INSERT INTO sync_queue(id, operation_type, entity_id, revision, state, attempts, next_retry_at, created_at, updated_at, last_error) VALUES(?1,?2,?3,?4,'pending',0,?5,?6,?6,NULL)", params![id, operation_type.trim(), entity_id, revision, timestamp, timestamp]).map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub fn list_sync_queue() -> Result<Vec<SyncQueueItem>, String> {
    let conn = get_connection().map_err(|e| e.to_string())?;
    conn.execute("UPDATE sync_queue SET state='retry', next_retry_at=NULL, updated_at=?1, last_error='Operación recuperada después de un cierre inesperado' WHERE state='processing' AND updated_at < ?2", params![now(), now() - 900]).map_err(|e| e.to_string())?;
    let mut statement = conn.prepare("SELECT id, operation_type, entity_id, revision, state, attempts, next_retry_at, created_at, updated_at, last_error FROM sync_queue ORDER BY created_at DESC").map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(SyncQueueItem {
                id: row.get(0)?,
                operation_type: row.get(1)?,
                entity_id: row.get(2)?,
                revision: row.get(3)?,
                state: row.get(4)?,
                attempts: row.get(5)?,
                next_retry_at: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
                last_error: row.get(9)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn claim_sync_operation(id: String) -> Result<bool, String> {
    let conn = get_connection().map_err(|e| e.to_string())?;
    let timestamp = now();
    let changed = conn.execute("UPDATE sync_queue SET state='processing', attempts=attempts+1, updated_at=?2 WHERE id=?1 AND state IN ('pending','retry') AND (next_retry_at IS NULL OR next_retry_at<=?2)", params![id, timestamp]).map_err(|e| e.to_string())?;
    Ok(changed == 1)
}

#[tauri::command]
pub fn finish_sync_operation(
    id: String,
    success: bool,
    error: Option<String>,
) -> Result<(), String> {
    let conn = get_connection().map_err(|e| e.to_string())?;
    let timestamp = now();
    if success {
        conn.execute(
            "UPDATE sync_queue SET state='completed', updated_at=?2, last_error=NULL WHERE id=?1",
            params![id, timestamp],
        )
        .map_err(|e| e.to_string())?;
    } else {
        let safe_error = error
            .unwrap_or_else(|| "Error de sincronización".into())
            .chars()
            .take(500)
            .collect::<String>();
        conn.execute("UPDATE sync_queue SET state=CASE WHEN attempts>=5 THEN 'failed' ELSE 'retry' END, next_retry_at=CASE WHEN attempts>=5 THEN NULL ELSE ?2 + MIN(3600, 30 * (1 << MIN(attempts, 6))) END, updated_at=?2, last_error=?3 WHERE id=?1", params![id, timestamp, safe_error]).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn retry_sync_operation(id: String) -> Result<(), String> {
    let conn = get_connection().map_err(|e| e.to_string())?;
    conn.execute("UPDATE sync_queue SET state='retry', next_retry_at=NULL, updated_at=?, last_error=NULL WHERE id=? AND state IN ('failed','retry')", params![now(), id]).map_err(|e| e.to_string())?;
    Ok(())
}
