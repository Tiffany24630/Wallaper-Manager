use crate::{
    commands::settings::DYNAMIC_INTERVALS, commands::wallpaper::set_active_wallpaper_internal,
    db::get_connection,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Clone, Default)]
pub struct DynamicScheduler {
    worker: Arc<Mutex<Option<Worker>>>,
    apply_lock: Arc<Mutex<()>>,
}

struct Worker {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

impl DynamicScheduler {
    pub fn restart(&self) {
        self.stop();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let apply_lock = Arc::clone(&self.apply_lock);
        let handle = thread::spawn(move || run_worker(thread_stop, apply_lock));
        *self.worker.lock().expect("dynamic scheduler lock") = Some(Worker { stop, handle });
    }

    pub fn stop(&self) {
        if let Some(worker) = self.worker.lock().expect("dynamic scheduler lock").take() {
            worker.stop.store(true, Ordering::Release);
            let _ = worker.handle.join();
        }
    }
}

fn run_worker(stop: Arc<AtomicBool>, apply_lock: Arc<Mutex<()>>) {
    loop {
        if stop.load(Ordering::Acquire) {
            return;
        }
        let interval = match apply_next(&apply_lock) {
            Ok(Some(minutes)) => minutes,
            Ok(None) => return,
            Err(error) => {
                set_dynamic_error(&error);
                1
            }
        };
        let seconds = interval.saturating_mul(60);
        for _ in 0..seconds {
            if stop.load(Ordering::Acquire) {
                return;
            }
            thread::sleep(Duration::from_secs(1));
        }
    }
}

fn apply_next(apply_lock: &Mutex<()>) -> Result<Option<u32>, String> {
    let _guard = apply_lock
        .lock()
        .map_err(|_| "No se pudo bloquear el cambio dinámico".to_string())?;
    let conn = get_connection().map_err(|e| e.to_string())?;
    let (enabled, playlist_id, current_id, interval): (i32, Option<String>, Option<String>, u32) = conn
        .query_row(
            "SELECT dynamic_enabled, dynamic_playlist_id, dynamic_current_wallpaper_id, dynamic_interval_minutes FROM settings LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|_| "No hay configuración de fondo dinámico".to_string())?;
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
            "SELECT w.id, w.path FROM playlist_wallpapers pw JOIN wallpapers w ON w.id = pw.wallpaper_id WHERE pw.playlist_id = ?1 ORDER BY pw.rowid",
        )
        .map_err(|e| e.to_string())?;
    let valid: Vec<(String, String)> = stmt
        .query_map([&playlist_id], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
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
        None => return Ok(Some(interval)),
    };
    set_active_wallpaper_internal(next_id.clone())?;
    conn.execute(
        "UPDATE settings SET dynamic_current_wallpaper_id = ?1, dynamic_last_error = NULL WHERE id = (SELECT id FROM settings LIMIT 1)",
        [&next_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(Some(interval))
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
    use super::{select_next_id, DYNAMIC_INTERVALS};

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
}
