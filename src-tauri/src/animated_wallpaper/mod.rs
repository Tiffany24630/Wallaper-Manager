use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use image::{
    codecs::gif::{GifDecoder, GifEncoder, Repeat},
    AnimationDecoder, Delay, Frame, ImageDecoder, Rgba, RgbaImage,
};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use uuid::Uuid;

#[cfg(target_os = "windows")]
mod desktop_host;
mod multimonitor;
#[cfg(target_os = "windows")]
mod native_window;
mod recovery;

use recovery::{
    AnimatedSurfaceLifecycle, AnimatedSurfaceState, FailureDecision, SurfaceFailureKind,
};

const MAX_PROTOTYPE_MEDIA_BYTES: u64 = 512 * 1024 * 1024;
pub(crate) const MAX_ANIMATED_FILE_BYTES: u64 = 128 * 1024 * 1024;
pub(crate) const MAX_ANIMATED_DIMENSION: u32 = 8192;
pub(crate) const MAX_ANIMATED_PIXEL_AREA: u64 = 33_177_600;
pub(crate) const MAX_ESTIMATED_DECODED_FRAME_BYTES: u64 = 64 * 1024 * 1024;
pub(crate) const MAX_ANIMATED_FRAMES: u32 = 240;
pub(crate) const MAX_ANIMATED_DURATION_MS: u64 = 10 * 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AnimatedResourceInfo {
    pub(crate) file_bytes: u64,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pixel_area: u64,
    pub(crate) frame_count: u32,
    pub(crate) duration_ms: u64,
    pub(crate) estimated_decoded_frame_bytes: u64,
}

fn resource_limit_reason(
    file_bytes: u64,
    width: u32,
    height: u32,
    monitor_width: u32,
    monitor_height: u32,
) -> Option<&'static str> {
    if file_bytes > MAX_PROTOTYPE_MEDIA_BYTES || file_bytes > MAX_ANIMATED_FILE_BYTES {
        return Some("ResourceLimitExceeded:file_size");
    }
    if width == 0
        || height == 0
        || width > MAX_ANIMATED_DIMENSION
        || height > MAX_ANIMATED_DIMENSION
    {
        return Some("ResourceLimitExceeded:dimensions");
    }
    let area = u64::from(width).saturating_mul(u64::from(height));
    if area > MAX_ANIMATED_PIXEL_AREA {
        return Some("ResourceLimitExceeded:pixel_area");
    }
    if monitor_width > 0
        && monitor_height > 0
        && (width > monitor_width.saturating_mul(4) || height > monitor_height.saturating_mul(4))
    {
        return Some("ResourceLimitExceeded:monitor_ratio");
    }
    if area.saturating_mul(4) > MAX_ESTIMATED_DECODED_FRAME_BYTES {
        return Some("ResourceLimitExceeded:decoded_frame");
    }
    None
}

