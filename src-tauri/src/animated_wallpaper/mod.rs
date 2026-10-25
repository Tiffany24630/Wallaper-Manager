use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
    sync::{Condvar, Mutex, OnceLock},
    thread,
    time::Duration,
};

use crate::models::monitor_assignment::WallpaperBackend;
use crate::models::monitor_assignment::{
    resolve_monitor_identities, MonitorGeometry, MonitorIdentity, MonitorResolution,
    UiMonitorDescriptor,
};
use crate::models::wallpaper_coordinator::{self, AssignmentEvidence, AssignmentOperation};
use image::{
    codecs::gif::{GifDecoder, GifEncoder, Repeat},
    AnimationDecoder, Delay, Frame, ImageDecoder, Rgba, RgbaImage,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};
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
pub(crate) enum AnimatedMediaFormat {
    Gif,
}

impl AnimatedMediaFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Gif => "gif",
        }
    }
}

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
    activation_id: String,
    assignment: AssignmentOperation,
    wallpaper_id: Option<String>,
    destroyed_confirmed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AnimatedActivationOutcome {
    Pending,
    Ready,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone)]
pub(crate) struct AnimatedActivationHandle {
    pub(crate) monitor_id: MonitorIdentity,
    pub(crate) generation: u64,
    pub(crate) activation_id: String,
}

static ACTIVATION_OUTCOMES: OnceLock<(Mutex<HashMap<String, AnimatedActivationOutcome>>, Condvar)> =
    OnceLock::new();

fn activation_outcomes() -> &'static (Mutex<HashMap<String, AnimatedActivationOutcome>>, Condvar) {
    ACTIVATION_OUTCOMES.get_or_init(|| (Mutex::new(HashMap::new()), Condvar::new()))
}

fn register_activation(activation_id: &str) {
    activation_outcomes()
        .0
        .lock()
        .expect("activation outcomes mutex poisoned")
        .insert(
            activation_id.to_string(),
            AnimatedActivationOutcome::Pending,
        );
}

fn signal_activation(activation_id: &str, outcome: AnimatedActivationOutcome) {
    if let Ok(mut outcomes) = activation_outcomes().0.lock() {
        if outcomes.get(activation_id) == Some(&AnimatedActivationOutcome::Pending) {
            outcomes.insert(activation_id.to_string(), outcome);
            activation_outcomes().1.notify_all();
        }
    }
}

