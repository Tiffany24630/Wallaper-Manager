use crate::db::get_connection;
use serde::Serialize;
use std::sync::{Mutex, OnceLock};
use sysinfo::{Disks, System};

static SYSTEM: OnceLock<Mutex<System>> = OnceLock::new();

fn used_storage(total: u64, available: u64) -> u64 {
    total.saturating_sub(available)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub cpu_usage: f32,
    pub total_memory: u64,
    pub used_memory: u64,
    pub total_storage: u64,
    pub used_storage: u64,
    pub os_name: String,
    pub hostname: String,
    pub uptime: u64,
    pub wallpaper_count: i64,
    pub active_wallpaper: Option<String>,
    pub cpu_name: String,
    pub logical_cores: usize,
    pub app_memory: u64,
}

#[tauri::command]
pub fn get_system_info() -> Result<SystemInfo, String> {
    let system_state = SYSTEM.get_or_init(|| Mutex::new(System::new_all()));
    let mut system = system_state
        .lock()
        .map_err(|_| "No se pudo acceder a las métricas del sistema".to_string())?;
    system.refresh_cpu();
    system.refresh_memory();
    if let Ok(pid) = sysinfo::get_current_pid() {
        system.refresh_process(pid);
    }

    let cpu_name = system
        .cpus()
        .first()
        .map(|cpu| cpu.brand().to_string())
        .unwrap_or_default();
    let logical_cores = system.cpus().len();
    let app_memory = sysinfo::get_current_pid()
        .ok()
        .and_then(|pid| system.process(pid).map(|process| process.memory()))
        .unwrap_or(0);

    let disks = Disks::new_with_refreshed_list();
    let mut total_storage = 0_u64;
    let mut available_storage = 0_u64;

    for disk in disks.list() {
        total_storage += disk.total_space();
        available_storage += disk.available_space();
    }

    let conn = get_connection().map_err(|e| e.to_string())?;

    let wallpaper_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM wallpapers", [], |row| row.get(0))
        .unwrap_or(0);

    let active_wallpaper: Option<String> = conn
        .query_row(
            "
            SELECT name
            FROM wallpapers
            WHERE active = 1
            LIMIT 1
            ",
            [],
            |row| row.get(0),
        )
        .ok();

    Ok(SystemInfo {
        cpu_usage: system.global_cpu_info().cpu_usage().clamp(0.0, 100.0),
        total_memory: system.total_memory(),
        used_memory: system.used_memory(),
        total_storage,

        used_storage: used_storage(total_storage, available_storage),

        os_name: System::name().unwrap_or_default(),

        hostname: System::host_name().unwrap_or_default(),

        uptime: System::uptime(),
        wallpaper_count,
        active_wallpaper,
        cpu_name,
        logical_cores,
        app_memory,
    })
}

#[cfg(test)]
mod tests {
    use super::used_storage;

    #[test]
    fn storage_usage_never_underflows() {
        assert_eq!(used_storage(1_000, 250), 750);
        assert_eq!(used_storage(250, 1_000), 0);
    }
}