pub(crate) fn validate_animated_gif(
    path: &Path,
    monitor_width: u32,
    monitor_height: u32,
) -> Result<AnimatedResourceInfo, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("MediaError: no se pudo leer el archivo: {error}"))?;
    if !metadata.is_file() {
        return Err("MediaError: la ruta no es un archivo".into());
    }
    let file_bytes = metadata.len();
    if file_bytes == 0 {
        return Err("MediaError: el archivo está vacío".into());
    }
    if file_bytes > MAX_PROTOTYPE_MEDIA_BYTES || file_bytes > MAX_ANIMATED_FILE_BYTES {
        return Err(format!(
            "ResourceLimitExceeded: archivo de {file_bytes} bytes supera el límite práctico de {} bytes (techo absoluto {} bytes)",
            MAX_ANIMATED_FILE_BYTES, MAX_PROTOTYPE_MEDIA_BYTES
        ));
    }
    if path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
        != Some("gif")
    {
        return Err("MediaError: el POC animado solo admite GIF".into());
    }

    let file = fs::File::open(path)
        .map_err(|error| format!("MediaError: no se pudo abrir el GIF: {error}"))?;
    let decoder = GifDecoder::new(std::io::BufReader::new(file))
        .map_err(|error| format!("MediaError: formato GIF inválido: {error}"))?;
    let (width, height) = decoder.dimensions();
    if let Some(reason) =
        resource_limit_reason(file_bytes, width, height, monitor_width, monitor_height)
    {
        return Err(reason.to_string());
    }
    let pixel_area = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| "ResourceLimitExceeded: overflow en el área del GIF".to_string())?;
    if width == 0
        || height == 0
        || width > MAX_ANIMATED_DIMENSION
        || height > MAX_ANIMATED_DIMENSION
    {
        return Err(format!(
            "ResourceLimitExceeded: dimensiones {width}x{height} superan el máximo de {MAX_ANIMATED_DIMENSION}"
        ));
    }
    if pixel_area > MAX_ANIMATED_PIXEL_AREA {
        return Err(format!(
            "ResourceLimitExceeded: área de {pixel_area} píxeles supera el máximo de {MAX_ANIMATED_PIXEL_AREA}"
        ));
    }
    if monitor_width > 0
        && monitor_height > 0
        && (width > monitor_width.saturating_mul(4) || height > monitor_height.saturating_mul(4))
    {
        return Err(format!(
            "ResourceLimitExceeded: resolución {width}x{height} es desproporcionada para el monitor {monitor_width}x{monitor_height}"
        ));
    }
    let estimated_decoded_frame_bytes = pixel_area
        .checked_mul(4)
        .ok_or_else(|| "ResourceLimitExceeded: overflow de memoria estimada".to_string())?;
    if estimated_decoded_frame_bytes > MAX_ESTIMATED_DECODED_FRAME_BYTES {
        return Err(format!(
            "ResourceLimitExceeded: frame RGBA estimado de {estimated_decoded_frame_bytes} bytes supera el máximo de {MAX_ESTIMATED_DECODED_FRAME_BYTES}"
        ));
    }

    let mut frame_count = 0u32;
    let mut duration_ms = 0u64;
    for frame in decoder.into_frames() {
        let frame =
            frame.map_err(|error| format!("MediaError: no se pudo decodificar el GIF: {error}"))?;
        frame_count = frame_count.saturating_add(1);
        if frame_count > MAX_ANIMATED_FRAMES {
            return Err(format!(
                "ResourceLimitExceeded: el GIF supera {MAX_ANIMATED_FRAMES} frames"
            ));
        }
        let (numerator, denominator) = frame.delay().numer_denom_ms();
        let frame_ms = u64::from(numerator).div_ceil(u64::from(denominator.max(1)));
        duration_ms = duration_ms.saturating_add(frame_ms);
        if duration_ms > MAX_ANIMATED_DURATION_MS {
            return Err(format!(
                "ResourceLimitExceeded: duración acumulada supera {MAX_ANIMATED_DURATION_MS} ms"
            ));
        }
    }
    if frame_count == 0 {
        return Err("MediaError: el GIF no contiene frames".into());
    }

    Ok(AnimatedResourceInfo {
        file_bytes,
        width,
        height,
        pixel_area,
        frame_count,
        duration_ms,
        estimated_decoded_frame_bytes,
    })
}

#[derive(Debug)]
struct PrototypeSurfaceRecord {
    lifecycle: AnimatedSurfaceLifecycle,
}

static PROTOTYPE_SURFACES: OnceLock<Mutex<BTreeMap<String, PrototypeSurfaceRecord>>> =
    OnceLock::new();

fn prototype_surfaces() -> &'static Mutex<BTreeMap<String, PrototypeSurfaceRecord>> {
    PROTOTYPE_SURFACES.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn register_prototype_surface(label: &str) -> Result<(), String> {
    let mut lifecycle = AnimatedSurfaceLifecycle::new();
    lifecycle.begin_create().map_err(str::to_string)?;
    lifecycle.webview_ready().map_err(str::to_string)?;
    lifecycle.playing().map_err(str::to_string)?;

    let mut surfaces = prototype_surfaces()
        .lock()
        .map_err(|_| "No se pudo bloquear el registro de superficies".to_string())?;
    if let Some(record) = surfaces.get(label) {
        if matches!(
            record.lifecycle.state(),
            AnimatedSurfaceState::Creating
                | AnimatedSurfaceState::WebViewReady
                | AnimatedSurfaceState::Playing
                | AnimatedSurfaceState::Closing
        ) {
            return Err("La superficie de diagnóstico ya está registrada".into());
        }
    }
    surfaces.insert(label.to_string(), PrototypeSurfaceRecord { lifecycle });
    Ok(())
}

fn begin_explicit_prototype_close(label: &str) {
    if let Ok(mut surfaces) = prototype_surfaces().lock() {
        if let Some(record) = surfaces.get_mut(label) {
            record.lifecycle.begin_close();
        }
    }
}

fn finish_prototype_close(label: &str) {
    if let Ok(mut surfaces) = prototype_surfaces().lock() {
        if let Some(record) = surfaces.get_mut(label) {
            record.lifecycle.finish_close();
        }
    }
}

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