pub(crate) fn wait_for_activation(
    handle: &AnimatedActivationHandle,
    timeout: Duration,
) -> AnimatedActivationOutcome {
    let deadline = std::time::Instant::now() + timeout;
    let (lock, condition) = activation_outcomes();
    let mut outcomes = lock.lock().expect("activation outcomes mutex poisoned");
    loop {
        let outcome = outcomes
            .get(&handle.activation_id)
            .copied()
            .unwrap_or(AnimatedActivationOutcome::Cancelled);
        if outcome != AnimatedActivationOutcome::Pending {
            outcomes.remove(&handle.activation_id);
            return outcome;
        }
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            outcomes.remove(&handle.activation_id);
            return AnimatedActivationOutcome::Failed;
        }
        let (next, timed_out) = condition
            .wait_timeout(outcomes, remaining)
            .expect("activation outcomes condition variable poisoned");
        outcomes = next;
        if timed_out.timed_out() {
            outcomes.remove(&handle.activation_id);
            return AnimatedActivationOutcome::Failed;
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnimatedActivationEvent {
    state: &'static str,
    monitor_id: String,
    generation: u64,
    activation_id: String,
    wallpaper_id: Option<String>,
    error: Option<String>,
}

fn emit_activation_event(app: &AppHandle, event: AnimatedActivationEvent) {
    if let Err(error) = app.emit("animated-activation", event) {
        eprintln!("ANIMATED_EVENT_EMIT_FAILED error={error}");
    }
}

fn activation_matches(record: &PrototypeSurfaceRecord, activation_id: &str) -> bool {
    record.activation_id == activation_id
}

fn diagnostic_surface_conflicts(has_current_assignment: bool, has_pending_operation: bool) -> bool {
    has_current_assignment || has_pending_operation
}

fn ensure_diagnostic_surface_is_isolated(monitor_id: &MonitorIdentity) -> Result<(), String> {
    let coordinator = wallpaper_coordinator::global()
        .lock()
        .map_err(|_| "No se pudo bloquear el coordinador de wallpapers".to_string())?;
    if !matches!(
        coordinator.lifecycle(),
        wallpaper_coordinator::CoordinatorLifecycle::Ready
    ) {
        return Err(
            "El POC diagnóstico no puede crear una superficie fuera del lifecycle Ready".into(),
        );
    }
    if diagnostic_surface_conflicts(
        coordinator.current(monitor_id).is_some(),
        coordinator.pending(monitor_id).is_some(),
    ) {
        return Err(format!(
            "El POC diagnóstico no puede crear una superficie sobre una asignación coordinada para {monitor_id}"
        ));
    }
    Ok(())
}

fn prototype_surface_label(monitor_id: &MonitorIdentity) -> String {
    let encoded = monitor_id
        .as_str()
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("animated-prototype-{encoded}")
}

static PROTOTYPE_SURFACES: OnceLock<Mutex<BTreeMap<String, PrototypeSurfaceRecord>>> =
    OnceLock::new();

fn prototype_surfaces() -> &'static Mutex<BTreeMap<String, PrototypeSurfaceRecord>> {
    PROTOTYPE_SURFACES.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn register_prototype_surface(
    label: &str,
    activation_id: &str,
    assignment: AssignmentOperation,
    wallpaper_id: Option<String>,
) -> Result<(), String> {
    let mut lifecycle = AnimatedSurfaceLifecycle::new();
    lifecycle.begin_create().map_err(str::to_string)?;
    lifecycle.webview_ready().map_err(str::to_string)?;

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
    surfaces.insert(
        label.to_string(),
        PrototypeSurfaceRecord {
            lifecycle,
            activation_id: activation_id.to_string(),
            assignment,
            wallpaper_id,
            destroyed_confirmed: false,
        },
    );
    register_activation(activation_id);
    Ok(())
}

fn begin_explicit_prototype_close(label: &str) {
    if let Ok(mut surfaces) = prototype_surfaces().lock() {
        if let Some(record) = surfaces.get_mut(label) {
            record.lifecycle.begin_close();
        }
    }
}

fn take_destroyed_surface(
    surfaces: &mut BTreeMap<String, PrototypeSurfaceRecord>,
    label: &str,
    activation_id: Option<&str>,
) -> Option<AssignmentOperation> {
    let Some(mut record) = surfaces.remove(label) else {
        return None;
    };
    if activation_id.is_some_and(|id| id != record.activation_id) {
        // A stale Destroyed event must not consume the record belonging to a
        // newer activation that reused the same logical surface label.
        surfaces.insert(label.to_string(), record);
        eprintln!("ANIMATED_SURFACE_CLOSE_IGNORED surface={label} reason=stale_activation");
        return None;
    }
    if record.destroyed_confirmed {
        surfaces.insert(label.to_string(), record);
        eprintln!(
            "ANIMATED_DESTROYED_IGNORED surface={label} activation={} reason=duplicate_confirmation",
            surfaces.get(label).map(|value| value.activation_id.as_str()).unwrap_or("unknown")
        );
        return None;
    }
    record.destroyed_confirmed = true;
    record.lifecycle.finish_close();
    let assignment = record.assignment.clone();
    signal_activation(&record.activation_id, AnimatedActivationOutcome::Cancelled);
    eprintln!(
        "ANIMATED_DESTROYED_CONFIRMED surface={label} activation={} monitor={} generation={}",
        record.activation_id,
        assignment.assignment.monitor,
        assignment.assignment.generation.value()
    );
    Some(assignment)
}

fn finish_prototype_close(label: &str, activation_id: Option<&str>) {
    let assignment = if let Ok(mut surfaces) = prototype_surfaces().lock() {
        take_destroyed_surface(&mut surfaces, label, activation_id)
    } else {
        None
    };
    if let Some(assignment) = assignment {
        if let Ok(mut coordinator) = wallpaper_coordinator::global().lock() {
            let result = if coordinator
                .pending(assignment.assignment.monitor_ref())
                .is_some()
            {
                coordinator.abort(&assignment)
            } else {
                coordinator.release(&assignment)
            };
            eprintln!(
                "ANIMATED_COORDINATOR_RELEASE generation={} result={result:?}",
                assignment.assignment.generation.value()
            );
        } else {
            eprintln!(
                "ANIMATED_COORDINATOR_RELEASE generation={} result=CoordinatorLockFailed",
                assignment.assignment.generation.value()
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MonitorBounds {
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
    let bounds = MonitorBounds {
        x,
        y,
        width,
        height,
    };
    multimonitor::LogicalBounds {
        x: bounds.x,
        y: bounds.y,
        width: bounds.width,
        height: bounds.height,
    }
    .validate()
    .map_err(|error| format!("Geometría de monitor inválida: {error}"))?;
    Ok(bounds)
}

#[cfg(target_os = "windows")]
pub(crate) fn resolve_monitor_for_identity(
    app: &AppHandle,
    monitor_id: &MonitorIdentity,
) -> Result<(MonitorBounds, u32), String> {
    let window = app
        .get_webview_window("main")
        .ok_or("No se encontrÃ³ la ventana principal de Lumina")?;
    let monitors = window
        .available_monitors()
        .map_err(|error| format!("No se pudieron enumerar los monitores: {error}"))?;
    let ui = monitors
        .iter()
        .map(|monitor| {
            let position = monitor.position();
            let size = monitor.size();
            Ok(UiMonitorDescriptor {
                name: monitor.name().cloned(),
                geometry: MonitorGeometry::new(position.x, position.y, size.width, size.height)
                    .map_err(str::to_string)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let native = crate::utils::windows_wallpaper::native_monitor_descriptors()
        .map_err(|error| error.to_string())?;
    let resolutions = resolve_monitor_identities(&ui, &native).map_err(str::to_string)?;
    let Some((monitor, resolution)) = monitors.iter().zip(resolutions).find(|(_, result)| {
        matches!(result, MonitorResolution::Resolved(identity) if identity == monitor_id)
    }) else {
        if native.iter().any(|descriptor| descriptor.device_path == monitor_id.as_str()) {
            return Err("La identidad del monitor existe, pero no pudo correlacionarse inequÃ­vocamente con Tauri".into());
        }
        return Err("El monitor solicitado ya no estÃ¡ disponible".into());
    };
    let position = monitor.position();
    let size = monitor.size();
    let bounds = bounds_from_geometry(position.x, position.y, size.width, size.height)?;
    let dpi = (monitor.scale_factor() * 96.0).round() as u32;
    if dpi == 0 {
        return Err("El monitor no tiene un DPI vÃ¡lido".into());
    }
    let _ = resolution;
    Ok((bounds, dpi))
}

#[cfg(target_os = "windows")]
fn monitor_position_for_identity(
    monitors: &[tauri::Monitor],
    monitor_id: &MonitorIdentity,
) -> Result<usize, String> {
    let ui = monitors
        .iter()
        .map(|monitor| {
            let position = monitor.position();
            let size = monitor.size();
            Ok(UiMonitorDescriptor {
                name: monitor.name().cloned(),
                geometry: MonitorGeometry::new(position.x, position.y, size.width, size.height)
                    .map_err(str::to_string)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let native = crate::utils::windows_wallpaper::native_monitor_descriptors()
        .map_err(|error| error.to_string())?;
    let resolutions = resolve_monitor_identities(&ui, &native).map_err(str::to_string)?;
    resolutions
        .iter()
        .position(|resolution| matches!(resolution, MonitorResolution::Resolved(identity) if identity == monitor_id))
        .ok_or_else(|| "El monitor solicitado no estÃ¡ disponible o es ambiguo".into())
}

#[cfg(target_os = "windows")]
fn resolved_monitor_identities(
    monitors: &[tauri::Monitor],
) -> Result<Vec<MonitorIdentity>, String> {
    let ui = monitors
        .iter()
        .map(|monitor| {
            let position = monitor.position();
            let size = monitor.size();
            Ok(UiMonitorDescriptor {
                name: monitor.name().cloned(),
                geometry: MonitorGeometry::new(position.x, position.y, size.width, size.height)
                    .map_err(str::to_string)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let native = crate::utils::windows_wallpaper::native_monitor_descriptors()
        .map_err(|error| error.to_string())?;
    resolve_monitor_identities(&ui, &native)
        .map_err(str::to_string)?
        .into_iter()
        .enumerate()
        .map(|(index, resolution)| match resolution {
            MonitorResolution::Resolved(identity) => Ok(identity),
            MonitorResolution::Ambiguous => {
                Err(format!("La identidad del monitor {index} es ambigua"))
            }
            MonitorResolution::Unavailable => Err(format!(
                "La identidad nativa del monitor {index} no estÃ¡ disponible"
            )),
        })
        .collect()
}

#[cfg(target_os = "windows")]
pub fn primary_monitor_identity(app: &AppHandle) -> Result<String, String> {
    let window = app
        .get_webview_window("main")
        .ok_or("No se encontrÃ³ la ventana principal de Lumina")?;
    let primary = window
        .primary_monitor()
        .map_err(|error| format!("No se pudo obtener el monitor principal: {error}"))?
        .ok_or("Windows no expuso un monitor principal")?;
    let position = primary.position();
    let size = primary.size();
    let geometry = MonitorGeometry::new(position.x, position.y, size.width, size.height)
        .map_err(str::to_string)?;
    let monitors = window
        .available_monitors()
        .map_err(|error| format!("No se pudieron enumerar los monitores: {error}"))?;
    let ui = monitors
        .iter()
        .map(|monitor| {
            let position = monitor.position();
            let size = monitor.size();
            Ok(UiMonitorDescriptor {
                name: monitor.name().cloned(),
                geometry: MonitorGeometry::new(position.x, position.y, size.width, size.height)
                    .map_err(str::to_string)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let native = crate::utils::windows_wallpaper::native_monitor_descriptors()
        .map_err(|error| error.to_string())?;
    let resolutions = resolve_monitor_identities(&ui, &native).map_err(str::to_string)?;
    let index = ui
        .iter()
        .position(|entry| entry.geometry == geometry)
        .ok_or("El monitor principal no aparece en la enumeraciÃ³n actual")?;
    match &resolutions[index] {
        MonitorResolution::Resolved(identity) => Ok(identity.as_str().to_string()),
        MonitorResolution::Ambiguous => Err("La identidad del monitor principal es ambigua".into()),
        MonitorResolution::Unavailable => {
            Err("La identidad del monitor principal no estÃ¡ disponible".into())
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn primary_monitor_identity(_app: &AppHandle) -> Result<String, String> {
    Err("La identidad nativa de monitores solo estÃ¡ disponible en Windows".into())
}

fn validate_media_source(path: &Path) -> Result<AnimatedMediaFormat, String> {
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
        Some("mp4") => Err(
            "ResourceFormatUnsupported: MP4 todavía no está admitido por el prototipo animado"
                .into(),
        ),
        Some("gif") => Ok(AnimatedMediaFormat::Gif),
        _ => Err("ResourceFormatUnsupported: el prototipo solo admite archivos GIF locales".into()),
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
    let _ = app;
    let directory = crate::db::app_data_dir()
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
    monitor_id: String,
    media_path: String,
    diagnostic: bool,
) -> Result<(), String> {
    open_animated_surface(app, monitor_id, media_path, diagnostic, None)
        .await
        .map(|_| ())
}

pub async fn open_library_animated(
    app: AppHandle,
    monitor_id: String,
    wallpaper_id: String,
    media_path: String,
) -> Result<AnimatedActivationHandle, String> {
    open_animated_surface(app, monitor_id, media_path, false, Some(wallpaper_id)).await
}

async fn open_animated_surface(
    app: AppHandle,
    monitor_id: String,
    media_path: String,
    diagnostic: bool,
    wallpaper_id: Option<String>,
) -> Result<AnimatedActivationHandle, String> {
    crate::db::ensure_physical_operations_allowed()?;
    let monitor_identity = MonitorIdentity::new(monitor_id.clone()).map_err(str::to_string)?;
    let source = PathBuf::from(media_path);
    let format = validate_media_source(&source)?;
    let monitors = app
        .get_webview_window("main")
        .ok_or("No se encontró la ventana principal de Lumina")?
        .available_monitors()
        .map_err(|e| format!("No se pudieron enumerar los monitores: {e}"))?;
    let monitor = monitors
        .get(monitor_position_for_identity(&monitors, &monitor_identity)?)
        .ok_or("El monitor seleccionado ya no está disponible")?;
    let position = monitor.position();
    let size = monitor.size();
    let bounds = bounds_from_geometry(position.x, position.y, size.width, size.height)?;
    let resource = validate_animated_gif(&source, bounds.width, bounds.height)?;
    eprintln!(
        "Animated resource accepted: {}x{} frames={} duration_ms={} estimated_decoded_frame_bytes={}",
        resource.width,
        resource.height,
        resource.frame_count,
        resource.duration_ms,
        resource.estimated_decoded_frame_bytes
    );
    let label = prototype_surface_label(&monitor_identity);
    let activation_id = Uuid::new_v4().to_string();

    if let Some(existing) = app.get_webview_window(&label) {
        begin_explicit_prototype_close(&label);
        existing
            .close()
            .map_err(|error| format!("No se pudo iniciar el reemplazo de Animated: {error}"))?;
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            let surface_gone = prototype_surfaces()
                .lock()
                .map(|surfaces| !surfaces.contains_key(&label))
                .unwrap_or(false);
            if app.get_webview_window(&label).is_none() && surface_gone {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
        if app.get_webview_window(&label).is_some()
            || prototype_surfaces()
                .lock()
                .map(|surfaces| surfaces.contains_key(&label))
                .unwrap_or(true)
        {
            return Err("La superficie Animated anterior no confirmó su cierre".into());
        }
    }

    if app.get_webview_window(&label).is_some() {
        return Err(
            "Ya existe una superficie de prototipo para ese monitor; ciérrala antes de abrir otra"
                .into(),
        );
    }

    let copied_media = copy_to_controlled_storage(&app, &source, format.extension())?;
    let media_query = encode_query_component(&copied_media.to_string_lossy());
    let error_test_query = if std::env::var_os("LUMINA_ANIMATED_MEDIA_ERROR_TEST").is_some() {
        "&mediaErrorTest=1"
    } else {
        ""
    };
    let route = if wallpaper_id.is_some() {
        "animated-surface"
    } else {
        "animated-prototype"
    };
    let url = WebviewUrl::App(
        format!(
            "index.html#/{route}?media={media_query}&surface={label}&activation={activation_id}{error_test_query}"
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
    let assignment = wallpaper_coordinator::global()
        .lock()
        .map_err(|_| "No se pudo bloquear el coordinador de wallpapers".to_string())?
        .reserve(monitor_identity.clone(), WallpaperBackend::Animated)
        .map_err(|error| error.to_string())?;
    let mut builder = WebviewWindowBuilder::new(&app, &label, url)
        .title(if wallpaper_id.is_some() {
            "Lumina Animated"
        } else {
            "Lumina animated wallpaper prototype"
        })
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
    let window = match builder.build() {
        Ok(window) => window,
        Err(error) => {
            if let Ok(mut coordinator) = wallpaper_coordinator::global().lock() {
                coordinator.abort(&assignment);
            }
            let _ = fs::remove_file(&cleanup_path);
            return Err(format!("No se pudo crear la superficie WebView2: {error}"));
        }
    };

    let event_cleanup_path = cleanup_path.clone();
    let event_label = label.clone();
    let event_activation_id = activation_id.clone();
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) {
            let temporary_removed =
                fs::remove_file(&event_cleanup_path).is_ok() || !event_cleanup_path.exists();
            finish_prototype_close(&event_label, Some(&event_activation_id));
            eprintln!("WEBVIEW2_CLOSED surface={event_label} source=tauri_destroyed");
            eprintln!("HWND_DESTROYED surface={event_label} source=tauri_destroyed");
            if temporary_removed {
                eprintln!("TEMPORARY_CLEANED surface={event_label} removed=true");
            } else {
                eprintln!(
                    "TEMPORARY_CLEANUP_PENDING surface={event_label} removed=false reason=destroyed_confirmed_but_file_unavailable"
                );
            }
            eprintln!("ANIMATED_SURFACE_CLOSED surface={event_label}");
        }
    });

    if let Err(error) =
        register_prototype_surface(&label, &activation_id, assignment.clone(), wallpaper_id)
    {
        if let Ok(mut coordinator) = wallpaper_coordinator::global().lock() {
            coordinator.abort(&assignment);
        }
        if let Err(close_error) = window.close() {
            eprintln!("ANIMATED_SURFACE_CLOSE_REQUEST_FAILED surface={label} error={close_error}");
        }
        return Err(error);
    }

    if workerw_mode {
        #[cfg(target_os = "windows")]
        if let Err(error) = desktop_host::attach_to_worker_w(&window, bounds) {
            if let Ok(mut coordinator) = wallpaper_coordinator::global().lock() {
                coordinator.abort(&assignment);
            }
            if let Err(close_error) = window.close() {
                eprintln!(
                    "ANIMATED_SURFACE_CLOSE_REQUEST_FAILED surface={label} error={close_error}"
                );
            }
            return Err(format!(
                "No se pudo activar el host WorkerW experimental: {error}"
            ));
        }
        #[cfg(not(target_os = "windows"))]
        {
            if let Ok(mut coordinator) = wallpaper_coordinator::global().lock() {
                coordinator.abort(&assignment);
            }
            if let Err(close_error) = window.close() {
                eprintln!(
                    "ANIMATED_SURFACE_CLOSE_REQUEST_FAILED surface={label} error={close_error}"
                );
            }
            return Err("El host WorkerW experimental solo está disponible en Windows".into());
        }
    }

    if let Err(error) = window.set_ignore_cursor_events(options.ignore_cursor_events) {
        if let Ok(mut coordinator) = wallpaper_coordinator::global().lock() {
            coordinator.abort(&assignment);
        }
        if let Err(close_error) = window.close() {
            eprintln!("ANIMATED_SURFACE_CLOSE_REQUEST_FAILED surface={label} error={close_error}");
        }
        return Err(format!(
            "No se pudo configurar la interacción del prototipo: {error}"
        ));
    }
    if let Err(error) = window.show() {
        begin_explicit_prototype_close(&label);
        if let Ok(mut coordinator) = wallpaper_coordinator::global().lock() {
            coordinator.abort(&assignment);
        }
        if let Err(close_error) = window.close() {
            eprintln!("ANIMATED_SURFACE_CLOSE_REQUEST_FAILED surface={label} error={close_error}");
        }
        return Err(format!(
            "No se pudo mostrar la superficie WebView2: {error}"
        ));
    }
    eprintln!("Animated prototype window shown; awaiting media-ready confirmation: {label}");
    Ok(AnimatedActivationHandle {
        monitor_id: assignment.assignment.monitor.clone(),
        generation: assignment.assignment.generation.value(),
        activation_id,
    })
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
    monitor_id: String,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let monitor_identity = MonitorIdentity::new(monitor_id.clone()).map_err(str::to_string)?;
        ensure_diagnostic_surface_is_isolated(&monitor_identity)?;
        let monitors = app
            .get_webview_window("main")
            .ok_or("No se encontró la ventana principal de Lumina")?
            .available_monitors()
            .map_err(|error| format!("No se pudieron enumerar los monitores: {error}"))?;
        let monitor = monitors
            .get(monitor_position_for_identity(&monitors, &monitor_identity)?)
            .ok_or("El monitor seleccionado ya no está disponible")?;
        let position = monitor.position();
        let size = monitor.size();
        let bounds = bounds_from_geometry(position.x, position.y, size.width, size.height)?;
        return native_window::open(&app, bounds);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, monitor_id);
        Err("El renderer HWND nativo solo está disponible en Windows".into())
    }
}

#[tauri::command]
pub fn open_animated_desktop_poc(app: AppHandle, monitor_id: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let monitor_identity = MonitorIdentity::new(monitor_id.clone()).map_err(str::to_string)?;
        ensure_diagnostic_surface_is_isolated(&monitor_identity)?;
        let monitors = app
            .get_webview_window("main")
            .ok_or("No se encontró la ventana principal de Lumina")?
            .available_monitors()
            .map_err(|error| format!("No se pudieron enumerar los monitores: {error}"))?;
        let monitor = monitors
            .get(monitor_position_for_identity(&monitors, &monitor_identity)?)
            .ok_or("El monitor seleccionado ya no está disponible")?;
        let position = monitor.position();
        let size = monitor.size();
        let bounds = bounds_from_geometry(position.x, position.y, size.width, size.height)?;
        return native_window::open_desktop_poc(&app, bounds);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, monitor_id);
        Err("El POC de escritorio solo está disponible en Windows".into())
    }
}

#[tauri::command]
pub fn open_animated_desktop_recovery_poc(
    app: AppHandle,
    monitor_id: String,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let monitor_identity = MonitorIdentity::new(monitor_id.clone()).map_err(str::to_string)?;
        ensure_diagnostic_surface_is_isolated(&monitor_identity)?;
        let monitors = app
            .get_webview_window("main")
            .ok_or("No se encontró la ventana principal de Lumina")?
            .available_monitors()
            .map_err(|error| format!("No se pudieron enumerar los monitores: {error}"))?;
        let monitor = monitors
            .get(monitor_position_for_identity(&monitors, &monitor_identity)?)
            .ok_or("El monitor seleccionado ya no está disponible")?;
        let position = monitor.position();
        let size = monitor.size();
        let bounds = bounds_from_geometry(position.x, position.y, size.width, size.height)?;
        return native_window::open_desktop_recovery_poc(&app, bounds);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, monitor_id);
        Err("El POC de recuperación solo está disponible en Windows".into())
    }
}

#[cfg(target_os = "windows")]
pub fn open_animated_lifecycle_poc(app: AppHandle, monitor_id: String) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or("No se encontró la ventana principal de Lumina")?;
    let monitor_identity = MonitorIdentity::new(monitor_id).map_err(str::to_string)?;
    let monitors = window
        .available_monitors()
        .map_err(|error| format!("No se pudieron enumerar los monitores: {error}"))?;
    let monitor = monitors
        .get(monitor_position_for_identity(&monitors, &monitor_identity)?)
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
    let monitor_id = monitor_identity;
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
        let resolved_ids = resolved_monitor_identities(&monitors)?;
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
            let id = resolved_ids
                .get(index)
                .cloned()
                .ok_or("La identidad nativa del monitor no estÃ¡ disponible")?;
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
        native_window::open_physical_surface(&app, "watcher-primary".into(), bounds, dpi, host)?;
        if std::env::var_os("LUMINA_ANIMATED_HOST_WATCHER_AUTOCLOSE").is_some() {
            std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_secs(5));
                eprintln!("[HostMonitorPOC] EXPLICIT_CLOSE_COMMAND");
                let monitor_id: MonitorIdentity = "watcher-primary".into();
                if let Err(error) = native_window::close_physical_surface(&monitor_id) {
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

        let resolved_ids = resolved_monitor_identities(&monitors)?;
        let mut created: Vec<MonitorIdentity> = Vec::new();
        for (index, monitor) in monitors.iter().take(2).enumerate() {
            let position = monitor.position();
            let size = monitor.size();
            let id = resolved_ids
                .get(index)
                .cloned()
                .ok_or("La identidad nativa del monitor no estÃ¡ disponible")?;
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
        let monitor_id = MonitorIdentity::new(monitor_id).map_err(str::to_string)?;
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
    monitor_id: String,
) -> Result<(), String> {
    let path = prototype_directory(&app)?.join(format!("diagnostic-{}.gif", Uuid::new_v4()));
    create_diagnostic_gif(&path)?;
    let diagnostic = std::env::var_os("LUMINA_ANIMATED_SMOKE_WALLPAPER").is_none();
    let result = open_animated_prototype(
        app,
        monitor_id,
        path.to_string_lossy().into_owned(),
        diagnostic,
    )
    .await;
    let _ = fs::remove_file(path);
    result
}

#[tauri::command]
pub fn close_animated_prototype(app: AppHandle, monitor_id: String) -> Result<(), String> {
    let monitor_identity = MonitorIdentity::new(monitor_id).map_err(str::to_string)?;
    let label = prototype_surface_label(&monitor_identity);
    if let Some(window) = app.get_webview_window(&label) {
        begin_explicit_prototype_close(&label);
        window
            .close()
            .map_err(|e| format!("No se pudo cerrar la superficie WebView2: {e}"))?;
    }
    Ok(())
}

pub(crate) fn close_animated_activation(
    app: &AppHandle,
    handle: &AnimatedActivationHandle,
) -> Result<(), String> {
    let label = prototype_surface_label(&handle.monitor_id);
    {
        let mut surfaces = prototype_surfaces()
            .lock()
            .map_err(|_| "No se pudo bloquear el registro de superficies".to_string())?;
        let Some(record) = surfaces.get_mut(&label) else {
            return Ok(());
        };
        if record.activation_id != handle.activation_id {
            eprintln!("ANIMATED_CLOSE_IGNORED surface={label} reason=stale_activation");
            return Ok(());
        }
        if matches!(
            record.lifecycle.state(),
            AnimatedSurfaceState::Closing
                | AnimatedSurfaceState::Closed
                | AnimatedSurfaceState::FallbackRequired
        ) {
            return Ok(());
        }
        record.lifecycle.begin_close();
    }
    if let Some(window) = app.get_webview_window(&label) {
        window
            .close()
            .map_err(|error| format!("No se pudo cerrar la superficie Animated: {error}"))?;
    } else {
        eprintln!(
            "ANIMATED_CLOSE_CONFIRMATION_PENDING surface={label} activation={} reason=window_not_found",
            handle.activation_id
        );
    }
    Ok(())
}

#[tauri::command]
pub fn report_animated_ready(
    app: AppHandle,
    surface_id: String,
    activation_id: String,
) -> Result<(), String> {
    let (assignment, wallpaper_id) = {
        let mut surfaces = prototype_surfaces()
            .lock()
            .map_err(|_| "No se pudo bloquear el registro de superficies".to_string())?;
        let Some(record) = surfaces.get_mut(&surface_id) else {
            eprintln!("ANIMATED_READY_IGNORED surface={surface_id} reason=unknown_surface");
            return Ok(());
        };
        if record.activation_id != activation_id {
            eprintln!("ANIMATED_READY_IGNORED surface={surface_id} reason=stale_activation");
            return Ok(());
        }
        if matches!(
            record.lifecycle.state(),
            AnimatedSurfaceState::Closing
                | AnimatedSurfaceState::Closed
                | AnimatedSurfaceState::FallbackRequired
        ) {
            eprintln!("ANIMATED_READY_IGNORED surface={surface_id} reason=not_active");
            return Ok(());
        }
        if record.lifecycle.state() == AnimatedSurfaceState::Playing {
            eprintln!("ANIMATED_READY_IGNORED surface={surface_id} reason=already_playing");
            return Ok(());
        }
        record.lifecycle.playing().map_err(str::to_string)?;
        (record.assignment.clone(), record.wallpaper_id.clone())
    };

    let activation_result = wallpaper_coordinator::global()
        .lock()
        .map_err(|_| "No se pudo bloquear el coordinador de wallpapers".to_string())
        .and_then(|mut coordinator| {
            coordinator
                .confirm_previous_deactivated(&assignment)
                .map_err(|error| error.to_string())?;
            coordinator
                .confirm_activated(&assignment)
                .map_err(|error| error.to_string())?;
            coordinator
                .commit_with_evidence(
                    &assignment,
                    AssignmentEvidence::ResourceReadyVisibilityUnverified,
                )
                .map_err(|error| error.to_string())
        });
    if let Err(error) = activation_result {
        signal_activation(&activation_id, AnimatedActivationOutcome::Failed);
        emit_activation_event(
            &app,
            AnimatedActivationEvent {
                state: "failed",
                monitor_id: assignment.assignment.monitor.to_string(),
                generation: assignment.assignment.generation.value(),
                activation_id: activation_id.clone(),
                wallpaper_id: wallpaper_id.clone(),
                error: Some(error.clone()),
            },
        );
        begin_explicit_prototype_close(&surface_id);
        if let Ok(mut coordinator) = wallpaper_coordinator::global().lock() {
            coordinator.abort(&assignment);
        }
        if let Some(window) = app.get_webview_window(&surface_id) {
            let _ = window.close();
        }
        return Err(error);
    }
    if let Some(wallpaper_id) = wallpaper_id.clone() {
        let mut conn = crate::db::get_connection().map_err(|error| error.to_string())?;
        if let Err(error) = crate::commands::wallpaper::persist_monitor_wallpaper_assignment(
            &mut conn,
            assignment.assignment.monitor.as_str(),
            &wallpaper_id,
        ) {
            signal_activation(&activation_id, AnimatedActivationOutcome::Failed);
            emit_activation_event(
                &app,
                AnimatedActivationEvent {
                    state: "failed",
                    monitor_id: assignment.assignment.monitor.to_string(),
                    generation: assignment.assignment.generation.value(),
                    activation_id: activation_id.clone(),
                    wallpaper_id: Some(wallpaper_id.clone()),
                    error: Some(error.to_string()),
                },
            );
            begin_explicit_prototype_close(&surface_id);
            if let Ok(mut coordinator) = wallpaper_coordinator::global().lock() {
                coordinator.abort(&assignment);
            }
            if let Some(window) = app.get_webview_window(&surface_id) {
                let _ = window.close();
            }
            return Err(format!(
                "Animated está listo como recurso, pero no se pudo persistir la asignación del monitor: {error}"
            ));
        }
    }
    signal_activation(&activation_id, AnimatedActivationOutcome::Ready);
    emit_activation_event(
        &app,
        AnimatedActivationEvent {
            state: "ready",
            monitor_id: assignment.assignment.monitor.to_string(),
            generation: assignment.assignment.generation.value(),
            activation_id: activation_id.clone(),
            wallpaper_id,
            error: None,
        },
    );
    eprintln!(
        "ANIMATED_READY surface={surface_id} activation={activation_id} backend=animated resource_ready=true visibility=unverified effective=false"
    );
    Ok(())
}

#[tauri::command]
pub fn report_animated_media_error(
    app: AppHandle,
    surface_id: String,
    activation_id: String,
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
        if !activation_matches(record, &activation_id) {
            eprintln!("ANIMATED_MEDIA_ERROR_IGNORED surface={surface_id} reason=stale_activation");
            return Ok(());
        }
        if matches!(
            record.lifecycle.state(),
            AnimatedSurfaceState::Closing
                | AnimatedSurfaceState::Closed
                | AnimatedSurfaceState::FallbackRequired
        ) {
            eprintln!(
                "ANIMATED_MEDIA_ERROR_IGNORED surface={surface_id} reason=surface_closing_closed_or_fallback"
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

    signal_activation(&activation_id, AnimatedActivationOutcome::Failed);

    if let Some(window) = app.get_webview_window(&surface_id) {
        if let Err(error) = window.close() {
            eprintln!("ANIMATED_MEDIA_ERROR_CLOSE_FAILED surface={surface_id} error={error}");
            eprintln!(
                "ANIMATED_CLOSE_CONFIRMATION_PENDING surface={surface_id} activation={activation_id} reason=close_request_failed"
            );
        }
    } else {
        eprintln!(
            "ANIMATED_CLOSE_CONFIRMATION_PENDING surface={surface_id} activation={activation_id} reason=window_not_found"
        );
    }

    if let Ok(surfaces) = prototype_surfaces().lock() {
        if let Some(record) = surfaces.get(&surface_id) {
            emit_activation_event(
                &app,
                AnimatedActivationEvent {
                    state: "failed",
                    monitor_id: record.assignment.assignment.monitor.to_string(),
                    generation: record.assignment.assignment.generation.value(),
                    activation_id: record.activation_id.clone(),
                    wallpaper_id: record.wallpaper_id.clone(),
                    error: reason.clone(),
                },
            );
        }
    }

    if decision == FailureDecision::FallbackRequired {
        eprintln!("ANIMATED_SURFACE_FALLBACK_REQUIRED surface={surface_id}");
    }
    Ok(())
}

/// Requests shutdown of every animated surface owned by this process.
///
/// Native surfaces join their message threads synchronously. Tauri/WebView2
/// surfaces are asynchronous: the `Destroyed` event is the only confirmation
/// that their HWND/WebView2 lifecycle actually finished. The return value is
/// therefore the number of Tauri surfaces still awaiting that callback.
pub fn close_all_animated_prototypes(app: &AppHandle) -> usize {
    let mut pending_destroyed_callbacks = 0;
    #[cfg(target_os = "windows")]
    {
        if let Err(error) = native_window::close() {
            eprintln!("ANIMATED_NATIVE_CLOSE_FAILED error={error}");
        }
        native_window::close_all_physical_surfaces();
    }
    for (label, window) in app.webview_windows() {
        if label.starts_with("animated-prototype-") {
            begin_explicit_prototype_close(&label);
            match window.close() {
                Ok(()) => {
                    pending_destroyed_callbacks += 1;
                    eprintln!("ANIMATED_CLOSE_REQUESTED surface={label} confirmation=Destroyed");
                }
                Err(error) => {
                    eprintln!("ANIMATED_CLOSE_REQUEST_FAILED surface={label} error={error}")
                }
            }
        }
    }
    if pending_destroyed_callbacks > 0 {
        eprintln!("ANIMATED_CLOSE_CONFIRMATION_PENDING count={pending_destroyed_callbacks}");
    }
    pending_destroyed_callbacks
}

#[cfg(test)]
mod tests {
    use super::{
        activation_matches, bounds_from_geometry, create_diagnostic_gif,
        diagnostic_surface_conflicts, encode_query_component, prototype_surface_label,
        register_activation, resource_limit_reason, signal_activation, smoke_variant_options,
        take_destroyed_surface, validate_animated_gif, validate_media_source, wait_for_activation,
        AnimatedActivationEvent, AnimatedActivationHandle, AnimatedActivationOutcome,
        AnimatedMediaFormat, AnimatedSurfaceLifecycle, PrototypeSurfaceRecord,
        MAX_ANIMATED_DIMENSION, MAX_ANIMATED_FILE_BYTES,
    };
    use crate::models::monitor_assignment::MonitorIdentity;
    use crate::models::monitor_assignment::WallpaperBackend;
    use crate::models::wallpaper_coordinator::WallpaperCoordinator;
    use image::GenericImageView;
    use std::{thread, time::Duration};
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
    fn prototype_labels_are_isolated_by_monitor_identity() {
        let first: MonitorIdentity = "\\\\?\\DISPLAY-A".into();
        let second: MonitorIdentity = "\\\\?\\DISPLAY-B".into();
        assert_ne!(
            prototype_surface_label(&first),
            prototype_surface_label(&second)
        );
    }

    #[test]
    fn activation_event_preserves_identity_and_unverified_state() {
        let event = AnimatedActivationEvent {
            state: "ready",
            monitor_id: "DISPLAY-A".into(),
            generation: 7,
            activation_id: "activation-7".into(),
            wallpaper_id: Some("wallpaper-7".into()),
            error: None,
        };
        let value = serde_json::to_value(event).unwrap();
        assert_eq!(value["state"], "ready");
        assert_eq!(value["monitorId"], "DISPLAY-A");
        assert_eq!(value["generation"], 7);
        assert_eq!(value["activationId"], "activation-7");
        assert_eq!(value["wallpaperId"], "wallpaper-7");
    }

    #[test]
    fn activation_wait_is_correlated_and_reaches_ready() {
        let activation_id = Uuid::new_v4().to_string();
        let handle = AnimatedActivationHandle {
            monitor_id: "DISPLAY-A".into(),
            generation: 11,
            activation_id: activation_id.clone(),
        };
        register_activation(&activation_id);
        let signal_id = activation_id.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(5));
            signal_activation(&signal_id, AnimatedActivationOutcome::Ready);
        });
        assert_eq!(
            wait_for_activation(&handle, Duration::from_secs(1)),
            AnimatedActivationOutcome::Ready
        );
        assert_eq!(
            wait_for_activation(&handle, Duration::from_millis(1)),
            AnimatedActivationOutcome::Cancelled
        );
    }

    #[test]
    fn diagnostic_surface_is_rejected_when_coordinator_state_exists() {
        assert!(!diagnostic_surface_conflicts(false, false));
        assert!(diagnostic_surface_conflicts(true, false));
        assert!(diagnostic_surface_conflicts(false, true));
    }

    #[test]
    fn stale_activation_cannot_target_a_new_surface_on_the_same_monitor() {
        let mut coordinator = WallpaperCoordinator::default();
        let assignment = coordinator
            .reserve("test-monitor".into(), WallpaperBackend::Animated)
            .unwrap();
        let old = PrototypeSurfaceRecord {
            lifecycle: AnimatedSurfaceLifecycle::new(),
            activation_id: "activation-old".into(),
            assignment,
            wallpaper_id: None,
            destroyed_confirmed: false,
        };
        assert!(activation_matches(&old, "activation-old"));
        assert!(!activation_matches(&old, "activation-new"));
    }

    #[test]
    fn destroyed_surface_is_removed_and_stale_close_keeps_new_record() {
        let mut coordinator = WallpaperCoordinator::default();
        let old_assignment = coordinator
            .reserve("test-monitor".into(), WallpaperBackend::Animated)
            .unwrap();
        let mut surfaces = std::collections::BTreeMap::new();
        surfaces.insert(
            "animated-surface".into(),
            PrototypeSurfaceRecord {
                lifecycle: AnimatedSurfaceLifecycle::new(),
                activation_id: "activation-old".into(),
                assignment: old_assignment,
                wallpaper_id: None,
                destroyed_confirmed: false,
            },
        );

        assert!(
            take_destroyed_surface(&mut surfaces, "animated-surface", Some("activation-old"))
                .is_some()
        );
        assert!(!surfaces.contains_key("animated-surface"));

        let mut newer_coordinator = WallpaperCoordinator::default();
        let new_assignment = newer_coordinator
            .reserve("test-monitor".into(), WallpaperBackend::Animated)
            .unwrap();
        surfaces.insert(
            "animated-surface".into(),
            PrototypeSurfaceRecord {
                lifecycle: AnimatedSurfaceLifecycle::new(),
                activation_id: "activation-new".into(),
                assignment: new_assignment,
                wallpaper_id: None,
                destroyed_confirmed: false,
            },
        );
        assert!(
            take_destroyed_surface(&mut surfaces, "animated-surface", Some("activation-old"))
                .is_none()
        );
        assert_eq!(
            surfaces
                .get("animated-surface")
                .map(|record| record.activation_id.as_str()),
            Some("activation-new")
        );
    }

    #[test]
    fn diagnostic_gif_is_readable_and_has_expected_dimensions() {
        let path = std::env::temp_dir().join(format!("lumina-diagnostic-{}.gif", Uuid::new_v4()));
        create_diagnostic_gif(&path).unwrap();
        assert_eq!(
            validate_media_source(&path).unwrap(),
            AnimatedMediaFormat::Gif
        );
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
    fn media_contract_rejects_mp4_and_unsupported_extensions() {
        let directory =
            std::env::temp_dir().join(format!("lumina-media-contract-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let mp4 = directory.join("clip.mp4");
        std::fs::write(&mp4, b"not a video").unwrap();
        let mp4_error = validate_media_source(&mp4).unwrap_err();
        assert!(mp4_error.contains("MP4"));

        let txt = directory.join("clip.txt");
        std::fs::write(&txt, b"GIF89a").unwrap();
        let text_error = validate_media_source(&txt).unwrap_err();
        assert!(text_error.contains("ResourceFormatUnsupported"));
        assert!(directory.exists());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn malformed_or_truncated_gif_is_rejected_before_resource_use() {
        let path = std::env::temp_dir().join(format!("lumina-truncated-{}.gif", Uuid::new_v4()));
        std::fs::write(&path, b"GIF89a").unwrap();
        let error = validate_animated_gif(&path, 2560, 1440).unwrap_err();
        assert!(error.contains("MediaError"));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn resource_limits_are_conservative_for_extreme_values() {
        assert_eq!(
            resource_limit_reason(u64::MAX, u32::MAX, u32::MAX, u32::MAX, u32::MAX),
            Some("ResourceLimitExceeded:file_size")
        );
        assert!(resource_limit_reason(1, u32::MAX, u32::MAX, 0, 0).is_some());
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
