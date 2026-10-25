use crate::commands::monitor::current_monitor_ids;
use crate::dynamic::DynamicScheduler;
use crate::startup;
use crate::{db::get_connection, models::settings::Settings};
use rusqlite::params;
use serde::Serialize;
use tauri::{AppHandle, State};

pub const DYNAMIC_INTERVALS: [u32; 10] = [1, 5, 10, 15, 30, 60, 120, 360, 720, 1440];

#[derive(Debug, Clone, Serialize)]
pub struct DynamicConfigurationIssue {
    pub code: String,
    pub message: String,
    pub blocking: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DynamicConfigurationDiagnostic {
    pub consistent: bool,
    pub requires_decision: bool,
    pub global_enabled: bool,
    pub global_playlist_id: Option<String>,
    pub monitor_assignment_count: u32,
    pub issues: Vec<DynamicConfigurationIssue>,
}

#[tauri::command]
pub fn get_dynamic_configuration_diagnostic(
    app: AppHandle,
) -> Result<DynamicConfigurationDiagnostic, String> {
    let mut conn = get_connection().map_err(|e| e.to_string())?;
    let monitor_result = current_monitor_ids(&app);
    let mut diagnostic = diagnose_dynamic_configuration(&mut conn, monitor_result.as_deref().ok())?;
    if let Err(error) = monitor_result {
        diagnostic.issues.push(DynamicConfigurationIssue {
            code: "monitor_presence_unavailable".into(),
            message: format!("No se pudo comprobar la presencia actual de los monitores: {error}"),
            blocking: false,
        });
    }
    Ok(diagnostic)
}

fn diagnose_dynamic_configuration(
    conn: &mut rusqlite::Connection,
    current_monitor_ids: Option<&[String]>,
) -> Result<DynamicConfigurationDiagnostic, String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let (global_enabled, global_playlist_id, current_wallpaper_id, interval): (
        i32,
        Option<String>,
        Option<String>,
        u32,
    ) = tx
        .query_row(
            "SELECT dynamic_enabled, dynamic_playlist_id, dynamic_current_wallpaper_id, dynamic_interval_minutes FROM settings LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|e| e.to_string())?;
    let mut issues = Vec::new();
    let global_enabled = global_enabled == 1;

    if !DYNAMIC_INTERVALS.contains(&interval) {
        issues.push(DynamicConfigurationIssue {
            code: "invalid_interval".into(),
            message: "El intervalo global no está soportado".into(),
            blocking: true,
        });
    }

    match global_playlist_id.as_deref() {
        None if global_enabled => issues.push(DynamicConfigurationIssue {
            code: "global_playlist_missing".into(),
            message: "El modo global está activo pero no tiene playlist seleccionada".into(),
            blocking: true,
        }),
        Some(playlist_id) => {
            let exists: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM playlists WHERE id = ?1)",
                    [playlist_id],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;
            if !exists {
                issues.push(DynamicConfigurationIssue {
                    code: "global_playlist_missing".into(),
                    message: "La playlist global seleccionada ya no existe".into(),
                    blocking: global_enabled,
                });
            } else {
                let members: i64 = tx
                    .query_row(
                        "SELECT COUNT(*) FROM playlist_wallpapers WHERE playlist_id = ?1",
                        [playlist_id],
                        |row| row.get(0),
                    )
                    .map_err(|e| e.to_string())?;
                if members == 0 {
                    issues.push(DynamicConfigurationIssue {
                        code: "global_playlist_empty".into(),
                        message: "La playlist global está vacía".into(),
                        blocking: false,
                    });
                }
                if let Some(wallpaper_id) = current_wallpaper_id.as_deref() {
                    let current_is_member: bool = tx
                        .query_row(
                            "SELECT EXISTS(SELECT 1 FROM playlist_wallpapers pw JOIN wallpapers w ON w.id = pw.wallpaper_id WHERE pw.playlist_id = ?1 AND pw.wallpaper_id = ?2)",
                            (playlist_id, wallpaper_id),
                            |row| row.get(0),
                        )
                        .map_err(|e| e.to_string())?;
                    if !current_is_member {
                        issues.push(DynamicConfigurationIssue {
                            code: "global_current_invalid".into(),
                            message:
                                "El wallpaper actual global no pertenece a la playlist seleccionada"
                                    .into(),
                            blocking: false,
                        });
                    }
                }
            }
        }
        None => {}
    }

