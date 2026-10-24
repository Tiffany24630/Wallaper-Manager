use crate::dynamic::DynamicScheduler;
use crate::startup;
use crate::{db::get_connection, models::settings::Settings};
use rusqlite::params;
use tauri::State;

pub const DYNAMIC_INTERVALS: [u32; 10] = [1, 5, 10, 15, 30, 60, 120, 360, 720, 1440];

#[tauri::command]
pub fn get_settings() -> Result<Settings, String> {
    let conn = get_connection().map_err(|e| e.to_string())?;

    let result = conn.query_row(
        "
        SELECT
            launch_at_startup,
            minimize_to_tray,
            hardware_acceleration,
            pause_on_battery,
            pause_when_maximized,
            scaling_mode,
            accent_color,
            ui_scale,
            dynamic_enabled,
            dynamic_playlist_id,
            dynamic_interval_minutes,
            dynamic_current_wallpaper_id,
            dynamic_last_error,
            cursor_enabled,
            active_cursor_id
        FROM settings
        LIMIT 1
        ",
        [],
        |row| {
            Ok(Settings {
                launch_at_startup: row.get::<_, i32>(0)? == 1,

                minimize_to_tray: row.get::<_, i32>(1)? == 1,

                hardware_acceleration: row.get::<_, i32>(2)? == 1,

                pause_on_battery: row.get::<_, i32>(3)? == 1,

                pause_when_maximized: row.get::<_, i32>(4)? == 1,

                scaling_mode: row.get(5)?,
                accent_color: row.get(6)?,
                ui_scale: row.get(7)?,
                dynamic_enabled: row.get::<_, i32>(8)? == 1,
                dynamic_playlist_id: row.get(9)?,
                dynamic_interval_minutes: row.get::<_, u32>(10)?,
                dynamic_current_wallpaper_id: row.get(11)?,
                dynamic_last_error: row.get(12)?,
                cursor_enabled: row.get::<_, i32>(13)? == 1,
                active_cursor_id: row.get(14)?,
            })
        },
    );

    let mut settings = result.map_err(|error| error.to_string())?;
    settings.launch_at_startup = startup::is_enabled()?;
    Ok(settings)
}

#[tauri::command]
pub fn save_settings(
    settings: Settings,
    scheduler: State<'_, DynamicScheduler>,
) -> Result<(), String> {
    if !["fill", "fit", "stretch", "center"].contains(&settings.scaling_mode.as_str()) {
        return Err("Modo de ajuste no válido".into());
    }
    if !["violet", "cyan", "rose", "emerald"].contains(&settings.accent_color.as_str()) {
        return Err("Color de acento no válido".into());
    }
    if !(85..=115).contains(&settings.ui_scale) {
        return Err("La escala de interfaz debe estar entre 85% y 115%".into());
    }
    if !DYNAMIC_INTERVALS.contains(&settings.dynamic_interval_minutes) {
        return Err("El intervalo del fondo dinámico no es válido".into());
    }
    if settings.dynamic_enabled && settings.dynamic_playlist_id.is_none() {
        return Err("Selecciona una lista antes de activar el fondo dinámico".into());
    }
    if settings.dynamic_enabled {
        let conn = get_connection().map_err(|e| e.to_string())?;
        let playlist_id = settings.dynamic_playlist_id.as_deref().unwrap_or_default();
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM playlists WHERE id = ?1)",
                [playlist_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if !exists {
            return Err("La playlist seleccionada ya no existe".into());
        }
        let monitor_playlists: i64 = conn
            .query_row("SELECT COUNT(*) FROM monitor_playlists", [], |row| {
                row.get(0)
            })
            .map_err(|e| e.to_string())?;
        if monitor_playlists > 0 {
            return Err("Desactiva primero las playlists asignadas por monitor para usar el fondo dinámico global".into());
        }
    }
    startup::set_enabled(settings.launch_at_startup)?;
    if startup::is_enabled()? != settings.launch_at_startup {
        return Err("Windows no pudo confirmar el estado del inicio automático de Lumina".into());
    }
    let mut conn = get_connection().map_err(|e| e.to_string())?;
    persist_user_settings(&mut conn, &settings)?;
    scheduler.restart();
    Ok(())
}

