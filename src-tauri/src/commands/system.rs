use serde::Serialize;
use sysinfo::{Disks, System};
use crate::db::get_connection;

#[derive(Serialize)]
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
}

#[tauri::command]
pub fn get_system_info() -> Result<SystemInfo, String> {
    let mut system = System::new_all();

    system.refresh_all();

    let disks = Disks::new_with_refreshed_list();
    let mut total_storage = 0_u64;
    let mut available_storage = 0_u64;

    for disk in disks.list() {
        total_storage += disk.total_space();
        available_storage += disk.available_space();
    }

    let conn = get_connection().map_err(|e| e.to_string())?;

    let wallpaper_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM wallpapers",
            [],
            |row| row.get(0),
        )
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
        cpu_usage: system.global_cpu_info().cpu_usage(),
        total_memory: system.total_memory(),
        used_memory: system.used_memory(),
        total_storage,

        used_storage:
            total_storage - available_storage,

        os_name:
            System::name().unwrap_or_default(),

        hostname:
            System::host_name().unwrap_or_default(),

        uptime: System::uptime(),
        wallpaper_count,
        active_wallpaper,
    })
}