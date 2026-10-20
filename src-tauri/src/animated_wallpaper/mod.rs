use std::{
    fs,
    path::{Path, PathBuf},
};

use image::{
    codecs::gif::{GifEncoder, Repeat},
    Delay, Frame, Rgba, RgbaImage,
};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use uuid::Uuid;

#[cfg(target_os = "windows")]
mod desktop_host;
mod multimonitor;
#[cfg(target_os = "windows")]
mod native_window;
mod recovery;

const MAX_PROTOTYPE_MEDIA_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MonitorBounds {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PrototypeWindowOptions {
    decorations: bool,
    focused: bool,
    focusable: bool,
    skip_taskbar: bool,
    always_on_bottom: bool,
    ignore_cursor_events: bool,
}

fn smoke_variant_options(variant: u8) -> Result<PrototypeWindowOptions, String> {
    let baseline = PrototypeWindowOptions {
        decorations: true,
        focused: true,
        focusable: true,
        skip_taskbar: false,
        always_on_bottom: false,
        ignore_cursor_events: false,
    };
    Ok(match variant {
        0 => baseline,
        1 => PrototypeWindowOptions {
            decorations: false,
            ..baseline
        },
        2 => PrototypeWindowOptions {
            decorations: false,
            focused: false,
            focusable: false,
            ..baseline
        },
        3 => PrototypeWindowOptions {
            decorations: false,
            focused: false,
            focusable: false,
            skip_taskbar: true,
            ..baseline
        },
        4 => PrototypeWindowOptions {
            decorations: false,
            focused: false,
            focusable: false,
            skip_taskbar: true,
            always_on_bottom: true,
            ..baseline
        },
        5 => PrototypeWindowOptions {
            decorations: false,
            focused: false,
            focusable: false,
            skip_taskbar: true,
            always_on_bottom: true,
            ignore_cursor_events: true,
        },
        _ => return Err("La variante del smoke test debe estar entre 0 y 5".into()),
    })
}

fn bounds_from_geometry(x: i32, y: i32, width: u32, height: u32) -> Result<MonitorBounds, String> {
    if width == 0 || height == 0 {
        return Err("El monitor no tiene un área visible válida".into());
    }
    Ok(MonitorBounds {
        x,
        y,
        width,
        height,
    })
}

fn validate_media_source(path: &Path) -> Result<&'static str, String> {
    if !path.is_file() {
        return Err("El archivo multimedia seleccionado no existe o no es un archivo".into());
    }
    let metadata = fs::metadata(path)
        .map_err(|_| "No se pudo leer el archivo multimedia seleccionado".to_string())?;
    if metadata.len() == 0 {
        return Err("El archivo multimedia seleccionado está vacío".into());
    }
    if metadata.len() > MAX_PROTOTYPE_MEDIA_BYTES {
        return Err("El medio de prueba supera el límite de 512 MiB".into());
    }
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("mp4") => Ok("mp4"),
        Some("gif") => Ok("gif"),
        _ => Err("El prototipo solo admite archivos locales .mp4 o .gif".into()),
    }
}

fn encode_query_component(value: &str) -> String {
    value.bytes().fold(String::new(), |mut encoded, byte| {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push_str(&format!("{byte:02X}"));
        }
        encoded
    })
}

fn prototype_directory(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("No se pudo obtener AppData de Lumina: {e}"))?
        .join("animated-prototype");
    fs::create_dir_all(&directory)
        .map_err(|e| format!("No se pudo preparar el almacenamiento temporal: {e}"))?;
    Ok(directory)
}

fn copy_to_controlled_storage(
    app: &AppHandle,
    source: &Path,
    extension: &str,
) -> Result<PathBuf, String> {
    let destination = prototype_directory(app)?.join(format!("{}.{}", Uuid::new_v4(), extension));
    fs::copy(source, &destination)
        .map_err(|e| format!("No se pudo copiar el medio al almacenamiento temporal: {e}"))?;
    Ok(destination)
}

fn fill_diagnostic_circle(image: &mut RgbaImage, offset: u32, color: Rgba<u8>) {
    for y in 45..135 {
        for x in offset..offset + 90 {
            let dx = x as i32 - (offset + 45) as i32;
            let dy = y as i32 - 90;
            if dx * dx + dy * dy <= 45 * 45 {
                image.put_pixel(x, y, color);
            }
        }
    }
}

