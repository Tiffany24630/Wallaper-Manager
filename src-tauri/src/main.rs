mod animated_wallpaper;
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
use models::wallpaper_coordinator;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};

fn smoke_diagnostic(stage: &str, detail: Option<String>) {
    if let Err(error) = db::record_smoke_diagnostic(stage, detail.as_deref()) {
        eprintln!("LUMINA_SMOKE_DIAGNOSTIC_FAILED stage={stage} error={error}");
    }
}

#[cfg(test)]
mod tests {
    use super::minimize_to_tray_value;

    #[test]
    fn minimize_to_tray_preserves_persisted_boolean() {
        assert_eq!(minimize_to_tray_value(Ok(1)).unwrap(), true);
        assert_eq!(minimize_to_tray_value(Ok(0)).unwrap(), false);
    }

    #[test]
    fn minimize_to_tray_does_not_hide_read_errors() {
        let error = minimize_to_tray_value(Err("sqlite unavailable".into())).unwrap_err();
        assert_eq!(error, "sqlite unavailable");
    }
}

fn smoke_thread_detail() -> serde_json::Value {
    serde_json::json!({
        "thread_id": format!("{:?}", std::thread::current().id()),
        "thread_name": std::thread::current().name(),
    })
}

fn record_main_window_state(window: &tauri::WebviewWindow, stage: &str) {
    smoke_diagnostic(
        &format!("{stage}_inner_size_started"),
        Some(smoke_thread_detail().to_string()),
    );
    let inner = window
        .inner_size()
        .map(|size| {
            smoke_diagnostic(
                &format!("{stage}_inner_size_finished"),
                Some(format!("{}x{}", size.width, size.height)),
            );
            format!("{}x{}", size.width, size.height)
        })
        .unwrap_or_else(|error| {
            smoke_diagnostic(
                &format!("{stage}_inner_size_failed"),
                Some(error.to_string()),
            );
            format!("error:{error}")
        });
    smoke_diagnostic(
        &format!("{stage}_outer_size_started"),
        Some(smoke_thread_detail().to_string()),
    );
    let outer = window
        .outer_size()
        .map(|size| {
            smoke_diagnostic(
                &format!("{stage}_outer_size_finished"),
                Some(format!("{}x{}", size.width, size.height)),
            );
            format!("{}x{}", size.width, size.height)
        })
        .unwrap_or_else(|error| {
            smoke_diagnostic(
                &format!("{stage}_outer_size_failed"),
                Some(error.to_string()),
            );
            format!("error:{error}")
        });
    smoke_diagnostic(
        &format!("{stage}_visibility_started"),
        Some(smoke_thread_detail().to_string()),
    );
    let visible = window
        .is_visible()
        .map(|value| {
            smoke_diagnostic(
                &format!("{stage}_visibility_finished"),
                Some(value.to_string()),
            );
            value.to_string()
        })
        .unwrap_or_else(|error| {
            smoke_diagnostic(
                &format!("{stage}_visibility_failed"),
                Some(error.to_string()),
            );
            format!("error:{error}")
        });
    smoke_diagnostic(
        &format!("{stage}_minimized_started"),
        Some(smoke_thread_detail().to_string()),
    );
    let minimized = window
        .is_minimized()
        .map(|value| {
            smoke_diagnostic(
                &format!("{stage}_minimized_finished"),
                Some(value.to_string()),
            );
            value.to_string()
        })
        .unwrap_or_else(|error| {
            smoke_diagnostic(
                &format!("{stage}_minimized_failed"),
                Some(error.to_string()),
            );
            format!("error:{error}")
        });
    smoke_diagnostic(
        &format!("{stage}_url_started"),
        Some(smoke_thread_detail().to_string()),
    );
    let url = window
        .url()
        .map(|value| {
            format!(
                "{}://{}{}",
                value.scheme(),
                value.host_str().unwrap_or(""),
                value.path()
            )
        })
        .map(|value| {
            smoke_diagnostic(&format!("{stage}_url_finished"), Some(value.clone()));
            value
        })
        .unwrap_or_else(|error| {
            smoke_diagnostic(&format!("{stage}_url_failed"), Some(error.to_string()));
            format!("error:{error}")
        });
    let detail = serde_json::json!({
        "label": window.label(),
        "inner": inner,
        "outer": outer,
        "visible": visible,
        "minimized": minimized,
        "url": url,
    })
    .to_string();
    smoke_diagnostic(stage, Some(detail));
}

