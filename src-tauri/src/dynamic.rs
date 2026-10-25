use crate::{
    animated_wallpaper::{self, AnimatedActivationOutcome},
    commands::monitor::current_monitor_ids,
    commands::settings::DYNAMIC_INTERVALS,
    commands::wallpaper::set_active_wallpaper_internal,
    db::get_connection,
    models::monitor_assignment::{MonitorIdentity, WallpaperBackend},
    models::wallpaper_coordinator,
    utils::windows_wallpaper::set_monitor_wallpaper,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Condvar, Mutex,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tauri::AppHandle;

#[derive(Clone)]
pub struct DynamicScheduler {
    worker: Arc<Mutex<Option<Worker>>>,
    apply_lock: Arc<Mutex<()>>,
    app_handle: Arc<Mutex<Option<AppHandle>>>,
}

impl Default for DynamicScheduler {
    fn default() -> Self {
        Self {
            worker: Arc::new(Mutex::new(None)),
            apply_lock: Arc::new(Mutex::new(())),
            app_handle: Arc::new(Mutex::new(None)),
        }
    }
}

pub struct ManualOperation<'a> {
    scheduler: &'a DynamicScheduler,
    guard: Option<std::sync::MutexGuard<'a, ()>>,
    restart_on_drop: bool,
    completed: bool,
}

impl ManualOperation<'_> {
    pub fn complete(&mut self) {
        self.completed = true;
    }

    pub fn complete_and_resume(&mut self) {
        self.completed = true;
        if self.restart_on_drop {
            self.restart_on_drop = false;
            self.scheduler.restart();
        }
    }
}

impl Drop for ManualOperation<'_> {
    fn drop(&mut self) {
        drop(self.guard.take());
        if self.restart_on_drop && !self.completed {
            self.scheduler.restart();
        }
    }
}

struct Worker {
    stop: Arc<AtomicBool>,
    wake: Arc<(Mutex<bool>, Condvar)>,
    handle: JoinHandle<()>,
}

impl DynamicScheduler {
    pub fn set_app_handle(&self, app: AppHandle) {
        *self.app_handle.lock().expect("dynamic scheduler app lock") = Some(app);
    }

    pub fn begin_manual_operation(&self) -> ManualOperation<'_> {
        let restart_on_drop = self
            .worker
            .lock()
            .expect("dynamic scheduler lock")
            .is_some();
        self.stop();
        ManualOperation {
            scheduler: self,
            guard: Some(
                self.apply_lock
                    .lock()
                    .expect("dynamic scheduler apply lock"),
            ),
            restart_on_drop,
            completed: false,
        }
    }

    pub fn restart(&self) {
        let mut slot = self.worker.lock().expect("dynamic scheduler lock");
        if let Some(worker) = slot.take() {
            stop_worker(worker);
        }
        if !scheduler_can_start_worker(crate::db::smoke_mode_requested().unwrap_or(false)) {
            return;
        }
        let stop = Arc::new(AtomicBool::new(false));
        let wake = Arc::new((Mutex::new(false), Condvar::new()));
        let thread_stop = Arc::clone(&stop);
        let thread_wake = Arc::clone(&wake);
        let apply_lock = Arc::clone(&self.apply_lock);
        let app_handle = self
            .app_handle
            .lock()
            .expect("dynamic scheduler app lock")
            .clone();
        let handle =
            thread::spawn(move || run_worker(thread_stop, thread_wake, apply_lock, app_handle));
        *slot = Some(Worker { stop, wake, handle });
    }

    pub fn stop(&self) {
        let mut slot = self.worker.lock().expect("dynamic scheduler lock");
        if let Some(worker) = slot.take() {
            stop_worker(worker);
        }
    }
}

fn scheduler_can_start_worker(smoke_mode: bool) -> bool {
    !smoke_mode
}

fn stop_worker(worker: Worker) {
    worker.stop.store(true, Ordering::Release);
    let (wake_lock, wake_condition) = &*worker.wake;
    if let Ok(mut requested) = wake_lock.lock() {
        *requested = true;
        wake_condition.notify_one();
    }
    if worker.handle.join().is_err() {
        eprintln!("[DynamicScheduler] SCHEDULER_WORKER_JOIN_FAILED");
    }
}

fn run_worker(
    stop: Arc<AtomicBool>,
    wake: Arc<(Mutex<bool>, Condvar)>,
    apply_lock: Arc<Mutex<()>>,
    app_handle: Option<AppHandle>,
) {
    loop {
        if stop.load(Ordering::Acquire) {
            return;
        }
        let interval = match apply_next(&apply_lock, app_handle.as_ref()) {
            Ok(Some(minutes)) => minutes,
            Ok(None) => return,
            Err(error) => {
                set_dynamic_error(&error);
                return;
            }
        };
        let duration = Duration::from_secs(interval.saturating_mul(60) as u64);
        if !wait_for_next_cycle(&stop, &wake, duration) {
            return;
        }
    }
}