fn create_diagnostic_gif(path: &Path) -> Result<(), String> {
    let mut first = RgbaImage::from_pixel(320, 180, Rgba([220, 38, 38, 255]));
    let mut second = RgbaImage::from_pixel(320, 180, Rgba([37, 99, 235, 255]));
    fill_diagnostic_circle(&mut first, 35, Rgba([255, 255, 255, 255]));
    fill_diagnostic_circle(&mut second, 195, Rgba([250, 204, 21, 255]));
    let file = fs::File::create(path)
        .map_err(|e| format!("No se pudo crear el GIF de diagnóstico: {e}"))?;
    let mut encoder = GifEncoder::new(file);
    encoder
        .set_repeat(Repeat::Infinite)
        .map_err(|e| format!("No se pudo configurar el loop del GIF de diagnóstico: {e}"))?;
    encoder
        .encode_frames([
            Frame::from_parts(first, 0, 0, Delay::from_numer_denom_ms(400, 1)),
            Frame::from_parts(second, 0, 0, Delay::from_numer_denom_ms(400, 1)),
        ])
        .map_err(|e| format!("No se pudo escribir el GIF de diagnóstico: {e}"))?;
    Ok(())
}

#[tauri::command]
pub async fn open_animated_prototype(
    app: AppHandle,
    monitor_index: usize,
    media_path: String,
    diagnostic: bool,
) -> Result<(), String> {
    let source = PathBuf::from(media_path);
    let extension = validate_media_source(&source)?;
    let monitors = app
        .get_webview_window("main")
        .ok_or("No se encontró la ventana principal de Lumina")?
        .available_monitors()
        .map_err(|e| format!("No se pudieron enumerar los monitores: {e}"))?;
    let monitor = monitors
        .get(monitor_index)
        .ok_or("El monitor seleccionado ya no está disponible")?;
    let position = monitor.position();
    let size = monitor.size();
    let bounds = bounds_from_geometry(position.x, position.y, size.width, size.height)?;
    let label = format!("animated-prototype-{monitor_index}");

    if app.get_webview_window(&label).is_some() {
        return Err(
            "Ya existe una superficie de prototipo para ese monitor; ciérrala antes de abrir otra"
                .into(),
        );
    }

    let copied_media = copy_to_controlled_storage(&app, &source, extension)?;
    let media_query = encode_query_component(&copied_media.to_string_lossy());
    let url = WebviewUrl::App(format!("index.html#/animated-prototype?media={media_query}").into());
    let cleanup_path = copied_media.clone();
    let workerw_mode = std::env::var_os("LUMINA_ANIMATED_SMOKE_WORKERW").is_some();
    let options = if workerw_mode {
        smoke_variant_options(5)?
    } else if let Ok(value) = std::env::var("LUMINA_ANIMATED_SMOKE_VARIANT") {
        smoke_variant_options(
            value
                .parse()
                .map_err(|_| "La variante del smoke test no es numérica".to_string())?,
        )?
    } else if diagnostic {
        smoke_variant_options(0)?
    } else {
        smoke_variant_options(5)?
    };
    eprintln!("Animated prototype options: {options:?}");
    let mut builder = WebviewWindowBuilder::new(&app, &label, url)
        .title("Lumina animated wallpaper prototype")
        .decorations(options.decorations)
        .resizable(false)
        .focused(options.focused)
        .focusable(options.focusable)
        .always_on_top(options.focused)
        .position(f64::from(bounds.x), f64::from(bounds.y))
        .inner_size(f64::from(bounds.width), f64::from(bounds.height))
        .visible(false)
        .on_navigation(|url| {
            url.scheme() == "tauri"
                || (url.scheme() == "http" && url.host_str() == Some("tauri.localhost"))
                || (cfg!(dev)
                    && url.scheme() == "http"
                    && matches!(url.host_str(), Some("127.0.0.1") | Some("localhost")))
        });
    builder = builder
        .skip_taskbar(options.skip_taskbar)
        .always_on_bottom(options.always_on_bottom);
    let window = builder.build().map_err(|e| {
        let _ = fs::remove_file(&cleanup_path);
        format!("No se pudo crear la superficie WebView2: {e}")
    })?;

    if workerw_mode {
        #[cfg(target_os = "windows")]
        if let Err(error) = desktop_host::attach_to_worker_w(&window, bounds) {
            let _ = window.close();
            let _ = fs::remove_file(&cleanup_path);
            return Err(format!(
                "No se pudo activar el host WorkerW experimental: {error}"
            ));
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = window.close();
            let _ = fs::remove_file(&cleanup_path);
            return Err("El host WorkerW experimental solo está disponible en Windows".into());
        }
    }

    if let Err(error) = window.set_ignore_cursor_events(options.ignore_cursor_events) {
        let _ = window.close();
        let _ = fs::remove_file(&cleanup_path);
        return Err(format!(
            "No se pudo configurar la interacción del prototipo: {error}"
        ));
    }
    let event_cleanup_path = cleanup_path.clone();
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) {
            let _ = fs::remove_file(&event_cleanup_path);
        }
    });
    if let Err(error) = window.show() {
        let _ = window.close();
        let _ = fs::remove_file(&cleanup_path);
        return Err(format!(
            "No se pudo mostrar la superficie WebView2: {error}"
        ));
    }
    eprintln!("Animated prototype window shown: {label}");
    Ok(())
}