    let monitor_rows: Vec<(String, String, Option<String>)> = {
        let mut statement = tx
            .prepare("SELECT monitor_id, playlist_id, current_wallpaper_id FROM monitor_playlists")
            .map_err(|e| e.to_string())?;
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        rows
    };
    let monitor_assignment_count = monitor_rows.len() as u32;
    if global_enabled && monitor_assignment_count > 0 {
        issues.push(DynamicConfigurationIssue {
            code: "global_monitor_conflict".into(),
            message: "El modo global y las playlists por monitor están activos al mismo tiempo; se requiere una decisión explícita".into(),
            blocking: true,
        });
    }
    for (monitor_id, playlist_id, current_id) in monitor_rows {
        if current_monitor_ids.is_some_and(|ids| !ids.iter().any(|id| id == &monitor_id)) {
            issues.push(DynamicConfigurationIssue {
                code: "monitor_unavailable".into(),
                message: "La configuración del monitor se conserva, pero el monitor no está disponible ahora".into(),
                blocking: false,
            });
        }
        let exists: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM playlists WHERE id = ?1)",
                [&playlist_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if !exists {
            issues.push(DynamicConfigurationIssue {
                code: "monitor_playlist_missing".into(),
                message: "Una configuración por monitor apunta a una playlist inexistente".into(),
                blocking: true,
            });
            continue;
        }
        let members: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM playlist_wallpapers WHERE playlist_id = ?1",
                [&playlist_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if members == 0 {
            issues.push(DynamicConfigurationIssue {
                code: "monitor_playlist_empty".into(),
                message: "Una playlist por monitor está vacía".into(),
                blocking: false,
            });
        }
        if let Some(current_id) = current_id {
            let valid: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM playlist_wallpapers pw JOIN wallpapers w ON w.id = pw.wallpaper_id WHERE pw.playlist_id = ?1 AND pw.wallpaper_id = ?2)",
                    (&playlist_id, &current_id),
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;
            if !valid {
                issues.push(DynamicConfigurationIssue {
                    code: "monitor_current_invalid".into(),
                    message: "Un wallpaper actual por monitor no pertenece a su playlist".into(),
                    blocking: false,
                });
            }
        }
    }
    drop(tx);
    let requires_decision = issues
        .iter()
        .any(|issue| issue.code == "global_monitor_conflict");
    let consistent = issues.iter().all(|issue| !issue.blocking);
    Ok(DynamicConfigurationDiagnostic {
        consistent,
        requires_decision,
        global_enabled,
        global_playlist_id,
        monitor_assignment_count,
        issues,
    })
}

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
    restart_scheduler_if_needed(
        apply_launch_at_startup_and_persist(
            settings.launch_at_startup,
            startup::set_enabled,
            startup::is_enabled,
            || {
                let mut conn = get_connection().map_err(|e| e.to_string())?;
                persist_user_settings(&mut conn, &settings)
            },
        ),
        || scheduler.restart(),
    )?;
    Ok(())
}

fn apply_launch_at_startup_and_persist<SetStartup, ReadStartup, Persist, Persisted>(
    requested: bool,
    set_startup: SetStartup,
    read_startup: ReadStartup,
    persist: Persist,
) -> Result<Persisted, String>
where
    SetStartup: FnOnce(bool) -> Result<(), String>,
    ReadStartup: FnOnce() -> Result<bool, String>,
    Persist: FnOnce() -> Result<Persisted, String>,
{
    set_startup(requested).map_err(|error| {
        format!("No se pudo actualizar el inicio automático de Lumina: {error}")
    })?;

    let applied = read_startup().map_err(|error| {
        format!(
            "Windows pudo haber actualizado el inicio automático, pero no se pudo verificar su estado: {error}"
        )
    })?;
    if applied != requested {
        return Err(format!(
            "Windows no confirmó el estado solicitado del inicio automático (solicitado={requested}, observado={applied})"
        ));
    }

    persist().map_err(|error| {
        format!(
            "Windows confirmó el inicio automático, pero SQLite no pudo persistir la preferencia; el estado puede haber quedado divergente: {error}"
        )
    })
}

fn restart_scheduler_if_needed(
    dynamic_configuration_changed: Result<bool, String>,
    restart: impl FnOnce(),
) -> Result<(), String> {
    if dynamic_configuration_changed? {
        restart();
    }
    Ok(())
}

