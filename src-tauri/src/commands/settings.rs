use crate::{db::get_connection, models::settings::Settings};
use rusqlite::params;

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
            ui_scale
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
            })
        },
    );

    match result {
        Ok(settings) => Ok(settings),

        Err(_) => Ok(Settings {
            launch_at_startup: false,
            minimize_to_tray: true,
            hardware_acceleration: true,
            pause_on_battery: false,
            pause_when_maximized: true,
            scaling_mode: "fill".into(),
            accent_color: "violet".into(),
            ui_scale: 100,
        }),
    }
}

#[tauri::command]
pub fn save_settings(settings: Settings) -> Result<(), String> {
    if !["fill", "fit", "stretch", "center"].contains(&settings.scaling_mode.as_str()) {
        return Err("Modo de ajuste no válido".into());
    }
    if !["violet", "cyan", "rose", "emerald"].contains(&settings.accent_color.as_str()) {
        return Err("Color de acento no válido".into());
    }
    if !(85..=115).contains(&settings.ui_scale) {
        return Err("La escala de interfaz debe estar entre 85% y 115%".into());
    }
    let mut conn = get_connection().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    tx.execute(
        "
        DELETE FROM settings
        ",
        [],
    )
    .map_err(|e| e.to_string())?;

    tx.execute(
        "
        INSERT INTO settings(
            launch_at_startup,
            minimize_to_tray,
            hardware_acceleration,
            pause_on_battery,
            pause_when_maximized,
            scaling_mode,
            accent_color,
            ui_scale
        )
        VALUES(
            ?1,
            ?2,
            ?3,
            ?4,
            ?5,
            ?6,
            ?7,
            ?8
        )
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
        ],
    )
    .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())
}