#[tauri::command]
pub fn inspect_animated_desktop_host() -> Result<serde_json::Value, String> {
    #[cfg(target_os = "windows")]
    {
        let inspection = desktop_host::inspect_desktop_structure()?;
        return Ok(serde_json::to_value(inspection).map_err(|error| {
            format!("No se pudo serializar el diagnóstico de Explorer: {error}")
        })?);
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("La inspección de Progman/WorkerW solo está disponible en Windows".into())
    }
}

#[tauri::command]
pub fn validate_animated_desktop_host() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        let result = native_window::validate_registered_desktop_host()?;
        let message = match result {
            desktop_host::HostValidation::Stable => {
                "HOST_STABLE generación física sin cambios".to_string()
            }
            desktop_host::HostValidation::Changed(reason) => {
                eprintln!("[HostMonitor] HOST_INVALIDATED reason={reason}");
                let _ = native_window::close();
                native_window::close_all_physical_surfaces();
                eprintln!("[HostMonitor] SURFACES_CLOSED host_obsolete=true");
                format!("HOST_CHANGED generación invalidada: {reason}")
            }
            desktop_host::HostValidation::Transient(reason) => {
                format!("HOST_TRANSIENT_ERROR se reintentará: {reason}")
            }
        };
        eprintln!("[HostMonitor] {message}");
        Ok(message)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("La validación del host Explorer solo está disponible en Windows".into())
    }
}

#[tauri::command]
pub fn open_native_animated_prototype_test(
    app: AppHandle,
    monitor_index: usize,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let monitors = app
            .get_webview_window("main")
            .ok_or("No se encontró la ventana principal de Lumina")?
            .available_monitors()
            .map_err(|error| format!("No se pudieron enumerar los monitores: {error}"))?;
        let monitor = monitors
            .get(monitor_index)
            .ok_or("El monitor seleccionado ya no está disponible")?;
        let position = monitor.position();
        let size = monitor.size();
        let bounds = bounds_from_geometry(position.x, position.y, size.width, size.height)?;
        return native_window::open(&app, bounds);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, monitor_index);
        Err("El renderer HWND nativo solo está disponible en Windows".into())
    }
}

#[tauri::command]
pub fn open_animated_desktop_poc(app: AppHandle, monitor_index: usize) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let monitors = app
            .get_webview_window("main")
            .ok_or("No se encontró la ventana principal de Lumina")?
            .available_monitors()
            .map_err(|error| format!("No se pudieron enumerar los monitores: {error}"))?;
        let monitor = monitors
            .get(monitor_index)
            .ok_or("El monitor seleccionado ya no está disponible")?;
        let position = monitor.position();
        let size = monitor.size();
        let bounds = bounds_from_geometry(position.x, position.y, size.width, size.height)?;
        return native_window::open_desktop_poc(&app, bounds);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, monitor_index);
        Err("El POC de escritorio solo está disponible en Windows".into())
    }
}