fn dynamic_configuration_changed(
    previous: &(i32, Option<String>, u32),
    settings: &Settings,
) -> bool {
    previous.0 != settings.dynamic_enabled as i32
        || previous.1.as_deref() != settings.dynamic_playlist_id.as_deref()
        || previous.2 != settings.dynamic_interval_minutes
}

fn persist_user_settings(
    conn: &mut rusqlite::Connection,
    settings: &Settings,
) -> Result<bool, String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    let previous_dynamic: (i32, Option<String>, u32) = tx
        .query_row(
            "SELECT dynamic_enabled, dynamic_playlist_id, dynamic_interval_minutes FROM settings LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|e| e.to_string())?;
    let dynamic_configuration_changed = dynamic_configuration_changed(&previous_dynamic, settings);

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
    Ok(dynamic_configuration_changed)
}

#[cfg(test)]
mod tests {
    use super::{
        apply_launch_at_startup_and_persist, diagnose_dynamic_configuration,
        dynamic_configuration_changed, persist_user_settings, restart_scheduler_if_needed,
    };
    use crate::models::settings::Settings;
    use rusqlite::Connection;
    use std::cell::Cell;

    #[test]
    fn startup_and_sqlite_success_is_reported_only_after_both_steps() {
        let external_state = Cell::new(false);
        let mut persisted = false;

        apply_launch_at_startup_and_persist(
            true,
            |requested| {
                external_state.set(requested);
                Ok(())
            },
            || Ok(external_state.get()),
            || {
                persisted = true;
                Ok(())
            },
        )
        .unwrap();

        assert!(external_state.get());
        assert!(persisted);
    }

    #[test]
    fn startup_failure_does_not_attempt_sqlite_persistence() {
        let mut persisted = false;
        let error = apply_launch_at_startup_and_persist(
            true,
            |_| Err("registro rechazado".into()),
            || Ok(true),
            || {
                persisted = true;
                Ok(())
            },
        )
        .unwrap_err();

        assert!(error.contains("registro rechazado"));
        assert!(!persisted);
    }

    #[test]
    fn sqlite_failure_after_external_success_reports_partial_state() {
        let external_state = Cell::new(false);
        let error = apply_launch_at_startup_and_persist(
            true,
            |requested| {
                external_state.set(requested);
                Ok(())
            },
            || Ok(external_state.get()),
            || {
                let conn = Connection::open_in_memory().unwrap();
                conn.execute("UPDATE missing_settings SET value = 1", [])
                    .map(|_| ())
                    .map_err(|db_error| db_error.to_string())
            },
        )
        .unwrap_err();

        assert!(external_state.get());
        assert!(error.contains("Windows confirmÃ³") || error.contains("Windows confirmó"));
        assert!(error.contains("SQLite"));
    }

    #[test]
    fn unverified_external_state_is_not_persisted_as_success() {
        let mut persisted = false;
        let error = apply_launch_at_startup_and_persist(
            true,
            |_| Ok(()),
            || Ok(false),
            || {
                persisted = true;
                Ok(())
            },
        )
        .unwrap_err();

        assert!(error.contains("solicitado"));
        assert!(!persisted);
    }

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
    fn scheduler_restart_tracks_only_persisted_dynamic_configuration() {
        let previous = (0, None, 10);
        let mut next = settings_snapshot();

        next.dynamic_enabled = true;
        next.dynamic_playlist_id = Some("playlist-a".into());
        assert!(dynamic_configuration_changed(&previous, &next));

        let previous_enabled = (1, Some("playlist-a".into()), 10);
        next = settings_snapshot();
        assert!(dynamic_configuration_changed(&previous_enabled, &next));

        next = settings_snapshot();
        next.dynamic_playlist_id = Some("playlist-a".into());
        assert!(dynamic_configuration_changed(&previous, &next));

        next.dynamic_playlist_id = Some("playlist-b".into());
        assert!(dynamic_configuration_changed(&previous_enabled, &next));

        next = settings_snapshot();
        next.dynamic_interval_minutes = 15;
        assert!(dynamic_configuration_changed(&previous, &next));

        next = settings_snapshot();
        next.accent_color = "cyan".into();
        next.ui_scale = 110;
        next.cursor_enabled = true;
        assert!(!dynamic_configuration_changed(&previous, &next));
        assert!(!dynamic_configuration_changed(
            &previous,
            &settings_snapshot()
        ));
    }

