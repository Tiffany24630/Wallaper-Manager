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
    let tx = conn.transaction().map_err(|e| e.to_string())?;

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
                THEN NULL ELSE dynamic_last_error END,
            cursor_enabled = ?12,
            active_cursor_id = ?13
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
            settings.cursor_enabled as i32,
            settings.active_cursor_id,
        ],
    )
    .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    scheduler.restart();
    Ok(())
}