#[tauri::command]
pub fn open_animated_desktop_recovery_poc(
    app: AppHandle,
    monitor_index: usize,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let monitors = app
            .get_webview_window("main")
            .ok_or("No se encontró la ventana principal de Lumina")?
            .available_monitors()
            .map_err(|error| format!("No se pudieron enumerar los monitores: {error}"))?;
        let monitor = monitors
            .get(monitor_index)
            .ok_or("El monitor seleccionado ya no está disponible")?;
        let position = monitor.position();
        let size = monitor.size();
        let bounds = bounds_from_geometry(position.x, position.y, size.width, size.height)?;
        return native_window::open_desktop_recovery_poc(&app, bounds);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, monitor_index);
        Err("El POC de recuperación solo está disponible en Windows".into())
    }
}

#[tauri::command]
pub fn open_animated_multimonitor_poc(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let window = app
            .get_webview_window("main")
            .ok_or("No se encontró la ventana principal de Lumina")?;
        let monitors = window
            .available_monitors()
            .map_err(|error| format!("No se pudieron enumerar los monitores: {error}"))?;
        let native_ids = crate::utils::windows_wallpaper::available_monitor_ids()
            .ok()
            .filter(|ids| ids.len() == monitors.len());
        let dpi = window
            .hwnd()
            .ok()
            .map(|hwnd| unsafe {
                windows_webview2::Win32::UI::HiDpi::GetDpiForWindow(
                    windows_webview2::Win32::Foundation::HWND(hwnd.0),
                )
            })
            .filter(|value| *value != 0)
            .unwrap_or(96);
        let primary_name = window
            .primary_monitor()
            .ok()
            .flatten()
            .and_then(|monitor| monitor.name().cloned());
        let mut actual = Vec::new();
        for (index, monitor) in monitors.iter().enumerate() {
            let position = monitor.position();
            let size = monitor.size();
            let id = native_ids
                .as_ref()
                .and_then(|ids| ids.get(index).cloned())
                .or_else(|| monitor.name().cloned().filter(|name| !name.is_empty()))
                .unwrap_or_else(|| {
                    format!(
                        "display-{}-{}-{}-{}",
                        position.x, position.y, size.width, size.height
                    )
                });
            let spec = multimonitor::MonitorSpec {
                id,
                bounds: multimonitor::LogicalBounds {
                    x: position.x,
                    y: position.y,
                    width: size.width,
                    height: size.height,
                },
                dpi,
                primary: primary_name
                    .as_ref()
                    .zip(monitor.name())
                    .is_some_and(|(primary, name)| primary == name),
                simulated: false,
            };
            eprintln!(
                "[MultiMonitorPOC] REAL_MONITOR id={} bounds=({},{} {}x{}) dpi={} primary={}",
                spec.id,
                spec.bounds.x,
                spec.bounds.y,
                spec.bounds.width,
                spec.bounds.height,
                spec.dpi,
                spec.primary
            );
            actual.push(spec);
        }
        eprintln!(
            "[MultiMonitorPOC] REAL_MONITOR_COUNT={} physical_visual_validation={}",
            actual.len(),
            actual.len() > 1
        );

        for (variant_index, topology) in multimonitor::simulated_topologies().iter().enumerate() {
            let mut coordinator = multimonitor::MonitorCoordinator::default();
            eprintln!("[MultiMonitorPOC] SIMULATED_TOPOLOGY index={variant_index}");
            for monitor in topology {
                eprintln!(
                    "[MultiMonitorPOC] TOPOLOGY_MONITOR id={} bounds=({},{} -> {},{}) dpi={} simulated={}",
                    monitor.id,
                    monitor.bounds.x,
                    monitor.bounds.y,
                    monitor.bounds.right(),
                    monitor.bounds.bottom(),
                    monitor.dpi,
                    monitor.simulated
                );
                coordinator
                    .create(monitor)
                    .map_err(|error| format!("No se pudo crear la instancia simulada: {error}"))?;
                let surface = coordinator
                    .surface(&monitor.id)
                    .ok_or("La superficie simulada no quedó registrada")?;
                eprintln!(
                    "[MultiMonitorPOC] SURFACE monitor={} renderer={} host={} temp={} dpi={}",
                    surface.monitor_id,
                    surface.renderer_id,
                    surface.host_id,
                    surface.temp_id,
                    surface.dpi
                );
                coordinator
                    .update_geometry(&monitor.id, monitor.bounds, monitor.dpi)
                    .map_err(|error| format!("No se pudo actualizar la geometría: {error}"))?;
            }
            eprintln!(
                "[MultiMonitorPOC] SURFACES_CREATED count={} renderer_count={}",
                topology.len(),
                coordinator.active_count()
            );
            let first = &topology[0];
            coordinator.mark_unavailable(&first.id);
            eprintln!(
                "[MultiMonitorPOC] SURFACE_CLOSED monitor={} renderer_count={}",
                first.id,
                coordinator.active_count()
            );
            coordinator
                .create(first)
                .map_err(|error| format!("No se pudo recrear la instancia simulada: {error}"))?;
            eprintln!(
                "[MultiMonitorPOC] SURFACE_RECOVERED monitor={} renderer_count={}",
                first.id,
                coordinator.active_count()
            );
            let second = &topology[1];
            coordinator.mark_unavailable(&second.id);
            eprintln!(
                "[MultiMonitorPOC] MONITOR_UNAVAILABLE monitor={} renderer_count={}",
                second.id,
                coordinator.active_count()
            );
            coordinator
                .create(second)
                .map_err(|error| format!("No se pudo restaurar la instancia simulada: {error}"))?;
            eprintln!(
                "[MultiMonitorPOC] MONITOR_RESTORED monitor={} renderer_count={}",
                second.id,
                coordinator.active_count()
            );
            coordinator.mark_unavailable(&first.id);
            coordinator.mark_unavailable(&second.id);
            eprintln!(
                "[MultiMonitorPOC] SIMULATED_CLEANUP renderer_count={}",
                coordinator.active_count()
            );
        }

        if let Some(monitor) = actual.first() {
            return native_window::open_desktop_poc(
                &app,
                MonitorBounds {
                    x: monitor.bounds.x,
                    y: monitor.bounds.y,
                    width: monitor.bounds.width,
                    height: monitor.bounds.height,
                },
            );
        }
        return Err("Windows no expuso ningún monitor disponible".into());
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        Err("El POC multi-monitor solo está disponible en Windows".into())
    }
}

