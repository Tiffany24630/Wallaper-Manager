mod commands;
mod db;
mod models;
mod utils;

use commands::{monitor::*, playlist::*, settings::*, system::*, wallpaper::*};
fn main() {
    tauri::Builder::default()
        .setup(|app| {
            db::initialize(app.handle()).map_err(std::io::Error::other)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_wallpapers,
            import_wallpaper,
            set_active_wallpaper,
            toggle_favorite,
            delete_wallpaper,
            get_system_info,
            get_settings,
            save_settings,
            get_monitors,
            get_playlists,
            create_playlist,
            delete_playlist,
            add_wallpaper_to_playlist,
            remove_wallpaper_from_playlist,
            get_playlist_wallpapers,
            scan_wallpaper_folder,
            assign_wallpaper_to_monitor,
            rotate_wallpaper,
            upscale_wallpaper,
        ])
        .plugin(tauri_plugin_dialog::init())
        .run(tauri::generate_context!())
        .expect("No se pudo iniciar Lumina");
}