fn spawn_smoke_differential_windows(app: &tauri::AppHandle) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let about_blank_script = r#"
          (() => {
            document.documentElement.innerHTML = `
              <head><title>Lumina WebView smoke A</title></head>
              <body style="margin:0;background:#f7f7f7;color:#202020;font:24px sans-serif;display:grid;place-items:center;height:100vh">
                <main><h1>WebView2 navigation A</h1><p>about:blank document initialized</p></main>
              </body>`;
          })();
        "#;
        let a_url = match "about:blank".parse() {
            Ok(url) => WebviewUrl::External(url),
            Err(error) => {
                smoke_diagnostic("smoke_diff_a_url_invalid", Some(error.to_string()));
                return;
            }
        };
        let a_result = WebviewWindowBuilder::new(&handle, "smoke-diff-a", a_url)
            .title("Lumina WebView smoke A")
            .inner_size(640.0, 360.0)
            .visible(true)
            .initialization_script(about_blank_script)
            .on_navigation(|url| {
                smoke_diagnostic("smoke_diff_a_navigation", Some(url.to_string()));
                true
            })
            .on_page_load(|webview, payload| {
                let detail = serde_json::json!({
                    "label": webview.label(),
                    "event": format!("{:?}", payload.event()),
                    "url": payload.url().to_string(),
                })
                .to_string();
                smoke_diagnostic("smoke_diff_a_page_load", Some(detail));
            })
            .build();
        match a_result {
            Ok(window) => smoke_diagnostic(
                "smoke_diff_a_created",
                Some(format!("label={} configured=640x360", window.label())),
            ),
            Err(error) => smoke_diagnostic("smoke_diff_a_create_failed", Some(error.to_string())),
        }

        let b_result = WebviewWindowBuilder::new(
            &handle,
            "smoke-diff-b",
            WebviewUrl::App("smoke-protocol.html".into()),
        )
        .title("Lumina WebView smoke B")
        .inner_size(640.0, 360.0)
        .visible(true)
        .on_navigation(|url| {
            smoke_diagnostic("smoke_diff_b_navigation", Some(url.to_string()));
            true
        })
        .on_page_load(|webview, payload| {
            let detail = serde_json::json!({
                "label": webview.label(),
                "event": format!("{:?}", payload.event()),
                "url": payload.url().to_string(),
            })
            .to_string();
            smoke_diagnostic("smoke_diff_b_page_load", Some(detail));
        })
        .build();
        match b_result {
            Ok(window) => smoke_diagnostic(
                "smoke_diff_b_created",
                Some(format!(
                    "label={} app_path=smoke-protocol.html",
                    window.label()
                )),
            ),
            Err(error) => smoke_diagnostic("smoke_diff_b_create_failed", Some(error.to_string())),
        }
    });
}

#[tauri::command]
fn record_smoke_diagnostic(stage: String, detail: Option<String>) -> Result<bool, String> {
    db::record_smoke_diagnostic(&stage, detail.as_deref())
}

#[tauri::command]
fn request_smoke_exit(
    app: tauri::AppHandle,
    token: String,
    reason: Option<String>,
    code: Option<i32>,
) -> Result<(), String> {
    db::smoke_token_matches(&token)?;
    let reason = reason.unwrap_or_else(|| "requested".into());
    if reason.len() > 256 {
        return Err("El motivo de salida smoke es demasiado largo".into());
    }
    let code = code.unwrap_or(0);
    smoke_diagnostic(
        "smoke_exit_requested",
        Some(serde_json::json!({ "reason": reason, "code": code }).to_string()),
    );
    app.exit(code);
    Ok(())
}