#[tauri::command]
pub fn open_animated_multimonitor_physical_poc(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let window = app
            .get_webview_window("main")
            .ok_or("No se encontró la ventana principal de Lumina")?;
        let monitors = window
            .available_monitors()
            .map_err(|error| format!("No se pudieron enumerar los monitores: {error}"))?;
        eprintln!(
            "[MultiMonitorPOC] PHYSICAL_MONITOR_COUNT={} physical_visual_validation={}",
            monitors.len(),
            monitors.len() >= 2
        );
        if monitors.len() < 2 {
            eprintln!(
                "[MultiMonitorPOC] MULTIMONITOR_FISICO=NO_PROBADO; se ejecutan únicamente las pruebas lógicas"
            );
            run_simulated_multimonitor_validation()?;
            return Ok(());
        }

        let native_ids = crate::utils::windows_wallpaper::available_monitor_ids()
            .ok()
            .filter(|ids| ids.len() == monitors.len());
        let mut created: Vec<String> = Vec::new();
        for (index, monitor) in monitors.iter().take(2).enumerate() {
            let position = monitor.position();
            let size = monitor.size();
            let id = native_ids
                .as_ref()
                .and_then(|ids| ids.get(index).cloned())
                .or_else(|| monitor.name().cloned().filter(|name| !name.is_empty()))
                .unwrap_or_else(|| format!("display-{}-{}-{}", position.x, position.y, size.width));
            let bounds = bounds_from_geometry(position.x, position.y, size.width, size.height)?;
            let dpi = (monitor.scale_factor() * 96.0).round() as u32;
            eprintln!(
                "[MultiMonitorPOC] PHYSICAL_MONITOR index={} id={} bounds=({},{} {}x{}) dpi={} primary={}",
                index,
                id,
                position.x,
                position.y,
                size.width,
                size.height,
                dpi,
                monitor.name().is_some_and(|name| window.primary_monitor().ok().flatten().and_then(|primary| primary.name().cloned()).as_ref() == Some(name))
            );
            let (host, experiment) = match desktop_host::prepare_modern_host() {
                Ok(value) => value,
                Err(error) => {
                    for previous in &created {
                        let _ = native_window::close_physical_surface(previous);
                    }
                    return Err(format!("No se pudo validar el host para {id}: {error}"));
                }
            };
            if !experiment.host_is_unambiguous {
                eprintln!(
                    "[MultiMonitorPOC] HOST_UNAVAILABLE monitor_id={} candidate_count={}",
                    id, experiment.candidate_count
                );
                for previous in created {
                    let _ = native_window::close_physical_surface(&previous);
                }
                return Err(format!("Host de escritorio no inequívoco para {id}"));
            }
            if let Err(error) = native_window::open_physical_surface(&app, id.clone(), bounds, host)
            {
                for previous in &created {
                    let _ = native_window::close_physical_surface(previous);
                }
                return Err(format!("No se pudo crear la superficie para {id}: {error}"));
            }
            eprintln!(
                "[MultiMonitorPOC] SURFACE_READY monitor_id={} host={} candidate_count={}",
                id, host.1, experiment.candidate_count
            );
            created.push(id);
        }
        eprintln!(
            "[MultiMonitorPOC] PHYSICAL_SURFACES_CREATED count={} independent=true",
            created.len()
        );
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        Err("El POC físico multi-monitor solo está disponible en Windows".into())
    }
}