pub fn prepare_animated_startup(app: &AppHandle) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let directory = prototype_directory(app)?;
        return native_window::prepare_startup(&directory);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        Ok(())
    }
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
    if extension == "gif" {
        let resource = validate_animated_gif(&source, bounds.width, bounds.height)?;
        eprintln!(
            "Animated resource accepted: {}x{} frames={} duration_ms={} estimated_decoded_frame_bytes={}",
            resource.width,
            resource.height,
            resource.frame_count,
            resource.duration_ms,
            resource.estimated_decoded_frame_bytes
        );
    }
    let label = format!("animated-prototype-{monitor_index}");

    if app.get_webview_window(&label).is_some() {
        return Err(
            "Ya existe una superficie de prototipo para ese monitor; ciérrala antes de abrir otra"
                .into(),
        );
    }

    let copied_media = copy_to_controlled_storage(&app, &source, extension)?;
    let media_query = encode_query_component(&copied_media.to_string_lossy());
    let error_test_query = if std::env::var_os("LUMINA_ANIMATED_MEDIA_ERROR_TEST").is_some() {
        "&mediaErrorTest=1"
    } else {
        ""
    };
    let url = WebviewUrl::App(
        format!(
            "index.html#/animated-prototype?media={media_query}&surface={label}{error_test_query}"
        )
        .into(),
    );
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

    if let Err(error) = register_prototype_surface(&label) {
        let _ = window.close();
        let _ = fs::remove_file(&cleanup_path);
        return Err(error);
    }

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
    let event_label = label.clone();
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) {
            let temporary_removed =
                fs::remove_file(&event_cleanup_path).is_ok() || !event_cleanup_path.exists();
            finish_prototype_close(&event_label);
            eprintln!("WEBVIEW2_CLOSED surface={event_label} source=tauri_destroyed");
            eprintln!("HWND_DESTROYED surface={event_label} source=tauri_destroyed");
            eprintln!("TEMPORARY_CLEANED surface={event_label} removed={temporary_removed}");
            eprintln!("ANIMATED_SURFACE_CLOSED surface={event_label}");
        }
    });
    if let Err(error) = window.show() {
        begin_explicit_prototype_close(&label);
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

#[cfg(target_os = "windows")]
pub fn open_animated_lifecycle_poc(app: AppHandle, monitor_index: usize) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or("No se encontró la ventana principal de Lumina")?;
    let monitor = window
        .available_monitors()
        .map_err(|error| format!("No se pudieron enumerar los monitores: {error}"))?
        .into_iter()
        .nth(monitor_index)
        .ok_or("No existe el monitor solicitado")?;
    let position = monitor.position();
    let size = monitor.size();
    let bounds = bounds_from_geometry(position.x, position.y, size.width, size.height)?;
    let dpi = (monitor.scale_factor() * 96.0).round() as u32;
    let (host, experiment) = desktop_host::prepare_modern_host()?;
    if !experiment.host_is_unambiguous {
        return Err(format!(
            "Host de escritorio no inequívoco: candidate_count={}",
            experiment.candidate_count
        ));
    }
    let monitor_id = "lifecycle-primary".to_string();
    native_window::open_physical_surface(&app, monitor_id.clone(), bounds, dpi, host)?;
    eprintln!(
        "[LifecyclePOC] HOST_VALIDATED monitor_id={} bounds=({},{} {}x{}) dpi={} candidate_count={}",
        monitor_id, bounds.x, bounds.y, bounds.width, bounds.height, dpi, experiment.candidate_count
    );
    eprintln!("[LifecyclePOC] SURFACE_ACTIVE");

    std::thread::spawn(move || {
        use recovery::{DesktopLifecycle, DesktopLifecycleAction, DesktopLifecycleEvent};
        use std::time::Duration;

        let mut lifecycle = DesktopLifecycle::new();
        std::thread::sleep(Duration::from_secs(2));
        if lifecycle.event(DesktopLifecycleEvent::Suspend)
            == DesktopLifecycleAction::SuspendSurfaces
        {
            eprintln!("[LifecyclePOC] POWER_SUSPEND_RECEIVED simulated=true");
            if let Err(error) = native_window::suspend_desired_surfaces_for_lifecycle() {
                eprintln!("[LifecyclePOC] SUSPEND_FAILED error={error}");
                let _ = native_window::close_physical_surface(&monitor_id);
                lifecycle.finish_close();
                return;
            }
            eprintln!("[LifecyclePOC] STATE=Suspended");
        }

        std::thread::sleep(Duration::from_secs(1));
        if lifecycle.event(DesktopLifecycleEvent::Resume) == DesktopLifecycleAction::RevalidateHost
        {
            eprintln!("[LifecyclePOC] POWER_RESUME_RECEIVED simulated=true");
            let host_valid = matches!(
                native_window::validate_registered_desktop_host(),
                Ok(desktop_host::HostValidation::Stable)
            );
            eprintln!("[LifecyclePOC] HOST_REVALIDATION_RESULT valid={host_valid}");
            if !host_valid {
                let _ = lifecycle.event(DesktopLifecycleEvent::HostInvalid);
            }
            if lifecycle.host_revalidated(host_valid) == DesktopLifecycleAction::RevalidateHost
                && host_valid
            {
                eprintln!("[LifecyclePOC] RECOVERY_STARTED reason=resume");
                let recovered = native_window::recover_desired_surfaces_for_lifecycle();
                lifecycle.recovery_finished(recovered);
                if recovered {
                    eprintln!("[LifecyclePOC] SURFACE_RECREATED reason=resume");
                } else {
                    eprintln!("[LifecyclePOC] RECOVERY_SKIPPED reason=host_unavailable");
                }
            }
        }

        std::thread::sleep(Duration::from_secs(1));
        if lifecycle.event(DesktopLifecycleEvent::SessionLock)
            == DesktopLifecycleAction::SuspendSurfaces
        {
            eprintln!("[LifecyclePOC] SESSION_LOCK_RECEIVED simulated=true");
            let _ = native_window::suspend_desired_surfaces_for_lifecycle();
            eprintln!("[LifecyclePOC] STATE=SessionLocked");
        }
        if lifecycle.event(DesktopLifecycleEvent::SessionUnlock)
            == DesktopLifecycleAction::RevalidateHost
        {
            eprintln!("[LifecyclePOC] SESSION_UNLOCK_RECEIVED simulated=true");
            let host_valid = matches!(
                native_window::validate_registered_desktop_host(),
                Ok(desktop_host::HostValidation::Stable)
            );
            eprintln!("[LifecyclePOC] HOST_REVALIDATION_RESULT valid={host_valid}");
            if !host_valid {
                let _ = lifecycle.event(DesktopLifecycleEvent::HostInvalid);
            }
            if lifecycle.host_revalidated(host_valid) == DesktopLifecycleAction::RevalidateHost
                && host_valid
            {
                eprintln!("[LifecyclePOC] RECOVERY_STARTED reason=unlock");
                let recovered = native_window::recover_desired_surfaces_for_lifecycle();
                lifecycle.recovery_finished(recovered);
                if recovered {
                    eprintln!("[LifecyclePOC] SURFACE_RECREATED reason=unlock");
                } else {
                    eprintln!("[LifecyclePOC] RECOVERY_SKIPPED reason=host_unavailable");
                }
            }
        }

        let _ = native_window::close_physical_surface(&monitor_id);
        lifecycle.event(DesktopLifecycleEvent::Close);
        lifecycle.finish_close();
        eprintln!(
            "[LifecyclePOC] LIFECYCLE_CLEANUP_COMPLETE state={:?}",
            lifecycle.state()
        );
    });
    Ok(())
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
pub fn open_animated_host_watcher_poc(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let window = app
            .get_webview_window("main")
            .ok_or("No se encontró la ventana principal de Lumina")?;
        let monitor = window
            .primary_monitor()
            .map_err(|error| format!("No se pudo obtener el monitor principal: {error}"))?
            .ok_or("Windows no expuso un monitor principal")?;
        let position = monitor.position();
        let size = monitor.size();
        let bounds = bounds_from_geometry(position.x, position.y, size.width, size.height)?;
        let dpi = (monitor.scale_factor() * 96.0).round() as u32;
        let temporary_directory = prototype_directory(&app)?;
        native_window::cleanup_orphaned_multi_temporaries(&temporary_directory)?;
        let (host, experiment) = desktop_host::prepare_modern_host()?;
        if !experiment.host_is_unambiguous {
            return Err(format!(
                "Host de escritorio no inequívoco: candidate_count={}",
                experiment.candidate_count
            ));
        }
        eprintln!(
            "[HostMonitorPOC] MONITOR_READY id=watcher-primary bounds=({},{} {}x{}) dpi={} candidate_count={}",
            bounds.x, bounds.y, bounds.width, bounds.height, dpi, experiment.candidate_count
        );
        native_window::open_physical_surface(
            &app,
            "watcher-primary".to_string(),
            bounds,
            dpi,
            host,
        )?;
        if std::env::var_os("LUMINA_ANIMATED_HOST_WATCHER_AUTOCLOSE").is_some() {
            std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_secs(5));
                eprintln!("[HostMonitorPOC] EXPLICIT_CLOSE_COMMAND");
                if let Err(error) = native_window::close_physical_surface("watcher-primary") {
                    eprintln!("[HostMonitorPOC] EXPLICIT_CLOSE_FAILED error={error}");
                }
            });
        }
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        Err("El POC del watcher solo está disponible en Windows".into())
    }
}

