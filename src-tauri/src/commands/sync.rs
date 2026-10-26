use crate::{
    commands::{
        cursor::replace_cursor_resource_in_connection, download::with_downloaded_synced_resource,
        wallpaper::replace_wallpaper_resource_in_connection,
    },
    db::{app_data_dir, get_connection},
    models::sync::{SyncConflict, SyncPlaylistConflict, SyncPlaylistState, SyncQueueItem},
};
use rusqlite::{params, Connection};
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

fn validate_conflict_kind(kind: &str) -> Result<(), String> {
    if ["wallpaper", "cursor", "playlist"].contains(&kind) {
        Ok(())
    } else {
        Err("Tipo de conflicto de sincronización no válido".into())
    }
}

fn record_sync_conflict_in_connection(
    conn: &Connection,
    kind: &str,
    record_id: &str,
    local_payload: &Value,
    remote_payload: &Value,
    base_revision: Option<i64>,
    remote_revision: i64,
) -> Result<String, String> {
    validate_conflict_kind(kind)?;
    if record_id.trim().is_empty() {
        return Err("Identidad de conflicto no válida".into());
    }
    let local_payload = serde_json::to_string(local_payload).map_err(|e| e.to_string())?;
    let remote_payload = serde_json::to_string(remote_payload).map_err(|e| e.to_string())?;
    if let Ok(existing) = conn.query_row(
        "SELECT id FROM sync_conflicts WHERE kind=?1 AND record_id=?2 AND local_payload=?3 AND remote_payload=?4 AND resolved=0 LIMIT 1",
        params![kind, record_id, local_payload, remote_payload],
        |row| row.get::<_, String>(0),
    ) {
        return Ok(existing);
    }
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO sync_conflicts(id,kind,record_id,local_payload,remote_payload,base_revision,remote_revision,created_at,resolved) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,0)",
        params![id, kind, record_id, local_payload, remote_payload, base_revision, remote_revision, now()],
    )
    .map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub fn record_sync_conflict(
    kind: String,
    record_id: String,
    local_payload: Value,
    remote_payload: Value,
    base_revision: Option<i64>,
    remote_revision: i64,
) -> Result<String, String> {
    let conn = get_connection().map_err(|e| e.to_string())?;
    record_sync_conflict_in_connection(
        &conn,
        &kind,
        &record_id,
        &local_payload,
        &remote_payload,
        base_revision,
        remote_revision,
    )
}