fn run_simulated_multimonitor_validation() -> Result<(), String> {
    for (variant_index, topology) in multimonitor::simulated_topologies().iter().enumerate() {
        let mut coordinator = multimonitor::MonitorCoordinator::default();
        eprintln!("[MultiMonitorPOC] PHYSICAL_SIMULATED_TOPOLOGY index={variant_index}");
        for monitor in topology {
            coordinator
                .create(monitor)
                .map_err(|error| format!("No se pudo crear la instancia simulada: {error}"))?;
            let surface = coordinator
                .surface(&monitor.id)
                .ok_or("La superficie simulada no quedó registrada")?;
            eprintln!(
                "[MultiMonitorPOC] PHYSICAL_SIMULATED_SURFACE monitor={} renderer={} host={} temp={} bounds=({},{} {}x{}) dpi={}",
                surface.monitor_id,
                surface.renderer_id,
                surface.host_id,
                surface.temp_id,
                surface.bounds.x,
                surface.bounds.y,
                surface.bounds.width,
                surface.bounds.height,
                surface.dpi
            );
            coordinator
                .update_geometry(&monitor.id, monitor.bounds, monitor.dpi)
                .map_err(|error| format!("No se pudo actualizar la geometría: {error}"))?;
        }
        let first = &topology[0];
        let second = &topology[1];
        eprintln!(
            "[MultiMonitorPOC] PHYSICAL_SIMULATED_ACTIVE count={}",
            coordinator.active_count()
        );
        coordinator.mark_unavailable(&first.id);
        eprintln!(
            "[MultiMonitorPOC] PHYSICAL_SIMULATED_CLOSE_A count={}",
            coordinator.active_count()
        );
        coordinator
            .create(first)
            .map_err(|error| format!("No se pudo recuperar A: {error}"))?;
        coordinator.mark_unavailable(&second.id);
        coordinator
            .create(second)
            .map_err(|error| format!("No se pudo restaurar B: {error}"))?;
        coordinator.mark_unavailable(&first.id);
        coordinator.mark_unavailable(&second.id);
        eprintln!(
            "[MultiMonitorPOC] PHYSICAL_SIMULATED_CLEANUP count={}",
            coordinator.active_count()
        );
        let stable = coordinator.observe_host(multimonitor::HostObservation::Stable);
        eprintln!(
            "[HostMonitor] SIMULATED_STABLE reaction={stable:?} generation={} valid={}",
            coordinator.host_generation(),
            coordinator.host_is_valid()
        );
        coordinator.invalidate_host();
        eprintln!("[HostMonitor] SIMULATED_CHANGED reaction=Invalidated");
        coordinator.rediscover_host();
        eprintln!(
            "[HostMonitor] SIMULATED_REDISCOVERED generation={} valid={}",
            coordinator.host_generation(),
            coordinator.host_is_valid()
        );
    }
    Ok(())
}