fn should_minimize_to_tray() -> Result<bool, String> {
    let connection = db::get_connection()
        .map_err(|error| format!("No se pudo leer la preferencia minimize_to_tray: {error}"))?;
    let value = connection
        .query_row("SELECT minimize_to_tray FROM settings LIMIT 1", [], |row| {
            row.get::<_, i32>(0)
        })
        .map_err(|error| format!("No se pudo leer la preferencia minimize_to_tray: {error}"))?;
    minimize_to_tray_value(Ok(value))
}

fn minimize_to_tray_value(value: Result<i32, String>) -> Result<bool, String> {
    value.map(|value| value == 1)
}

fn shutdown_runtime(app: &tauri::AppHandle, scheduler: &DynamicScheduler) {
    // Stop producers before closing their consumers. This prevents a final
    // scheduler rotation from reserving work while surfaces are being torn
    // down. The coordinator remains in ShuttingDown until surface cleanup has
    // had a chance to release its own assignments.
    scheduler.stop();
    if let Ok(mut coordinator) = wallpaper_coordinator::global().lock() {
        match coordinator.begin_shutdown() {
            Ok(cancelled) => {
                eprintln!("WALLPAPER_COORDINATOR_SHUTTING_DOWN cancelled_pending={cancelled}")
            }
            Err(error) => eprintln!("WALLPAPER_COORDINATOR_SHUTDOWN_BEGIN_FAILED error={error}"),
        }
    } else {
        eprintln!("WALLPAPER_COORDINATOR_SHUTDOWN_BEGIN_FAILED error=lock");
    }
    let pending_surface_confirmations = if db::smoke_mode_requested().unwrap_or(false) {
        smoke_diagnostic("animated_shutdown_skipped_smoke", None);
        0
    } else {
        animated_wallpaper::close_all_animated_prototypes(app)
    };
    if pending_surface_confirmations != 0 {
        eprintln!(
            "WALLPAPER_COORDINATOR_CLOSE_CONFIRMATION_PENDING count={pending_surface_confirmations}"
        );
    }
    if let Ok(mut coordinator) = wallpaper_coordinator::global().lock() {
        match coordinator.finish_shutdown() {
            Ok(()) => eprintln!("WALLPAPER_COORDINATOR_SHUTDOWN_COMPLETE"),
            Err(error) => eprintln!("WALLPAPER_COORDINATOR_SHUTDOWN_INCOMPLETE error={error}"),
        }
    } else {
        eprintln!("WALLPAPER_COORDINATOR_SHUTDOWN_INCOMPLETE error=lock");
    }
}

