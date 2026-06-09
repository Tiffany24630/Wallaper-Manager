mod commands;
mod db;
mod models;

use commands::{
    monitor::*,
    playlist::*,
    settings::*,
    system::*,
    wallpaper::*,
};

fn main() {
    db::migrations::run_migrations();

    tauri::Builder::default()
        .invoke_handler(
            tauri::generate_handler![
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
            ]
        )
        .run(tauri::generate_context!(),).expect("error");
}