fn persist_user_settings(
    conn: &mut rusqlite::Connection,
    settings: &Settings,
) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // Re-check dynamic configuration inside the write transaction. The
    // validation performed by `save_settings` happens before opening this
    // transaction, so a playlist deletion or monitor assignment could race
    // with a stale settings snapshot and otherwise resurrect invalid global
    // configuration.
    if settings.dynamic_enabled {
        let playlist_id = settings
            .dynamic_playlist_id
            .as_deref()
            .ok_or_else(|| "Selecciona una lista antes de activar el fondo dinámico".to_string())?;
        let playlist_exists: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM playlists WHERE id = ?1)",
                [playlist_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if !playlist_exists {
            return Err("La playlist seleccionada ya no existe".into());
        }
        let monitor_playlists: i64 = tx
            .query_row("SELECT COUNT(*) FROM monitor_playlists", [], |row| {
                row.get(0)
            })
            .map_err(|e| e.to_string())?;
        if monitor_playlists > 0 {
            return Err(
                "Desactiva primero las playlists asignadas por monitor para usar el fondo dinámico global"
                    .into(),
            );
        }
    }

    tx.execute(
        "
        UPDATE settings SET
            launch_at_startup = ?1,
            minimize_to_tray = ?2,
            hardware_acceleration = ?3,
            pause_on_battery = ?4,
            pause_when_maximized = ?5,
            scaling_mode = ?6,
            accent_color = ?7,
            ui_scale = ?8,
            dynamic_enabled = ?9,
            dynamic_playlist_id = ?10,
            dynamic_interval_minutes = ?11,
            dynamic_current_wallpaper_id = CASE
                WHEN dynamic_enabled != ?9
                  OR dynamic_playlist_id IS NOT ?10
                  OR dynamic_interval_minutes != ?11
                THEN NULL ELSE dynamic_current_wallpaper_id END,
            dynamic_last_error = CASE
                WHEN dynamic_enabled != ?9
                  OR dynamic_playlist_id IS NOT ?10
                  OR dynamic_interval_minutes != ?11
                THEN NULL ELSE dynamic_last_error END
        WHERE id = (SELECT id FROM settings LIMIT 1)
        ",
        params![
            settings.launch_at_startup as i32,
            settings.minimize_to_tray as i32,
            settings.hardware_acceleration as i32,
            settings.pause_on_battery as i32,
            settings.pause_when_maximized as i32,
            settings.scaling_mode,
            settings.accent_color,
            settings.ui_scale,
            settings.dynamic_enabled as i32,
            settings.dynamic_playlist_id,
            settings.dynamic_interval_minutes,
        ],
    )
    .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::persist_user_settings;
    use crate::models::settings::Settings;
    use rusqlite::Connection;

    fn settings_snapshot() -> Settings {
        Settings {
            launch_at_startup: false,
            minimize_to_tray: true,
            hardware_acceleration: true,
            pause_on_battery: false,
            pause_when_maximized: true,
            scaling_mode: "fill".into(),
            accent_color: "violet".into(),
            ui_scale: 100,
            dynamic_enabled: false,
            dynamic_playlist_id: None,
            dynamic_interval_minutes: 10,
            dynamic_current_wallpaper_id: None,
            dynamic_last_error: None,
            cursor_enabled: false,
            active_cursor_id: Some("stale-cursor".into()),
        }
    }

    #[test]
    fn stale_settings_snapshot_does_not_overwrite_cursor_writer() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE settings(
                id INTEGER PRIMARY KEY,
                launch_at_startup INTEGER,
                minimize_to_tray INTEGER,
                hardware_acceleration INTEGER,
                pause_on_battery INTEGER,
                pause_when_maximized INTEGER,
                scaling_mode TEXT,
                accent_color TEXT,
                ui_scale INTEGER,
                dynamic_enabled INTEGER,
                dynamic_playlist_id TEXT,
                dynamic_interval_minutes INTEGER,
                dynamic_current_wallpaper_id TEXT,
                dynamic_last_error TEXT,
                cursor_enabled INTEGER,
                active_cursor_id TEXT
            );
            INSERT INTO settings VALUES
                (1, 0, 1, 1, 0, 1, 'fill', 'violet', 100, 0, NULL, 5,
                 'wallpaper-current', 'old error', 0, NULL);",
        )
        .unwrap();

        // Another backend writer commits after the UI read its snapshot.
        conn.execute(
            "UPDATE settings SET cursor_enabled = 1, active_cursor_id = 'cursor-new' WHERE id = 1",
            [],
        )
        .unwrap();

        persist_user_settings(&mut conn, &settings_snapshot()).unwrap();

        let persisted: (i32, Option<String>, i32) = conn
            .query_row(
                "SELECT cursor_enabled, active_cursor_id, dynamic_interval_minutes FROM settings WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(persisted, (1, Some("cursor-new".into()), 10));
    }

    #[test]
    fn stale_settings_snapshot_cannot_resurrect_deleted_dynamic_playlist() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE settings(
                id INTEGER PRIMARY KEY,
                launch_at_startup INTEGER,
                minimize_to_tray INTEGER,
                hardware_acceleration INTEGER,
                pause_on_battery INTEGER,
                pause_when_maximized INTEGER,
                scaling_mode TEXT,
                accent_color TEXT,
                ui_scale INTEGER,
                dynamic_enabled INTEGER,
                dynamic_playlist_id TEXT,
                dynamic_interval_minutes INTEGER,
                dynamic_current_wallpaper_id TEXT,
                dynamic_last_error TEXT,
                cursor_enabled INTEGER,
                active_cursor_id TEXT
            );
            CREATE TABLE playlists(id TEXT PRIMARY KEY);
            CREATE TABLE monitor_playlists(monitor_id TEXT PRIMARY KEY);
            INSERT INTO settings VALUES
                (1, 0, 1, 1, 0, 1, 'fill', 'violet', 100, 1, 'playlist-old', 5,
                 'wallpaper-current', NULL, 0, NULL);
            INSERT INTO playlists VALUES ('playlist-old');",
        )
        .unwrap();

        // The UI read this snapshot before another backend operation removed
        // the playlist and cleared the global dynamic configuration.
        let mut stale = settings_snapshot();
        stale.dynamic_enabled = true;
        stale.dynamic_playlist_id = Some("playlist-old".into());

        conn.execute("DELETE FROM playlists WHERE id = 'playlist-old'", [])
            .unwrap();
        conn.execute(
            "UPDATE settings SET dynamic_enabled = 0, dynamic_playlist_id = NULL, dynamic_current_wallpaper_id = NULL, dynamic_last_error = 'playlist deleted' WHERE id = 1",
            [],
        )
        .unwrap();

        let error = persist_user_settings(&mut conn, &stale).unwrap_err();
        assert_eq!(error, "La playlist seleccionada ya no existe");

        let persisted: (i32, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT dynamic_enabled, dynamic_playlist_id, dynamic_last_error FROM settings WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(persisted, (0, None, Some("playlist deleted".into())));
    }
}