#[tauri::command]
pub fn open_animated_multimonitor_physical_poc(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let temporary_directory = prototype_directory(&app)?;
        native_window::cleanup_orphaned_multi_temporaries(&temporary_directory)?;
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
            if let Err(error) =
                native_window::open_physical_surface(&app, id.clone(), bounds, dpi, host)
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
        begin_explicit_prototype_close(&label);
        window
            .close()
            .map_err(|e| format!("No se pudo cerrar la superficie WebView2: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
pub fn report_animated_media_error(
    app: AppHandle,
    surface_id: String,
    kind: String,
    reason: Option<String>,
) -> Result<(), String> {
    if !surface_id.starts_with("animated-prototype-") {
        return Err("Identificador de superficie no permitido".into());
    }

    let failure_kind = match kind.as_str() {
        "media" => SurfaceFailureKind::Media,
        "webview" => SurfaceFailureKind::WebView,
        _ => return Err("Tipo de error de superficie no permitido".into()),
    };

    let decision = {
        let mut surfaces = prototype_surfaces()
            .lock()
            .map_err(|_| "No se pudo bloquear el registro de superficies".to_string())?;
        let Some(record) = surfaces.get_mut(&surface_id) else {
            eprintln!("ANIMATED_MEDIA_ERROR_IGNORED surface={surface_id} reason=unknown_surface");
            return Ok(());
        };
        if matches!(
            record.lifecycle.state(),
            AnimatedSurfaceState::Closing | AnimatedSurfaceState::Closed
        ) {
            eprintln!(
                "ANIMATED_MEDIA_ERROR_IGNORED surface={surface_id} reason=surface_closing_or_closed"
            );
            return Ok(());
        }
        let decision = record.lifecycle.fail(failure_kind);
        record.lifecycle.begin_close();
        eprintln!(
            "ANIMATED_MEDIA_ERROR_RECEIVED surface={surface_id} kind={kind} failures={} reason={}",
            record.lifecycle.failures(),
            reason.as_deref().unwrap_or("unspecified")
        );
        decision
    };

    if let Some(window) = app.get_webview_window(&surface_id) {
        if let Err(error) = window.close() {
            eprintln!("ANIMATED_MEDIA_ERROR_CLOSE_FAILED surface={surface_id} error={error}");
            finish_prototype_close(&surface_id);
        }
    } else {
        finish_prototype_close(&surface_id);
    }

    if decision == FailureDecision::FallbackRequired {
        eprintln!("ANIMATED_SURFACE_FALLBACK_REQUIRED surface={surface_id}");
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
            begin_explicit_prototype_close(&label);
            let _ = window.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        bounds_from_geometry, create_diagnostic_gif, encode_query_component, resource_limit_reason,
        smoke_variant_options, validate_animated_gif, validate_media_source,
        MAX_ANIMATED_DIMENSION, MAX_ANIMATED_FILE_BYTES,
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
    fn animated_resource_limits_reject_oversized_dimensions_area_memory_and_file() {
        assert_eq!(
            resource_limit_reason(MAX_ANIMATED_FILE_BYTES + 1, 320, 180, 2560, 1440),
            Some("ResourceLimitExceeded:file_size")
        );
        assert_eq!(
            resource_limit_reason(1, MAX_ANIMATED_DIMENSION + 1, 1, 2560, 1440),
            Some("ResourceLimitExceeded:dimensions")
        );
        assert_eq!(
            resource_limit_reason(1, 6000, 6000, 8192, 8192),
            Some("ResourceLimitExceeded:pixel_area")
        );
        assert_eq!(
            resource_limit_reason(1, 5000, 4000, 8192, 8192),
            Some("ResourceLimitExceeded:decoded_frame")
        );
        assert_eq!(resource_limit_reason(1, 320, 180, 2560, 1440), None);
    }

    #[test]
    fn small_diagnostic_gif_is_accepted_with_measured_metadata() {
        let path = std::env::temp_dir().join(format!("lumina-resource-{}.gif", Uuid::new_v4()));
        create_diagnostic_gif(&path).unwrap();
        let info = validate_animated_gif(&path, 2560, 1440).unwrap();
        assert_eq!((info.width, info.height), (320, 180));
        assert_eq!(info.frame_count, 2);
        assert!(info.duration_ms > 0);
        assert_eq!(info.estimated_decoded_frame_bytes, 320 * 180 * 4);
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