#[tauri::command]
pub fn close_native_animated_prototype() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        return native_window::close();
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("El renderer HWND nativo solo está disponible en Windows".into())
    }
}

#[tauri::command]
pub fn close_animated_multimonitor_physical_poc(monitor_id: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        return native_window::close_physical_surface(&monitor_id);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = monitor_id;
        Err("El POC físico multi-monitor solo está disponible en Windows".into())
    }
}

#[tauri::command]
pub async fn open_animated_prototype_test(
    app: AppHandle,
    monitor_index: usize,
) -> Result<(), String> {
    let path = prototype_directory(&app)?.join(format!("diagnostic-{}.gif", Uuid::new_v4()));
    create_diagnostic_gif(&path)?;
    let diagnostic = std::env::var_os("LUMINA_ANIMATED_SMOKE_WALLPAPER").is_none();
    let result = open_animated_prototype(
        app,
        monitor_index,
        path.to_string_lossy().into_owned(),
        diagnostic,
    )
    .await;
    let _ = fs::remove_file(path);
    result
}

#[tauri::command]
pub fn close_animated_prototype(app: AppHandle, monitor_index: usize) -> Result<(), String> {
    let label = format!("animated-prototype-{monitor_index}");
    if let Some(window) = app.get_webview_window(&label) {
        window
            .close()
            .map_err(|e| format!("No se pudo cerrar la superficie WebView2: {e}"))?;
    }
    Ok(())
}

pub fn close_all_animated_prototypes(app: &AppHandle) {
    #[cfg(target_os = "windows")]
    {
        let _ = native_window::close();
        native_window::close_all_physical_surfaces();
    }
    for (label, window) in app.webview_windows() {
        if label.starts_with("animated-prototype-") {
            let _ = window.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        bounds_from_geometry, create_diagnostic_gif, encode_query_component, smoke_variant_options,
        validate_media_source,
    };
    use image::GenericImageView;
    use uuid::Uuid;

    #[test]
    fn accepts_negative_monitor_coordinates() {
        assert_eq!(
            bounds_from_geometry(-1920, 0, 1920, 1080).unwrap(),
            super::MonitorBounds {
                x: -1920,
                y: 0,
                width: 1920,
                height: 1080
            }
        );
    }

    #[test]
    fn rejects_empty_monitor_bounds() {
        assert!(bounds_from_geometry(0, 0, 0, 1080).is_err());
        assert!(bounds_from_geometry(0, 0, 1920, 0).is_err());
    }

    #[test]
    fn encodes_paths_for_the_local_route_query() {
        assert_eq!(
            encode_query_component(r"C:\Lumina Media\clip.mp4"),
            "C%3A%5CLumina%20Media%5Cclip.mp4"
        );
    }

    #[test]
    fn prototype_labels_are_isolated_by_monitor_index() {
        assert_eq!(format!("animated-prototype-{}", 0), "animated-prototype-0");
        assert_ne!(
            format!("animated-prototype-{}", 0),
            format!("animated-prototype-{}", 1)
        );
    }

    #[test]
    fn diagnostic_gif_is_readable_and_has_expected_dimensions() {
        let path = std::env::temp_dir().join(format!("lumina-diagnostic-{}.gif", Uuid::new_v4()));
        create_diagnostic_gif(&path).unwrap();
        assert_eq!(validate_media_source(&path).unwrap(), "gif");
        let image = image::open(&path).unwrap();
        assert_eq!(image.dimensions(), (320, 180));
        assert!(std::fs::metadata(&path).unwrap().len() > 0);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn smoke_variants_add_window_flags_incrementally() {
        assert!(!smoke_variant_options(0).unwrap().skip_taskbar);
        assert!(!smoke_variant_options(1).unwrap().skip_taskbar);
        assert!(!smoke_variant_options(2).unwrap().focusable);
        assert!(smoke_variant_options(3).unwrap().skip_taskbar);
        assert!(smoke_variant_options(4).unwrap().always_on_bottom);
        assert!(smoke_variant_options(5).unwrap().ignore_cursor_events);
        assert!(smoke_variant_options(6).is_err());
    }
}