    #[test]
    fn scheduler_restart_is_not_called_when_persistence_fails() {
        let mut restarted = false;
        let result = restart_scheduler_if_needed(Err("sqlite unavailable".into()), || {
            restarted = true;
        });

        assert_eq!(result, Err("sqlite unavailable".into()));
        assert!(!restarted);
    }

    #[test]
    fn unchanged_dynamic_configuration_preserves_derived_state() {
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
                (1, 0, 1, 1, 0, 1, 'fill', 'violet', 100, 0, NULL, 10,
                 'wallpaper-current', 'old error', 0, NULL);",
        )
        .unwrap();

        let changed = persist_user_settings(&mut conn, &settings_snapshot()).unwrap();
        assert!(!changed);

        let derived: (Option<String>, Option<String>) = conn
            .query_row(
                "SELECT dynamic_current_wallpaper_id, dynamic_last_error FROM settings",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            derived,
            (Some("wallpaper-current".into()), Some("old error".into()))
        );
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

    fn diagnostic_schema(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE settings(dynamic_enabled INTEGER, dynamic_playlist_id TEXT, dynamic_interval_minutes INTEGER, dynamic_current_wallpaper_id TEXT);
             CREATE TABLE playlists(id TEXT PRIMARY KEY);
             CREATE TABLE wallpapers(id TEXT PRIMARY KEY);
             CREATE TABLE playlist_wallpapers(playlist_id TEXT, wallpaper_id TEXT);
             CREATE TABLE monitor_playlists(monitor_id TEXT PRIMARY KEY, playlist_id TEXT, current_wallpaper_id TEXT);",
        )
        .unwrap();
    }

    #[test]
    fn dynamic_diagnostic_is_read_only_for_consistent_global_state() {
        let mut conn = Connection::open_in_memory().unwrap();
        diagnostic_schema(&conn);
        conn.execute(
            "INSERT INTO settings VALUES (1, 'playlist-a', 5, 'wallpaper-a')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO playlists VALUES ('playlist-a')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO playlist_wallpapers VALUES ('playlist-a', 'wallpaper-a')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO wallpapers VALUES ('wallpaper-a')", [])
            .unwrap();

        let diagnostic =
            diagnose_dynamic_configuration(&mut conn, Some(&["monitor-a".into()])).unwrap();
        assert!(diagnostic.consistent);
        assert!(diagnostic.issues.is_empty());
        let settings: (i32, Option<String>) = conn
            .query_row(
                "SELECT dynamic_enabled, dynamic_playlist_id FROM settings",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(settings, (1, Some("playlist-a".into())));
    }

    #[test]
    fn dynamic_diagnostic_reports_global_monitor_conflict_without_mutation() {
        let mut conn = Connection::open_in_memory().unwrap();
        diagnostic_schema(&conn);
        conn.execute(
            "INSERT INTO settings VALUES (1, 'playlist-global', 5, NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO playlists VALUES ('playlist-global'), ('playlist-monitor')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO monitor_playlists VALUES ('monitor-a', 'playlist-monitor', NULL)",
            [],
        )
        .unwrap();

        let diagnostic =
            diagnose_dynamic_configuration(&mut conn, Some(&["monitor-a".into()])).unwrap();
        assert!(!diagnostic.consistent);
        assert!(diagnostic.requires_decision);
        assert_eq!(diagnostic.monitor_assignment_count, 1);
        assert!(diagnostic
            .issues
            .iter()
            .any(|issue| issue.code == "global_monitor_conflict"));
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM monitor_playlists", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn dynamic_diagnostic_distinguishes_missing_playlist_and_absent_monitor() {
        let mut conn = Connection::open_in_memory().unwrap();
        diagnostic_schema(&conn);
        conn.execute("INSERT INTO settings VALUES (0, NULL, 5, NULL)", [])
            .unwrap();
        conn.execute(
            "INSERT INTO monitor_playlists VALUES ('monitor-missing', 'playlist-gone', NULL)",
            [],
        )
        .unwrap();

        let diagnostic =
            diagnose_dynamic_configuration(&mut conn, Some(&["monitor-present".into()])).unwrap();
        assert!(diagnostic
            .issues
            .iter()
            .any(|issue| issue.code == "monitor_playlist_missing"));
        assert!(diagnostic
            .issues
            .iter()
            .any(|issue| issue.code == "monitor_unavailable"));
        assert_eq!(diagnostic.monitor_assignment_count, 1);
    }
}
