mod commands;
mod db;
mod dynamic;
mod models;
mod startup;
mod utils;

use commands::{
    cursor::*, download::*, monitor::*, playlist::*, secure_storage::*, settings::*, sync::*,
    system::*, wallpaper::*,
};
use dynamic::DynamicScheduler;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    Manager, WindowEvent,
};

fn should_minimize_to_tray() -> bool {
    db::get_connection()
        .and_then(|connection| {
            connection.query_row("SELECT minimize_to_tray FROM settings LIMIT 1", [], |row| {
                row.get::<_, i32>(0)
            })
        })
        .map(|value| value == 1)
        .unwrap_or(true)
}

fn main() {
    let scheduler = DynamicScheduler::default();
    let scheduler_for_setup = scheduler.clone();
    let scheduler_for_shutdown = scheduler.clone();
    let result = tauri::Builder::default()
        .manage(scheduler)
        .setup(move |app| {
            db::initialize(app.handle()).map_err(std::io::Error::other)?;
            if let Err(error) = restore_active_cursor() {
                eprintln!("No se pudo restaurar el cursor activo: {error}");
            }
            if let Err(error) = restore_monitor_wallpapers(app.handle()) {
                eprintln!("No se pudieron restaurar las asignaciones por monitor: {error}");
            }
            let window = app.get_webview_window("main").ok_or_else(|| {
                std::io::Error::other("No se encontró la ventana principal de Lumina")
            })?;
            let window_for_close = window.clone();
            window.on_window_event(move |event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    if should_minimize_to_tray() {
                        api.prevent_close();
                        let _ = window_for_close.hide();
                    }
                }
            });

            let handle = app.handle().clone();
            let show = MenuItemBuilder::with_id("show", "Abrir Lumina").build(app)?;
            let hide = MenuItemBuilder::with_id("hide", "Ocultar ventana").build(app)?;
            let dynamic = MenuItemBuilder::with_id(
                "dynamic-status",
                "Fondo dinámico: consulta Ajustes para ver el estado",
            )
            .enabled(false)
            .build(app)?;
            let exit = MenuItemBuilder::with_id("exit", "Salir de Lumina").build(app)?;
            let menu = MenuBuilder::new(app)
                .item(&show)
                .item(&hide)
                .separator()
                .item(&dynamic)
                .separator()
                .item(&exit)
                .build()?;
            let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/32x32.png"))?;
            TrayIconBuilder::with_id("main-tray")
                .icon(icon)
                .tooltip("Lumina")
                .menu(&menu)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "hide" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.hide();
                        }
                    }
                    "exit" => app.exit(0),
                    _ => {}
                })
                .build(&handle)?;
            scheduler_for_setup.restart();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_wallpapers,
            import_wallpaper,
            import_synced_wallpaper,
            set_active_wallpaper,
            toggle_favorite,
            delete_wallpaper,
            get_system_info,
            get_settings,
            save_settings,
            get_monitors,
            get_monitor_playlists,
            assign_playlist_to_monitor,
            remove_playlist_from_monitor,
            get_playlists,
            create_playlist,
            delete_playlist,
            add_wallpaper_to_playlist,
            remove_wallpaper_from_playlist,
            get_playlist_wallpapers,
            import_synced_playlist,
            apply_synced_playlist,
            scan_wallpaper_folder,
            assign_wallpaper_to_monitor,
            rotate_random_wallpaper,
            rename_wallpaper,
            rotate_wallpaper_version,
            upscale_wallpaper,
            list_cursors,
            import_cursor,
            import_synced_cursor,
            rename_cursor,
            activate_cursor,
            restore_default_cursor,
            delete_cursor,
            enqueue_sync_operation,
            list_sync_queue,
            claim_sync_operation,
            finish_sync_operation,
            retry_sync_operation,
            record_sync_tombstone,
            list_sync_tombstones,
            mark_playlist_local_change,
            list_sync_playlist_states,
            mark_playlist_synced,
            record_playlist_conflict,
            list_playlist_conflicts,
            get_secure_session_token,
            set_secure_session_token,
            delete_secure_session_token,
            download_and_import_synced_resource,
        ])
        .plugin(tauri_plugin_dialog::init())
        .run(tauri::generate_context!());
    scheduler_for_shutdown.stop();
    result.expect("No se pudo iniciar Lumina");
}