#[tauri::command]
pub fn list_sync_conflicts() -> Result<Vec<SyncConflict>, String> {
    let conn = get_connection().map_err(|e| e.to_string())?;
    let mut statement = conn
        .prepare("SELECT id,kind,record_id,local_payload,remote_payload,base_revision,remote_revision,last_error FROM sync_conflicts WHERE resolved=0 ORDER BY created_at")
        .map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(SyncConflict {
                id: row.get(0)?,
                kind: row.get(1)?,
                record_id: row.get(2)?,
                local_payload: row.get(3)?,
                remote_payload: row.get(4)?,
                base_revision: row.get(5)?,
                remote_revision: row.get(6)?,
                last_error: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn resolve_sync_conflict(id: String, kind: String) -> Result<(), String> {
    validate_conflict_kind(&kind)?;
    let conn = get_connection().map_err(|e| e.to_string())?;
    resolve_sync_conflict_in_connection(&conn, &id, &kind)
}

#[tauri::command]
pub fn resolve_sync_conflict_keep_local(id: String, kind: String) -> Result<(), String> {
    validate_conflict_kind(&kind)?;
    let conn = get_connection().map_err(|e| e.to_string())?;
    resolve_sync_conflict_in_connection(&conn, &id, &kind)
}

#[tauri::command]
pub fn record_sync_conflict_error(id: String, kind: String, error: String) -> Result<(), String> {
    validate_conflict_kind(&kind)?;
    let conn = get_connection().map_err(|e| e.to_string())?;
    let error = error.chars().take(500).collect::<String>();
    let changed = conn
        .execute(
            "UPDATE sync_conflicts SET last_error=?3 WHERE id=?1 AND kind=?2 AND resolved=0",
            params![id, kind, error],
        )
        .map_err(|e| e.to_string())?;
    if changed == 1 {
        Ok(())
    } else {
        Err("El conflicto no existe, ya fue resuelto o su tipo no coincide".into())
    }
}

#[tauri::command]
pub async fn accept_remote_sync_conflict(
    conflict_id: String,
    kind: String,
    file_id: String,
    api_url: String,
    device_id: String,
    expected_hash: String,
    extension: String,
    remote_revision: i64,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        accept_remote_sync_conflict_sync(
            conflict_id,
            kind,
            file_id,
            api_url,
            device_id,
            expected_hash,
            extension,
            remote_revision,
        )
    })
    .await
    .map_err(|error| format!("La resolución remota terminó inesperadamente: {error}"))?
}

fn accept_remote_sync_conflict_sync(
    conflict_id: String,
    kind: String,
    file_id: String,
    api_url: String,
    device_id: String,
    expected_hash: String,
    extension: String,
    remote_revision: i64,
) -> Result<(), String> {
    validate_conflict_kind(&kind)?;
    if kind == "playlist" {
        return Err("La aceptación de playlists usa su flujo existente".into());
    }
    let (record_id, stored_revision, remote_payload): (String, i64, String) = {
        let conn = get_connection().map_err(|error| error.to_string())?;
        conn.query_row(
            "SELECT record_id,remote_revision,remote_payload FROM sync_conflicts WHERE id=?1 AND kind=?2 AND resolved=0",
            params![conflict_id, kind],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|_| "El conflicto no existe, ya fue resuelto o su tipo no coincide".to_string())?
    };
    if stored_revision != remote_revision {
        return Err(
            "La revisión remota del conflicto cambió; vuelve a sincronizar antes de aceptar".into(),
        );
    }
    let payload: Value = serde_json::from_str(&remote_payload)
        .map_err(|error| format!("Los metadatos remotos del conflicto no son válidos: {error}"))?;
    validate_remote_conflict_payload(
        &kind,
        &payload,
        &expected_hash,
        stored_revision,
        remote_revision,
    )?;
    let payload_hash = payload
        .get("hash")
        .and_then(Value::as_str)
        .ok_or("El conflicto no contiene el hash remoto necesario para reemplazar el recurso")?;
    if payload_hash.to_ascii_lowercase() != expected_hash.to_ascii_lowercase() {
        return Err("El hash solicitado no coincide con el hash del conflicto".into());
    }
    if kind == "wallpaper"
        && payload
            .get("resource_type")
            .and_then(Value::as_str)
            .is_some_and(|value| value != "static")
    {
        return Err("La aceptación remota de wallpapers Animated no está soportada".into());
    }
    let extension = extension.trim_start_matches('.').to_ascii_lowercase();
    let root = app_data_dir().map_err(|error| error.to_string())?;
    with_downloaded_synced_resource(
        api_url,
        device_id,
        file_id,
        kind.clone(),
        extension,
        expected_hash,
        Some((&record_id, remote_revision)),
        |temporary| {
            let mut conn = get_connection().map_err(|error| error.to_string())?;
            if kind == "wallpaper" {
                replace_wallpaper_resource_in_connection(&mut conn, &root, &record_id, temporary)
            } else {
                replace_cursor_resource_in_connection(&mut conn, &root, &record_id, temporary)
            }
        },
    )?;
    let conn = get_connection().map_err(|error| error.to_string())?;
    resolve_sync_conflict_in_connection(&conn, &conflict_id, &kind)
}

fn validate_remote_conflict_payload(
    kind: &str,
    payload: &Value,
    expected_hash: &str,
    stored_revision: i64,
    requested_revision: i64,
) -> Result<(), String> {
    if stored_revision != requested_revision {
        return Err(
            "La revisión remota del conflicto cambió; vuelve a sincronizar antes de aceptar".into(),
        );
    }
    let payload_hash = payload
        .get("hash")
        .and_then(Value::as_str)
        .ok_or("El conflicto no contiene el hash remoto necesario para reemplazar el recurso")?;
    if payload_hash.to_ascii_lowercase() != expected_hash.to_ascii_lowercase() {
        return Err("El hash solicitado no coincide con el hash del conflicto".into());
    }
    if kind == "wallpaper"
        && payload
            .get("resource_type")
            .and_then(Value::as_str)
            .is_some_and(|value| value != "static")
    {
        return Err("La aceptación remota de wallpapers Animated no está soportada".into());
    }
    Ok(())
}

fn resolve_sync_conflict_in_connection(
    conn: &Connection,
    id: &str,
    kind: &str,
) -> Result<(), String> {
    let changed = conn
        .execute(
            "UPDATE sync_conflicts SET resolved=1,last_error=NULL WHERE id=?1 AND kind=?2 AND resolved=0",
            params![id, kind],
        )
        .map_err(|e| e.to_string())?;
    if changed == 1 {
        Ok(())
    } else {
        Err("El conflicto no existe, ya fue resuelto o su tipo no coincide".into())
    }
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
    let generic_id = record_sync_conflict_in_connection(
        &conn,
        "playlist",
        &playlist_id,
        &local_payload,
        &remote_payload,
        base_revision,
        remote_revision,
    )?;
    let local_payload = serde_json::to_string(&local_payload).map_err(|e| e.to_string())?;
    let remote_payload = serde_json::to_string(&remote_payload).map_err(|e| e.to_string())?;
    if let Ok(existing) = conn.query_row("SELECT id FROM sync_playlist_conflicts WHERE playlist_id=?1 AND local_payload=?2 AND remote_payload=?3 AND resolved=0 LIMIT 1", params![playlist_id, local_payload, remote_payload], |row| row.get::<_, String>(0)) { return Ok(existing); }
    let id = Uuid::new_v4().to_string();
    conn.execute("INSERT INTO sync_playlist_conflicts(id,playlist_id,local_payload,remote_payload,base_revision,remote_revision,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)", params![id, playlist_id, local_payload, remote_payload, base_revision, remote_revision, now()]).map_err(|e| e.to_string())?;
    Ok(generic_id)
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
    conn.execute("INSERT INTO sync_tombstones(entity_id, kind, created_at) VALUES(?1, ?2, ?3) ON CONFLICT(entity_id, kind) DO UPDATE SET created_at=excluded.created_at", params![entity_id, kind, now()]).map_err(|e| e.to_string())?;
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

#[cfg(test)]
mod tests {
    use super::{
        record_sync_conflict_in_connection, resolve_sync_conflict_in_connection,
        validate_remote_conflict_payload,
    };
    use rusqlite::Connection;
    use serde_json::json;

    fn connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE sync_conflicts(
                id TEXT PRIMARY KEY, kind TEXT NOT NULL, record_id TEXT NOT NULL,
                local_payload TEXT NOT NULL, remote_payload TEXT NOT NULL,
                base_revision INTEGER, remote_revision INTEGER NOT NULL,
                created_at INTEGER NOT NULL, last_error TEXT,
                resolved INTEGER NOT NULL DEFAULT 0
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn conflicts_are_persisted_by_kind_for_the_same_record_id() {
        let conn = connection();
        for kind in ["wallpaper", "cursor", "playlist"] {
            record_sync_conflict_in_connection(
                &conn,
                kind,
                "same-id",
                &json!({"local": kind}),
                &json!({"remote": kind}),
                Some(1),
                2,
            )
            .unwrap();
        }
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sync_conflicts", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn conflict_resolution_is_typed_and_only_marks_confirmed_rows() {
        let conn = connection();
        let id = record_sync_conflict_in_connection(
            &conn,
            "playlist",
            "same-id",
            &json!({"local": 1}),
            &json!({"remote": 2}),
            Some(1),
            2,
        )
        .unwrap();
        assert!(resolve_sync_conflict_in_connection(&conn, &id, "wallpaper").is_err());
        let unresolved: i64 = conn
            .query_row(
                "SELECT resolved FROM sync_conflicts WHERE id=?1",
                [&id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(unresolved, 0);
        conn.execute(
            "UPDATE sync_conflicts SET last_error='No se pudo aplicar' WHERE id=?1",
            [&id],
        )
        .unwrap();
        let error: String = conn
            .query_row(
                "SELECT last_error FROM sync_conflicts WHERE id=?1",
                [&id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(error, "No se pudo aplicar");
        resolve_sync_conflict_in_connection(&conn, &id, "playlist").unwrap();
        assert!(resolve_sync_conflict_in_connection(&conn, &id, "playlist").is_err());
    }

    #[test]
    fn keeping_local_only_changes_the_conflict_row() {
        let conn = connection();
        let id = record_sync_conflict_in_connection(
            &conn,
            "wallpaper",
            "same-id",
            &json!({"name": "local", "hash": "local-hash"}),
            &json!({"name": "remote", "hash": "remote-hash"}),
            Some(1),
            2,
        )
        .unwrap();
        resolve_sync_conflict_in_connection(&conn, &id, "wallpaper").unwrap();
        let (kind, record_id, local_payload, remote_payload, resolved): (
            String,
            String,
            String,
            String,
            i64,
        ) = conn
            .query_row(
                "SELECT kind,record_id,local_payload,remote_payload,resolved FROM sync_conflicts WHERE id=?1",
                [&id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .unwrap();
        assert_eq!(kind, "wallpaper");
        assert_eq!(record_id, "same-id");
        assert!(local_payload.contains("local-hash"));
        assert!(remote_payload.contains("remote-hash"));
        assert_eq!(resolved, 1);
    }

    #[test]
    fn remote_conflict_validation_rejects_hash_or_revision_mismatch() {
        let payload = json!({"hash": "a".repeat(64), "resource_type": "static"});
        assert!(
            validate_remote_conflict_payload("wallpaper", &payload, &"b".repeat(64), 4, 4).is_err()
        );
        assert!(
            validate_remote_conflict_payload("wallpaper", &payload, &"a".repeat(64), 4, 5).is_err()
        );
    }

    #[test]
    fn remote_conflict_validation_requires_hash_and_rejects_animated_wallpapers() {
        let without_hash = json!({"resource_type": "static"});
        assert!(
            validate_remote_conflict_payload("cursor", &without_hash, &"a".repeat(64), 1, 1)
                .is_err()
        );
        let animated = json!({"hash": "a".repeat(64), "resource_type": "animated"});
        assert!(
            validate_remote_conflict_payload("wallpaper", &animated, &"a".repeat(64), 1, 1)
                .is_err()
        );
        assert!(
            validate_remote_conflict_payload("cursor", &animated, &"a".repeat(64), 1, 1).is_ok()
        );
    }
}