fn main() {
    eprintln!("LUMINA_START main_entered");
    let smoke_mode = match db::smoke_mode_requested() {
        Ok(value) => value,
        Err(error) => {
            eprintln!("LUMINA_SMOKE_ARGUMENTS_INVALID error={error}");
            return;
        }
    };
    let smoke_exit_after_init = match db::smoke_exit_after_init_requested() {
        Ok(value) => value,
        Err(error) => {
            eprintln!("LUMINA_SMOKE_ARGUMENTS_INVALID error={error}");
            return;
        }
    };
    if smoke_mode {
        match db::prepare_smoke_webview_data_dir() {
            Ok(Some(webview_dir)) => std::env::set_var("LOCALAPPDATA", webview_dir),
            Ok(None) => unreachable!("smoke mode requires a smoke WebView directory"),
            Err(error) => {
                eprintln!("LUMINA_SMOKE_WEBVIEW_SETUP_FAILED error={error}");
                return;
            }
        }
    }
    if let Err(error) = db::record_smoke_diagnostic_before_database("setup_started", None) {
        eprintln!("LUMINA_SMOKE_DIAGNOSTIC_FAILED stage=setup_started error={error}");
    }
    let mut context = tauri::generate_context!();
    let smoke_window_config = if smoke_mode {
        let config = context
            .config()
            .app
            .windows
            .iter()
            .find(|window| window.label == "main")
            .cloned();
        context.config_mut().app.windows.clear();
        config
    } else {
        None
    };
    let scheduler = DynamicScheduler::default();
    let scheduler_for_setup = scheduler.clone();
    let scheduler_for_shutdown = scheduler.clone();
    let result = tauri::Builder::default()
        .manage(scheduler)
        .on_page_load(|webview, payload| {
            let detail = serde_json::json!({
                "label": webview.label(),
                "event": format!("{:?}", payload.event()),
                "url": format!("{}://{}{}", payload.url().scheme(), payload.url().host_str().unwrap_or(""), payload.url().path()),
            })
            .to_string();
            smoke_diagnostic("webview_page_load", Some(detail));
        })
        .setup(move |app| {
            scheduler_for_setup.set_app_handle(app.handle().clone());
            db::initialize(app.handle()).map_err(std::io::Error::other)?;
            smoke_diagnostic("sqlite_initialized", None);
            wallpaper_coordinator::global()
                .lock()
                .map_err(|_| std::io::Error::other("No se pudo bloquear el coordinador"))?
                .initialize()
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            eprintln!("WALLPAPER_COORDINATOR_READY");
            smoke_diagnostic("coordinator_initialized", None);
            if smoke_mode {
                smoke_diagnostic("animated_startup_skipped_smoke", None);
            } else {
                smoke_diagnostic("animated_startup_started", None);
                if let Err(error) = animated_wallpaper::prepare_animated_startup(app.handle()) {
                    smoke_diagnostic("animated_startup_failed", Some(error.to_string()));
                    eprintln!("No se pudo preparar el arranque limpio del POC animado: {error}");
                }
            }
            if smoke_mode {
                smoke_diagnostic("cursor_restore_skipped_smoke", None);
            } else {
                smoke_diagnostic("cursor_restore_started", None);
                if let Err(error) = restore_active_cursor() {
                    smoke_diagnostic("cursor_restore_failed", Some(error.clone()));
                    eprintln!("No se pudo restaurar el cursor activo: {error}");
                }
            }
            if smoke_mode {
                smoke_diagnostic("monitor_restore_skipped_smoke", None);
            } else if smoke_window_config.is_none() {
                smoke_diagnostic("monitor_restore_started", None);
                if let Err(error) = restore_monitor_wallpapers(app.handle()) {
                smoke_diagnostic("monitor_restore_failed", Some(error.clone()));
                eprintln!("No se pudieron restaurar las asignaciones por monitor: {error}");
                }
            } else {
                smoke_diagnostic("monitor_restore_deferred_until_main_window", None);
            }
            smoke_diagnostic("main_window_lookup_started", None);
            let window = app.get_webview_window("main");
            let window = window;
            if let Some(ref window) = window {
                smoke_diagnostic("main_window_lookup_succeeded", None);
                let window_for_close = window.clone();
                window.on_window_event(move |event| {
                    let stage = match event {
                        WindowEvent::Resized(_) => "main_window_resized",
                        WindowEvent::Moved(_) => "main_window_moved",
                        WindowEvent::Focused(_) => "main_window_focused",
                        WindowEvent::CloseRequested { .. } => "main_window_close_requested",
                        WindowEvent::Destroyed => "main_window_destroyed",
                        _ => "main_window_event",
                    };
                    smoke_diagnostic(stage, None);
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        match should_minimize_to_tray() {
                            Ok(true) => {
                                api.prevent_close();
                                let _ = window_for_close.hide();
                            }
                            Ok(false) => {}
                            Err(error) => {
                                eprintln!("No se pudo decidir minimizar al tray; se permite el cierre: {error}");
                                smoke_diagnostic("minimize_to_tray_read_failed", Some(error));
                            }
                        }
                    }
                });
                smoke_diagnostic("main_window_event_listener_registered", None);
                record_main_window_state(&window, "main_window_state_before_handlers");
            } else if smoke_window_config.is_none() {
                return Err(std::io::Error::other("No se encontró la ventana principal de Lumina").into());
            } else {
                smoke_diagnostic("main_window_lookup_deferred", None);
            }

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
            if let Some(window_config) = smoke_window_config.clone() {
                let window_handle = handle.clone();
                let webview_data_dir = db::app_data_dir()
                    .map_err(|error| std::io::Error::other(error.to_string()))?
                    .join("webview");
                std::fs::create_dir_all(&webview_data_dir)?;
                tauri::async_runtime::spawn(async move {
                    let mut configured = serde_json::json!({
                        "label": window_config.label.clone(),
                        "url": format!("{:?}", window_config.url),
                        "width": window_config.width,
                        "height": window_config.height,
                        "visible": window_config.visible,
                        "pid": std::process::id(),
                    });
                    if let Some(object) = configured.as_object_mut() {
                        object.insert("execution".into(), smoke_thread_detail());
                    }
                    let configured = configured.to_string();
                    smoke_diagnostic("main_window_build_started", Some(configured));
                    let builder = match WebviewWindowBuilder::from_config(&window_handle, &window_config) {
                        Ok(builder) => {
                            smoke_diagnostic(
                                "main_window_builder_configured",
                                Some(smoke_thread_detail().to_string()),
                            );
                            builder
                                .data_directory(webview_data_dir)
                                .initialization_script(if smoke_exit_after_init {
                                    format!(
                                        "window.__LUMINA_SMOKE__ = true; window.__LUMINA_SMOKE_EXIT__ = true; window.__LUMINA_SMOKE_TOKEN__ = {};",
                                        serde_json::to_string(&db::smoke_token().ok().flatten()).unwrap_or_else(|_| "null".into())
                                    )
                                } else {
                                    "window.__LUMINA_SMOKE__ = true;".into()
                                })
                                .on_navigation(|url| {
                                    smoke_diagnostic(
                                        "webview_navigation_requested",
                                        Some(url.to_string()),
                                    );
                                    true
                                })
                                .on_page_load(|webview, payload| {
                                    let detail = serde_json::json!({
                                        "label": webview.label(),
                                        "event": format!("{:?}", payload.event()),
                                        "url": format!("{}://{}{}", payload.url().scheme(), payload.url().host_str().unwrap_or(""), payload.url().path()),
                                    })
                                    .to_string();
                                    smoke_diagnostic("webview_page_load_dynamic", Some(detail));
                                })
                        }
                        Err(error) => {
                            smoke_diagnostic("main_window_build_failed", Some(error.to_string()));
                            return;
                        }
                    };
                    smoke_diagnostic(
                        "main_window_native_build_started",
                        Some(smoke_thread_detail().to_string()),
                    );
                    let build_result = builder.build();
                    smoke_diagnostic(
                        "main_window_build_returned",
                        Some(
                            serde_json::json!({
                                "result": if build_result.is_ok() { "ok" } else { "error" },
                                "execution": smoke_thread_detail(),
                            })
                            .to_string(),
                        ),
                    );
                    match build_result {
                        Ok(window) => {
                            smoke_diagnostic(
                                "main_window_build_succeeded",
                                Some(smoke_thread_detail().to_string()),
                            );
                            smoke_diagnostic("main_window_created_with_smoke_webview_dir", None);
                            let window_for_close = window.clone();
                            window.on_window_event(move |event| {
                                let stage = match event {
                                    WindowEvent::Resized(_) => "main_window_resized",
                                    WindowEvent::Moved(_) => "main_window_moved",
                                    WindowEvent::Focused(_) => "main_window_focused",
                                    WindowEvent::CloseRequested { .. } => "main_window_close_requested",
                                    WindowEvent::Destroyed => "main_window_destroyed",
                                    _ => "main_window_event",
                                };
                                smoke_diagnostic(stage, None);
                                if let WindowEvent::CloseRequested { api, .. } = event {
                                    match should_minimize_to_tray() {
                                        Ok(true) => {
                                            api.prevent_close();
                                            let _ = window_for_close.hide();
                                        }
                                        Ok(false) => {}
                                        Err(error) => {
                                            eprintln!("No se pudo decidir minimizar al tray; se permite el cierre: {error}");
                                            smoke_diagnostic("minimize_to_tray_read_failed", Some(error));
                                        }
                                    }
                                }
                            });
                            smoke_diagnostic(
                                "main_window_event_listener_registered",
                                Some(smoke_thread_detail().to_string()),
                            );
                            record_main_window_state(&window, "main_window_post_build_state");
                        }
                        Err(error) => {
                            smoke_diagnostic("main_window_build_failed", Some(error.to_string()))
                        }
                    }
                });
            }
            if smoke_window_config.is_some() {
                spawn_smoke_differential_windows(app.handle());
            }
            if !smoke_mode && std::env::var_os("LUMINA_ANIMATED_SMOKE_TEST").is_some() {
                let smoke_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let result = match animated_wallpaper::primary_monitor_identity(&smoke_handle) {
                        Ok(monitor_id) => {
                            animated_wallpaper::open_animated_prototype_test(
                                smoke_handle,
                                monitor_id,
                            )
                            .await
                        }
                        Err(error) => Err(error),
                    };
                    if let Err(error) = result {
                        eprintln!("Animated WebView2 smoke test failed: {error}");
                    }
                });
            }
            if !smoke_mode && std::env::var_os("LUMINA_ANIMATED_MEDIA_ERROR_TEST").is_some() {
                let media_error_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let result =
                        match animated_wallpaper::primary_monitor_identity(&media_error_handle) {
                            Ok(monitor_id) => {
                                animated_wallpaper::open_animated_prototype_test(
                                    media_error_handle,
                                    monitor_id,
                                )
                                .await
                            }
                            Err(error) => Err(error),
                        };
                    if let Err(error) = result {
                        eprintln!("Animated media error test failed: {error}");
                    }
                });
            }
            if !smoke_mode {
            if std::env::var_os("LUMINA_ANIMATED_NATIVE_MEDIA_ERROR_TEST").is_some() {
                let monitor_id =
                    animated_wallpaper::primary_monitor_identity(app.handle()).unwrap_or_default();
                match animated_wallpaper::open_animated_desktop_poc(
                    app.handle().clone(),
                    monitor_id,
                ) {
                    Ok(()) => eprintln!("Native animated media error test started"),
                    Err(error) => eprintln!("Native animated media error test failed: {error}"),
                }
            } else if std::env::var_os("LUMINA_NATIVE_ANIMATED_TEST").is_some() {
                match animated_wallpaper::open_native_animated_prototype_test(
                    app.handle().clone(),
                    animated_wallpaper::primary_monitor_identity(app.handle()).unwrap_or_default(),
                ) {
                    Ok(()) => eprintln!("Native animated WebView2 smoke test started"),
                    Err(error) => eprintln!("Native animated WebView2 smoke test failed: {error}"),
                }
            }
            if std::env::var_os("LUMINA_ANIMATED_DESKTOP_POC").is_some() {
                let monitor_id =
                    animated_wallpaper::primary_monitor_identity(app.handle()).unwrap_or_default();
                match animated_wallpaper::open_animated_desktop_poc(
                    app.handle().clone(),
                    monitor_id,
                ) {
                    Ok(()) => eprintln!("Animated desktop POC started"),
                    Err(error) => eprintln!("Animated desktop POC refused or failed: {error}"),
                }
            } else if std::env::var_os("LUMINA_ANIMATED_RECOVERY_TEST").is_some() {
                match animated_wallpaper::open_animated_desktop_recovery_poc(
                    app.handle().clone(),
                    animated_wallpaper::primary_monitor_identity(app.handle()).unwrap_or_default(),
                ) {
                    Ok(()) => eprintln!("Animated desktop recovery POC started"),
                    Err(error) => eprintln!("Animated desktop recovery POC failed: {error}"),
                }
            } else if std::env::var_os("LUMINA_ANIMATED_LIFECYCLE_TEST").is_some() {
                let monitor_id =
                    animated_wallpaper::primary_monitor_identity(app.handle()).unwrap_or_default();
                match animated_wallpaper::open_animated_lifecycle_poc(
                    app.handle().clone(),
                    monitor_id,
                ) {
                    Ok(()) => eprintln!("Animated lifecycle POC started"),
                    Err(error) => eprintln!("Animated lifecycle POC failed: {error}"),
                }
            } else if std::env::var_os("LUMINA_ANIMATED_MULTIMONITOR_TEST").is_some() {
                match animated_wallpaper::open_animated_multimonitor_poc(app.handle().clone()) {
                    Ok(()) => eprintln!("Animated multi-monitor POC started"),
                    Err(error) => eprintln!("Animated multi-monitor POC failed: {error}"),
                }
            } else if std::env::var_os("LUMINA_ANIMATED_HOST_WATCHER_TEST").is_some() {
                match animated_wallpaper::open_animated_host_watcher_poc(app.handle().clone()) {
                    Ok(()) => eprintln!("Animated host watcher POC started"),
                    Err(error) => eprintln!("Animated host watcher POC failed: {error}"),
                }
            } else if std::env::var_os("LUMINA_ANIMATED_MULTIMONITOR_PHYSICAL_TEST").is_some() {
                match animated_wallpaper::open_animated_multimonitor_physical_poc(
                    app.handle().clone(),
                ) {
                    Ok(()) => eprintln!("Animated physical multi-monitor POC started"),
                    Err(error) => eprintln!("Animated physical multi-monitor POC failed: {error}"),
                }
            }
            if std::env::var_os("LUMINA_ANIMATED_DESKTOP_INSPECT").is_some() {
                match animated_wallpaper::inspect_animated_desktop_host() {
                    Ok(value) => eprintln!(
                        "Animated desktop structure:\n{}",
                        serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
                    ),
                    Err(error) => eprintln!("Animated desktop inspection failed: {error}"),
                }
            }
            if std::env::var_os("LUMINA_ANIMATED_HOST_MONITOR").is_some() {
                match animated_wallpaper::validate_animated_desktop_host() {
                    Ok(message) => eprintln!("Animated host monitor result: {message}"),
                    Err(error) => eprintln!("Animated host monitor failed: {error}"),
                }
            }
            }
            if let Some(window) = window.as_ref() {
                record_main_window_state(window, "main_window_state_after_tray");
            }
            scheduler_for_setup.restart();
            smoke_diagnostic(
                "setup_finished",
                Some(smoke_thread_detail().to_string()),
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_wallpapers,
            list_wallpaper_derivative_ids,
            regenerate_wallpaper_thumbnail,
            import_wallpaper,
            import_synced_wallpaper,
            set_active_wallpaper,
            toggle_favorite,
            delete_wallpaper,
            get_system_info,
            get_settings,
            save_settings,
            get_dynamic_configuration_diagnostic,
            get_monitors,
            get_monitor_wallpapers,
            get_monitor_playlists,
            assign_playlist_to_monitor,
            remove_playlist_from_monitor,
            get_playlists,
            create_playlist,
            rename_playlist,
            reorder_playlist,
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
            record_sync_conflict,
            list_sync_conflicts,
            resolve_sync_conflict,
            record_sync_conflict_error,
            get_secure_session_token,
            set_secure_session_token,
            delete_secure_session_token,
            download_and_import_synced_resource,
            animated_wallpaper::open_animated_prototype,
            animated_wallpaper::open_animated_prototype_test,
            animated_wallpaper::inspect_animated_desktop_host,
            animated_wallpaper::validate_animated_desktop_host,
            animated_wallpaper::open_native_animated_prototype_test,
            animated_wallpaper::open_animated_desktop_poc,
            animated_wallpaper::open_animated_desktop_recovery_poc,
            animated_wallpaper::open_animated_multimonitor_poc,
            animated_wallpaper::open_animated_host_watcher_poc,
            animated_wallpaper::open_animated_multimonitor_physical_poc,
            animated_wallpaper::close_native_animated_prototype,
            animated_wallpaper::close_animated_multimonitor_physical_poc,
            animated_wallpaper::close_animated_prototype,
            animated_wallpaper::report_animated_ready,
            animated_wallpaper::report_animated_media_error,
            record_smoke_diagnostic,
            request_smoke_exit,
        ])
        .plugin(tauri_plugin_dialog::init())
        .build(context);
    match result {
        Ok(app) => app.run(move |app, event| {
            if matches!(event, RunEvent::Ready) {
                smoke_diagnostic("event_loop_ready", Some(smoke_thread_detail().to_string()));
            }
            if matches!(event, RunEvent::ExitRequested { .. } | RunEvent::Exit) {
                smoke_diagnostic("event_loop_exit", None);
                shutdown_runtime(app, &scheduler_for_shutdown);
            }
        }),
        Err(error) => {
            scheduler_for_shutdown.stop();
            panic!("No se pudo iniciar Lumina: {error}");
        }
    }
}