fn apply_next(
    apply_lock: &Mutex<()>,
    app_handle: Option<&AppHandle>,
) -> Result<Option<u32>, String> {
    let _guard = apply_lock
        .lock()
        .map_err(|_| "No se pudo bloquear el cambio dinámico".to_string())?;
    let mut conn = get_connection().map_err(|e| e.to_string())?;
    let (enabled, playlist_id, current_id, interval): (i32, Option<String>, Option<String>, u32) = conn
        .query_row(
            "SELECT dynamic_enabled, dynamic_playlist_id, dynamic_current_wallpaper_id, dynamic_interval_minutes FROM settings LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|_| "No hay configuración de fondo dinámico".to_string())?;
    let monitor_assignments: Vec<(String, String, Option<String>)> = {
        let mut statement = conn
            .prepare(
                "SELECT monitor_id, playlist_id, current_wallpaper_id FROM monitor_playlists ORDER BY monitor_id",
            )
            .map_err(|e| e.to_string())?;
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .map_err(|e| e.to_string())?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        rows
    };
    if !monitor_assignments.is_empty() {
        let current_monitor_ids = app_handle.map(current_monitor_ids);
        wallpaper::set_mode(scaling_mode(&conn))
            .map_err(|e| format!("No se pudo cambiar el modo de ajuste: {e}"))?;
        for (monitor_id, playlist_id, current_id) in monitor_assignments {
            match &current_monitor_ids {
                Some(Ok(ids)) if !ids.iter().any(|id| id == &monitor_id) => {
                    set_monitor_playlist_error(
                        &conn,
                        &monitor_id,
                        "El monitor no estÃ¡ disponible; se conserva la configuraciÃ³n",
                    );
                    continue;
                }
                Some(Err(error)) => {
                    set_monitor_playlist_error(
                        &conn,
                        &monitor_id,
                        &format!("No se pudo resolver la presencia del monitor: {error}"),
                    );
                    continue;
                }
                _ => {}
            }
            let valid = playlist_wallpapers(&conn, &playlist_id)?;
            if valid.is_empty() {
                set_monitor_playlist_error(
                    &conn,
                    &monitor_id,
                    "La playlist no tiene fondos accesibles",
                );
                continue;
            }
            let Some(next_id) = select_next_id(&valid, current_id.as_deref()) else {
                continue;
            };
            let Some(path) = valid
                .iter()
                .find(|(id, _)| id == &next_id)
                .map(|(_, path)| path)
            else {
                set_monitor_playlist_error(
                    &conn,
                    &monitor_id,
                    "El wallpaper seleccionado ya no existe",
                );
                continue;
            };
            if wallpaper_is_animated(&conn, &next_id)? {
                let Some(app) = app_handle else {
                    set_monitor_playlist_error(
                        &conn,
                        &monitor_id,
                        "No hay contexto de aplicaciÃ³n para activar Animated",
                    );
                    continue;
                };
                let handle =
                    match tauri::async_runtime::block_on(animated_wallpaper::open_library_animated(
                        app.clone(),
                        monitor_id.clone(),
                        next_id.clone(),
                        path.to_string(),
                    )) {
                        Ok(handle) => handle,
                        Err(error) => {
                            set_monitor_playlist_error(&conn, &monitor_id, &error);
                            continue;
                        }
                    };
                match animated_wallpaper::wait_for_activation(&handle, Duration::from_secs(30)) {
                    AnimatedActivationOutcome::Ready => {
                        if let Err(error) =
                            persist_monitor_rotation(&mut conn, &monitor_id, &next_id)
                        {
                            let cleanup_error =
                                animated_wallpaper::close_animated_activation(app, &handle)
                                    .err()
                                    .map(|error| format!("; cleanup: {error}"))
                                    .unwrap_or_default();
                            set_monitor_playlist_error(
                                &conn,
                                &monitor_id,
                                &format!(
                                    "Animated listo, pero no se pudo persistir la asignaciÃ³n: {error}{cleanup_error}"
                                ),
                            );
                        }
                    }
                    AnimatedActivationOutcome::Pending => {
                        let cleanup_error =
                            animated_wallpaper::close_animated_activation(app, &handle)
                                .err()
                                .map(|error| format!("; cleanup: {error}"))
                                .unwrap_or_default();
                        set_monitor_playlist_error(
                            &conn,
                            &monitor_id,
                            &format!("Animated sigue pendiente{cleanup_error}"),
                        );
                    }
                    AnimatedActivationOutcome::Failed => {
                        let cleanup_error =
                            animated_wallpaper::close_animated_activation(app, &handle)
                                .err()
                                .map(|error| format!("; cleanup: {error}"))
                                .unwrap_or_default();
                        set_monitor_playlist_error(
                            &conn,
                            &monitor_id,
                            &format!("No se pudo activar Animated{cleanup_error}"),
                        );
                    }
                    AnimatedActivationOutcome::Cancelled => {
                        let cleanup_error =
                            animated_wallpaper::close_animated_activation(app, &handle)
                                .err()
                                .map(|error| format!("; cleanup: {error}"))
                                .unwrap_or_default();
                        set_monitor_playlist_error(
                            &conn,
                            &monitor_id,
                            &format!("La activaciÃ³n Animated fue cancelada{cleanup_error}"),
                        );
                    }
                }
                continue;
            }
            let identity = match MonitorIdentity::new(monitor_id.clone()) {
                Ok(identity) => identity,
                Err(error) => {
                    set_monitor_playlist_error(&conn, &monitor_id, error);
                    continue;
                }
            };
            let operation = match wallpaper_coordinator::global().lock() {
                Ok(mut coordinator) => coordinator.reserve(identity, WallpaperBackend::Static),
                Err(_) => Err(crate::models::wallpaper_coordinator::CoordinatorError::Obsolete),
            };
            let operation = match operation {
                Ok(operation) => operation,
                Err(error) => {
                    if matches!(
                        error,
                        crate::models::wallpaper_coordinator::CoordinatorError::ShuttingDown
                            | crate::models::wallpaper_coordinator::CoordinatorError::Shutdown
                    ) {
                        return Ok(None);
                    }
                    set_monitor_playlist_error(&conn, &monitor_id, &error.to_string());
                    continue;
                }
            };
            if let Err(error) = wallpaper_coordinator::global()
                .lock()
                .map_err(|_| "No se pudo bloquear el coordinador de wallpapers".to_string())
                .and_then(|coordinator| {
                    coordinator
                        .validate_previous_deactivated(&operation)
                        .map_err(|error| error.to_string())
                })
            {
                if let Ok(mut coordinator) = wallpaper_coordinator::global().lock() {
                    coordinator.abort(&operation);
                }
                set_monitor_playlist_error(&conn, &monitor_id, &error);
                continue;
            }
            match set_monitor_wallpaper(&monitor_id, path) {
                Ok(()) => {
                    let committed = wallpaper_coordinator::global()
                        .lock()
                        .map_err(|_| "No se pudo bloquear el coordinador de wallpapers".to_string())
                        .and_then(|mut coordinator| {
                            coordinator
                                .confirm_previous_deactivated(&operation)
                                .map_err(|error| error.to_string())?;
                            coordinator
                                .confirm_activated(&operation)
                                .map_err(|error| error.to_string())?;
                            coordinator
                                .commit(&operation)
                                .map_err(|error| error.to_string())
                        });
                    match committed {
                        Ok(_) => {
                            if let Err(error) =
                                persist_monitor_rotation(&mut conn, &monitor_id, &next_id)
                            {
                                set_monitor_playlist_error(&conn, &monitor_id, &error);
                            }
                        }
                        Err(error) => {
                            if let Ok(mut coordinator) = wallpaper_coordinator::global().lock() {
                                coordinator.abort(&operation);
                            }
                            set_monitor_playlist_error(&conn, &monitor_id, &error.to_string())
                        }
                    }
                }
                Err(error) => {
                    if let Ok(mut coordinator) = wallpaper_coordinator::global().lock() {
                        coordinator.abort(&operation);
                    }
                    set_monitor_playlist_error(&conn, &monitor_id, &error.to_string());
                }
            }
        }
        return Ok(Some(interval));
    }
    if enabled != 1 {
        return Ok(None);
    }
    if !DYNAMIC_INTERVALS.contains(&interval) {
        return Err("El intervalo del fondo dinámico no es válido".into());
    }
    let playlist_id =
        playlist_id.ok_or("El fondo dinámico no tiene una lista seleccionada".to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT w.id, w.path FROM playlist_wallpapers pw JOIN wallpapers w ON w.id = pw.wallpaper_id WHERE pw.playlist_id = ?1 ORDER BY pw.position ASC, pw.wallpaper_id ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows: Vec<(String, String)> = stmt
        .query_map([&playlist_id], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let valid: Vec<(String, String)> = rows
        .into_iter()
        .filter(|(_, path)| std::path::Path::new(path).is_file())
        .collect();
    if valid.is_empty() {
        conn.execute(
            "UPDATE settings SET dynamic_enabled = 0, dynamic_last_error = ?1 WHERE id = (SELECT id FROM settings LIMIT 1)",
            ["La lista dinámica está vacía o no tiene fondos accesibles"],
        )
        .map_err(|e| e.to_string())?;
        return Ok(None);
    }
    let next_id = match select_next_id(&valid, current_id.as_deref()) {
        Some(id) => id,
        None => {
            conn.execute(
                "UPDATE settings SET dynamic_last_error = NULL WHERE id = (SELECT id FROM settings LIMIT 1)",
                [],
            )
            .map_err(|e| e.to_string())?;
            return Ok(Some(interval));
        }
    };
    if wallpaper_is_animated(&conn, &next_id)? {
        conn.execute(
            "UPDATE settings SET dynamic_last_error = ?1 WHERE id = (SELECT id FROM settings LIMIT 1)",
            ["El scheduler seleccionÃ³ un Animated, pero su activaciÃ³n de superficie requiere un monitor explÃ­cito"],
        )
        .map_err(|e| e.to_string())?;
        return Ok(Some(interval));
    }
    set_active_wallpaper_internal(next_id.clone())?;
    conn.execute(
        "UPDATE settings SET dynamic_current_wallpaper_id = ?1, dynamic_last_error = NULL WHERE id = (SELECT id FROM settings LIMIT 1)",
        [&next_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(Some(interval))
}

fn wallpaper_is_animated(conn: &rusqlite::Connection, wallpaper_id: &str) -> Result<bool, String> {
    let resource_type: String = conn
        .query_row(
            "SELECT COALESCE(resource_type, 'static') FROM wallpapers WHERE id = ?1",
            [wallpaper_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(resource_type == "animated")
}

fn scaling_mode(conn: &rusqlite::Connection) -> wallpaper::Mode {
    match conn
        .query_row("SELECT scaling_mode FROM settings LIMIT 1", [], |row| {
            row.get::<_, String>(0)
        })
        .unwrap_or_else(|_| "fill".into())
        .as_str()
    {
        "fit" => wallpaper::Mode::Fit,
        "stretch" => wallpaper::Mode::Stretch,
        "center" => wallpaper::Mode::Center,
        _ => wallpaper::Mode::Crop,
    }
}

fn wait_for_next_cycle(
    stop: &AtomicBool,
    wake: &Arc<(Mutex<bool>, Condvar)>,
    duration: Duration,
) -> bool {
    if stop.load(Ordering::Acquire) {
        return false;
    }
    let (wake_lock, wake_condition) = &**wake;
    let Ok(guard) = wake_lock.lock() else {
        return false;
    };
    let wait = wake_condition.wait_timeout_while(guard, duration, |requested| {
        !*requested && !stop.load(Ordering::Acquire)
    });
    let Ok((mut guard, _)) = wait else {
        return false;
    };
    *guard = false;
    !stop.load(Ordering::Acquire)
}

fn playlist_wallpapers(
    conn: &rusqlite::Connection,
    playlist_id: &str,
) -> Result<Vec<(String, String)>, String> {
    let mut statement = conn
        .prepare(
            "SELECT w.id, w.path FROM playlist_wallpapers pw JOIN wallpapers w ON w.id = pw.wallpaper_id WHERE pw.playlist_id = ?1 ORDER BY pw.position ASC, pw.wallpaper_id ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([playlist_id], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| e.to_string())?;
    let rows = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .filter(|(_, path)| std::path::Path::new(path).is_file())
        .collect())
}

fn set_monitor_playlist_error(conn: &rusqlite::Connection, monitor_id: &str, error: &str) {
    let _ = conn.execute(
        "UPDATE monitor_playlists SET last_error = ?1 WHERE monitor_id = ?2",
        (error, monitor_id),
    );
}

fn persist_monitor_rotation(
    conn: &mut rusqlite::Connection,
    monitor_id: &str,
    wallpaper_id: &str,
) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE monitor_playlists SET current_wallpaper_id = ?1, last_error = NULL WHERE monitor_id = ?2",
        (wallpaper_id, monitor_id),
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT OR REPLACE INTO monitor_wallpapers(monitor_id, wallpaper_id) VALUES(?1, ?2)",
        (monitor_id, wallpaper_id),
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

fn select_next_id(valid: &[(String, String)], current: Option<&str>) -> Option<String> {
    if valid.is_empty() {
        return None;
    }
    match current.and_then(|id| valid.iter().position(|(candidate, _)| candidate == id)) {
        Some(index) if valid.len() > 1 => Some(valid[(index + 1) % valid.len()].0.clone()),
        Some(_) => None,
        None => Some(valid[0].0.clone()),
    }
}

fn set_dynamic_error(error: &str) {
    if let Ok(conn) = get_connection() {
        let _ = conn.execute(
            "UPDATE settings SET dynamic_last_error = ?1 WHERE id = (SELECT id FROM settings LIMIT 1)",
            [error],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        persist_monitor_rotation, scheduler_can_start_worker, select_next_id, wait_for_next_cycle,
        DynamicScheduler, DYNAMIC_INTERVALS,
    };
    use rusqlite::Connection;
    use std::sync::{atomic::AtomicBool, Arc, Condvar, Mutex};
    use std::time::Duration;

    #[test]
    fn accepts_only_supported_intervals() {
        assert!(DYNAMIC_INTERVALS.contains(&1));
        assert!(DYNAMIC_INTERVALS.contains(&1440));
        assert!(!DYNAMIC_INTERVALS.contains(&2));
    }

    #[test]
    fn selects_next_wallpaper_without_repeating_single_item() {
        let items = vec![("one".into(), "one.png".into())];
        assert_eq!(select_next_id(&items, None), Some("one".into()));
        assert_eq!(select_next_id(&items, Some("one")), None);
    }

    #[test]
    fn selects_sequentially_and_recovers_when_current_was_removed() {
        let items = vec![
            ("one".into(), "one.png".into()),
            ("two".into(), "two.png".into()),
            ("three".into(), "three.png".into()),
        ];
        assert_eq!(select_next_id(&items, Some("one")), Some("two".into()));
        assert_eq!(select_next_id(&items, Some("three")), Some("one".into()));
        assert_eq!(select_next_id(&items, Some("deleted")), Some("one".into()));
        assert_eq!(select_next_id(&[], Some("deleted")), None);
    }

    #[test]
    fn monitor_rotation_persists_playlist_current_and_monitor_assignment_atomically() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE monitor_playlists(monitor_id TEXT PRIMARY KEY, current_wallpaper_id TEXT, last_error TEXT);
             CREATE TABLE monitor_wallpapers(monitor_id TEXT PRIMARY KEY, wallpaper_id TEXT NOT NULL);
             INSERT INTO monitor_playlists VALUES ('monitor-a', NULL, 'old error');",
        )
        .unwrap();

        persist_monitor_rotation(&mut conn, "monitor-a", "wallpaper-z").unwrap();

        let current: String = conn
            .query_row(
                "SELECT current_wallpaper_id FROM monitor_playlists WHERE monitor_id = 'monitor-a'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let assignment: String = conn
            .query_row(
                "SELECT wallpaper_id FROM monitor_wallpapers WHERE monitor_id = 'monitor-a'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(current, "wallpaper-z");
        assert_eq!(assignment, "wallpaper-z");
    }

    #[test]
    fn monitor_rotation_does_not_leave_playlist_current_on_assignment_failure() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE monitor_playlists(monitor_id TEXT PRIMARY KEY, current_wallpaper_id TEXT, last_error TEXT)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO monitor_playlists VALUES ('monitor-a', 'wallpaper-old', NULL)",
            [],
        )
        .unwrap();

        assert!(persist_monitor_rotation(&mut conn, "monitor-a", "wallpaper-new").is_err());

        let current: String = conn
            .query_row(
                "SELECT current_wallpaper_id FROM monitor_playlists WHERE monitor_id = 'monitor-a'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(current, "wallpaper-old");
    }

    #[test]
    fn stopped_scheduler_does_not_wait_for_an_interval() {
        let stop = AtomicBool::new(true);
        let wake = Arc::new((Mutex::new(false), Condvar::new()));

        assert!(!wait_for_next_cycle(&stop, &wake, Duration::from_secs(60)));
    }

    #[test]
    fn manual_operation_serializes_application_access_and_stop_is_idempotent() {
        let scheduler = DynamicScheduler::default();
        scheduler.stop();
        scheduler.stop();

        let mut operation = scheduler.begin_manual_operation();
        assert!(scheduler.apply_lock.try_lock().is_err());
        operation.complete();
        drop(operation);
        assert!(scheduler.apply_lock.try_lock().is_ok());
    }

    #[test]
    fn smoke_scheduler_cannot_start_a_worker() {
        assert!(!scheduler_can_start_worker(true));
        assert!(scheduler_can_start_worker(false));
    }
}
