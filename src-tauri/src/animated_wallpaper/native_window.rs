use super::{
    create_diagnostic_gif, desktop_host, prototype_directory,
    recovery::{
        RecoveryLifecycle, RecoveryPolicy, RecoveryProbe, RecoveryScenario, StartupRecoveryPlan,
    },
    MonitorBounds,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Condvar, Mutex, OnceLock,
    },
    thread,
    time::Duration,
};

use tauri::AppHandle;
use webview2_com::{
    CoreWebView2EnvironmentOptions, CreateCoreWebView2ControllerCompletedHandler,
    CreateCoreWebView2EnvironmentCompletedHandler, Microsoft::Web::WebView2::Win32::*,
    NavigationCompletedEventHandler, WebMessageReceivedEventHandler,
};
use windows_webview2::{
    core::{w, Interface, HSTRING, PCWSTR},
    Win32::{
        Foundation::{
            GetLastError, E_POINTER, HINSTANCE, HWND, LPARAM, LRESULT, WIN32_ERROR, WPARAM,
        },
        Graphics::Gdi::UpdateWindow,
        System::{
            Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED},
            LibraryLoader::GetModuleHandleW,
        },
        UI::HiDpi::GetDpiForWindow,
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
            GetParent, GetWindow, GetWindowLongPtrW, GetWindowRect, PostMessageW, PostQuitMessage,
            RegisterClassW, SetWindowPos, ShowWindow, TranslateMessage, CS_HREDRAW, CS_VREDRAW,
            GWL_EXSTYLE, GW_OWNER, HWND_BOTTOM, HWND_TOP, HWND_TOPMOST, MSG, SWP_NOACTIVATE,
            SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_SHOW, WM_APP, WM_CLOSE, WM_DESTROY,
            WM_NCDESTROY, WM_QUIT, WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
            WS_EX_TRANSPARENT, WS_POPUP,
        },
    },
};

const NATIVE_WINDOW_CLASS: PCWSTR = w!("LuminaNativeAnimatedPrototype");
const MAX_NATIVE_SURFACES: usize = 1;
const MAX_NATIVE_MEDIA_BYTES: u64 = 512 * 1024 * 1024;
const WM_NATIVE_MEDIA_ERROR: u32 = WM_APP + 59;

static NATIVE_SURFACE: OnceLock<Mutex<Option<isize>>> = OnceLock::new();
static NATIVE_THREAD: OnceLock<Mutex<Option<thread::JoinHandle<()>>>> = OnceLock::new();
static MULTI_SURFACES: OnceLock<Mutex<BTreeMap<String, isize>>> = OnceLock::new();
static MULTI_THREADS: OnceLock<Mutex<BTreeMap<String, thread::JoinHandle<()>>>> = OnceLock::new();
static REGISTERED_DESKTOP_HOST: OnceLock<Mutex<Option<desktop_host::RawDesktopHost>>> =
    OnceLock::new();
static DESIRED_SURFACES: OnceLock<Mutex<BTreeMap<String, DesiredSurface>>> = OnceLock::new();
static HOST_WATCHER: OnceLock<Mutex<Option<HostWatcher>>> = OnceLock::new();
static ACTIVE_MULTI_TEMPORARIES: OnceLock<Mutex<BTreeSet<PathBuf>>> = OnceLock::new();

#[derive(Clone)]
struct DesiredSurface {
    app: AppHandle,
    monitor_id: String,
    bounds: MonitorBounds,
    dpi: u32,
}

struct HostWatcher {
    stop: Arc<AtomicBool>,
    wake: Arc<(Mutex<bool>, Condvar)>,
    thread: thread::JoinHandle<()>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ZOrderVariant {
    Normal,
    Bottom,
    Top,
    Topmost,
    ToolWindow,
    ClickThrough,
}

impl ZOrderVariant {
    fn from_environment() -> Result<Self, String> {
        match std::env::var("LUMINA_NATIVE_Z_VARIANT")
            .unwrap_or_else(|_| "toolwindow".to_string())
            .to_ascii_lowercase()
            .as_str()
        {
            "normal" => Ok(Self::Normal),
            "bottom" => Ok(Self::Bottom),
            "top" => Ok(Self::Top),
            "topmost" => Ok(Self::Topmost),
            "toolwindow" => Ok(Self::ToolWindow),
            "clickthrough" => Ok(Self::ClickThrough),
            value => Err(format!(
                "LUMINA_NATIVE_Z_VARIANT inválida: {value}; usa normal, bottom, top, topmost, toolwindow o clickthrough"
            )),
        }
    }

    fn ex_style(self) -> windows_webview2::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE {
        match self {
            Self::Normal => Default::default(),
            Self::Bottom | Self::Top | Self::Topmost | Self::ToolWindow => {
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE
            }
            Self::ClickThrough => WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT,
        }
    }

    fn requested_z_order(self) -> Option<HWND> {
        match self {
            Self::Bottom => Some(HWND_BOTTOM),
            Self::Top => Some(HWND_TOP),
            Self::Topmost => Some(HWND_TOPMOST),
            Self::Normal | Self::ToolWindow | Self::ClickThrough => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Bottom => "bottom",
            Self::Top => "top",
            Self::Topmost => "topmost",
            Self::ToolWindow => "toolwindow",
            Self::ClickThrough => "clickthrough",
        }
    }
}

fn native_surface() -> &'static Mutex<Option<isize>> {
    NATIVE_SURFACE.get_or_init(|| Mutex::new(None))
}

fn native_thread() -> &'static Mutex<Option<thread::JoinHandle<()>>> {
    NATIVE_THREAD.get_or_init(|| Mutex::new(None))
}

fn multi_surfaces() -> &'static Mutex<BTreeMap<String, isize>> {
    MULTI_SURFACES.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn multi_threads() -> &'static Mutex<BTreeMap<String, thread::JoinHandle<()>>> {
    MULTI_THREADS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn registered_desktop_host() -> &'static Mutex<Option<desktop_host::RawDesktopHost>> {
    REGISTERED_DESKTOP_HOST.get_or_init(|| Mutex::new(None))
}

fn register_desktop_host(host: desktop_host::RawDesktopHost) {
    *registered_desktop_host()
        .lock()
        .expect("registered host mutex poisoned") = Some(host);
}

fn desired_surfaces() -> &'static Mutex<BTreeMap<String, DesiredSurface>> {
    DESIRED_SURFACES.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn host_watcher() -> &'static Mutex<Option<HostWatcher>> {
    HOST_WATCHER.get_or_init(|| Mutex::new(None))
}

fn active_multi_temporaries() -> &'static Mutex<BTreeSet<PathBuf>> {
    ACTIVE_MULTI_TEMPORARIES.get_or_init(|| Mutex::new(BTreeSet::new()))
}

fn is_owned_multi_temporary(path: &Path, directory: &Path) -> bool {
    path.parent() == Some(directory)
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("native-multimonitor-") && name.ends_with(".gif"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TemporaryClassification {
    OwnedOrphan,
    Active,
    Foreign,
}

fn classify_multi_temporary(
    path: &Path,
    directory: &Path,
    active: &BTreeSet<PathBuf>,
) -> TemporaryClassification {
    if !is_owned_multi_temporary(path, directory) {
        TemporaryClassification::Foreign
    } else if active.contains(path) {
        TemporaryClassification::Active
    } else {
        TemporaryClassification::OwnedOrphan
    }
}

pub fn cleanup_orphaned_multi_temporaries(directory: &Path) -> Result<(), String> {
    let active = active_multi_temporaries()
        .lock()
        .expect("active temporary mutex poisoned")
        .clone();
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            eprintln!(
                "[TempCleanup] STARTUP_CONTINUES reason=directory_unreadable path={} error={error}",
                directory.display()
            );
            return Ok(());
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                eprintln!("[TempCleanup] TEMP_PRESERVED reason=read_entry error={error}");
                continue;
            }
        };
        let path = entry.path();
        match classify_multi_temporary(&path, directory, &active) {
            TemporaryClassification::Foreign => {}
            TemporaryClassification::Active => eprintln!(
                "[TempCleanup] TEMP_PRESERVED reason=active path={}",
                path.display()
            ),
            TemporaryClassification::OwnedOrphan => match std::fs::remove_file(&path) {
                Ok(()) => eprintln!("[TempCleanup] TEMP_REMOVED path={}", path.display()),
                Err(error) => eprintln!(
                    "[TempCleanup] TEMP_PRESERVED reason=remove_failed path={} error={error}",
                    path.display()
                ),
            },
        }
    }
    Ok(())
}

pub fn prepare_startup(directory: &Path) -> Result<(), String> {
    let plan = StartupRecoveryPlan::for_new_process();
    if plan.discard_native_references {
        *registered_desktop_host()
            .lock()
            .expect("registered host mutex poisoned") = None;
        desired_surfaces()
            .lock()
            .expect("desired surfaces mutex poisoned")
            .clear();
    }
    if plan.clean_owned_temporaries {
        cleanup_orphaned_multi_temporaries(directory)?;
    }
    eprintln!(
        "[HostMonitor] STARTUP_CLEAN references_discarded={} watcher_requires_new_validation={}",
        plan.discard_native_references, plan.start_watcher_only_after_validation
    );
    Ok(())
}

unsafe extern "system" fn native_window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_CLOSE => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        WM_NCDESTROY => DefWindowProcW(window, message, wparam, lparam),
        _ => DefWindowProcW(window, message, wparam, lparam),
    }
}

fn register_native_window_class() -> Result<HINSTANCE, String> {
    let instance = unsafe {
        GetModuleHandleW(None).map_err(|error| format!("GetModuleHandleW falló: {error}"))?
    };
    let class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(native_window_proc),
        hInstance: instance.into(),
        lpszClassName: NATIVE_WINDOW_CLASS,
        ..Default::default()
    };
    let atom = unsafe { RegisterClassW(&class) };
    if atom == 0 {
        if unsafe { GetLastError() } == WIN32_ERROR(1410) {
            return Ok(instance.into());
        }
        return Err("RegisterClassW no pudo registrar la clase nativa".into());
    }
    Ok(instance.into())
}

fn create_native_window(bounds: MonitorBounds, variant: ZOrderVariant) -> Result<HWND, String> {
    let instance = register_native_window_class()?;
    let window = unsafe {
        CreateWindowExW(
            variant.ex_style(),
            NATIVE_WINDOW_CLASS,
            w!("Lumina Native Animated Prototype"),
            WS_POPUP,
            bounds.x,
            bounds.y,
            bounds.width as i32,
            bounds.height as i32,
            None,
            None,
            Some(instance),
            None,
        )
    }
    .map_err(|error| format!("CreateWindowExW falló: {error}"))?;
    eprintln!(
        "[NativeAnimated] HWND creado: {:?}; rect: ({}, {}, {}, {})",
        window, bounds.x, bounds.y, bounds.width, bounds.height
    );
    Ok(window)
}

fn apply_z_order(hwnd: HWND, variant: ZOrderVariant) -> Result<(), String> {
    if let Some(z_order) = variant.requested_z_order() {
        unsafe {
            SetWindowPos(
                hwnd,
                Some(z_order),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
            .map_err(|error| format!("SetWindowPos({}) falló: {error}", variant.label()))?;
        }
    }
    Ok(())
}

fn log_window_diagnostics(hwnd: HWND, bounds: MonitorBounds, variant: ZOrderVariant) {
    let mut rect = windows_webview2::Win32::Foundation::RECT::default();
    let rect_ok = unsafe { GetWindowRect(hwnd, &mut rect).is_ok() };
    let parent = unsafe { GetParent(hwnd) }.ok();
    let owner = unsafe { GetWindow(hwnd, GW_OWNER) }.ok();
    let previous = unsafe {
        GetWindow(
            hwnd,
            windows_webview2::Win32::UI::WindowsAndMessaging::GW_HWNDPREV,
        )
    }
    .ok();
    let ex_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    eprintln!(
        "[NativeAnimated] z-diagnostics variant={} hwnd={:?} requested_z={:?} rect=({}, {}, {}, {}) rect_ok={} monitor_rect=({}, {}, {}, {}) exstyle=0x{:X} parent={:?} owner={:?} z_previous={:?} dpi={}",
        variant.label(),
        hwnd,
        variant.requested_z_order(),
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top,
        rect_ok,
        bounds.x,
        bounds.y,
        bounds.width,
        bounds.height,
        ex_style,
        parent,
        owner,
        previous,
        dpi,
    );
}

fn encode_uri_component(value: &str) -> String {
    let mut result = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'/' | b'.' | b'-' | b'_' | b'~') {
            result.push(byte as char);
        } else {
            result.push('%');
            result.push_str(&format!("{byte:02X}"));
        }
    }
    result
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn native_html(media_path: &Path) -> String {
    let filename = media_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("diagnostic.gif");
    let uri = html_escape(&format!(
        "https://lumina.local/{}",
        encode_uri_component(filename)
    ));
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><style>
        html,body{{margin:0;width:100%;height:100%;overflow:hidden;background:#111827}}
        body{{display:grid;place-items:center;font-family:Segoe UI, sans-serif}}
        img{{width:100%;height:100%;object-fit:cover;display:block}}
        #status{{position:fixed;top:18px;left:18px;padding:10px 14px;border-radius:10px;
          color:#fff;background:rgba(15,23,42,.9);font-size:16px;z-index:2}}
        </style></head><body>
        <div id="status">Native HWND WebView2 · cargando GIF…</div>
        <img src="{uri}" alt="GIF de diagnóstico" onload="document.getElementById('status').textContent='Native HWND WebView2 · GIF cargado'" onerror="document.getElementById('status').textContent='Native HWND WebView2 · error de carga'">
        <script>document.querySelector('img').addEventListener('error', function() {{ window.chrome?.webview?.postMessage('lumina-media-error'); }});</script>
        </body></html>"#
    )
}

fn create_webview(
    hwnd: HWND,
    bounds: MonitorBounds,
    media_path: &Path,
) -> Result<ICoreWebView2Controller, String> {
    let (environment_tx, environment_rx) = mpsc::channel();
    let options = ICoreWebView2EnvironmentOptions::from(CoreWebView2EnvironmentOptions::default());
    let environment_handler = CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(
        move |error_code, environment| {
            let result: webview2_com::Result<ICoreWebView2Environment> = (|| {
                error_code?;
                environment.ok_or_else(|| webview2_com::Error::from(E_POINTER))
            })();
            environment_tx
                .send(result)
                .map_err(|_| windows_webview2::core::Error::from(E_POINTER))
        },
    ));
    unsafe {
        CreateCoreWebView2EnvironmentWithOptions(
            PCWSTR::null(),
            PCWSTR::null(),
            &options,
            &environment_handler,
        )
        .map_err(|error| format!("CreateCoreWebView2EnvironmentWithOptions falló: {error}"))?;
    }
    let environment = webview2_com::wait_with_pump(environment_rx)
        .map_err(|error| format!("WebView2 no pudo inicializarse: {error}"))?
        .map_err(|error| format!("WebView2 devolvió un error de inicialización: {error}"))?;
    eprintln!("[NativeAnimated] WebView2 inicializado");

    let (controller_tx, controller_rx) = mpsc::channel();
    let controller_handler = CreateCoreWebView2ControllerCompletedHandler::create(Box::new(
        move |error_code, controller| {
            let result: webview2_com::Result<ICoreWebView2Controller> = (|| {
                error_code?;
                controller.ok_or_else(|| webview2_com::Error::from(E_POINTER))
            })();
            controller_tx
                .send(result)
                .map_err(|_| windows_webview2::core::Error::from(E_POINTER))
        },
    ));
    let webview_hwnd = windows_webview2::Win32::Foundation::HWND(hwnd.0);
    unsafe {
        environment
            .CreateCoreWebView2Controller(webview_hwnd, &controller_handler)
            .map_err(|error| format!("CreateCoreWebView2Controller falló: {error}"))?;
    }
    let controller = webview2_com::wait_with_pump(controller_rx)
        .map_err(|error| format!("WebView2 Controller no pudo inicializarse: {error}"))?
        .map_err(|error| format!("WebView2 devolvió un error al crear el Controller: {error}"))?;
    let webview = unsafe {
        controller.CoreWebView2().map_err(|error| {
            controller.Close().ok();
            format!("WebViewError: no se pudo obtener ICoreWebView2: {error}")
        })?
    };
    let webview3 = webview.cast::<ICoreWebView2_3>().map_err(|error| {
        unsafe { controller.Close().ok() };
        format!("WebViewError: WebView2 no expone el mapeo de host local: {error}")
    })?;
    let folder = media_path
        .parent()
        .ok_or_else(|| "El medio nativo no tiene un directorio padre válido".to_string())?;
    let folder = HSTRING::from(folder.to_string_lossy().as_ref());
    let host = HSTRING::from("lumina.local");
    unsafe {
        webview3
            .SetVirtualHostNameToFolderMapping(
                &host,
                &folder,
                COREWEBVIEW2_HOST_RESOURCE_ACCESS_KIND_ALLOW,
            )
            .map_err(|error| {
                controller.Close().ok();
                format!("WebViewError: no se pudo mapear el medio local: {error}")
            })?;
    }
    let rect = windows_webview2::Win32::Foundation::RECT {
        left: 0,
        top: 0,
        right: bounds.width as i32,
        bottom: bounds.height as i32,
    };
    unsafe {
        controller.SetBounds(rect).map_err(|error| {
            controller.Close().ok();
            format!("WebViewError: no se pudo ajustar el tamaño: {error}")
        })?;
        controller.SetIsVisible(true).map_err(|error| {
            controller.Close().ok();
            format!("WebViewError: no se pudo mostrar WebView2: {error}")
        })?;
        let navigation_handler = NavigationCompletedEventHandler::create(Box::new(|_, _| {
            eprintln!("[NativeAnimated] URL cargada");
            Ok(())
        }));
        let mut token = 0i64;
        webview
            .add_NavigationCompleted(&navigation_handler, &mut token)
            .map_err(|error| {
                controller.Close().ok();
                format!("WebViewError: no se pudo registrar NavigationCompleted: {error}")
            })?;
        let media_error_hwnd = hwnd.0 as isize;
        let media_error_handler =
            WebMessageReceivedEventHandler::create(Box::new(move |_webview, _args| {
                eprintln!("[NativeAnimated] MEDIA_ERROR_MESSAGE_RECEIVED");
                let _ = PostMessageW(
                    Some(HWND(media_error_hwnd as *mut _)),
                    WM_NATIVE_MEDIA_ERROR,
                    WPARAM(0),
                    LPARAM(0),
                );
                Ok(())
            }));
        webview
            .add_WebMessageReceived(&media_error_handler, &mut token)
            .map_err(|error| {
                controller.Close().ok();
                format!("WebViewError: no se pudo registrar el canal de errores de media: {error}")
            })?;
        let html = HSTRING::from(native_html(media_path));
        webview.NavigateToString(&html).map_err(|error| {
            controller.Close().ok();
            format!("MediaError: no se pudo cargar el HTML del GIF: {error}")
        })?;
    }
    eprintln!("[NativeAnimated] GIF cargado en la ruta local del WebView2");
    Ok(controller)
}

fn validate_native_media(media_path: &Path) -> Result<(), String> {
    let metadata = std::fs::metadata(media_path)
        .map_err(|error| format!("MediaError: no se pudo leer el archivo: {error}"))?;
    if !metadata.is_file() {
        return Err("MediaError: la ruta no es un archivo".into());
    }
    if metadata.len() == 0 {
        return Err("MediaError: el archivo está vacío".into());
    }
    if metadata.len() > MAX_NATIVE_MEDIA_BYTES {
        return Err(format!(
            "MediaError: el archivo supera el límite de {} MiB",
            MAX_NATIVE_MEDIA_BYTES / (1024 * 1024)
        ));
    }
    let extension = media_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());
    if extension.as_deref() != Some("gif") {
        return Err("MediaError: el POC nativo solo admite GIF".into());
    }
    let reader = image::ImageReader::open(media_path)
        .map_err(|error| format!("MediaError: no se pudo abrir el GIF: {error}"))?
        .with_guessed_format()
        .map_err(|error| format!("MediaError: formato GIF inválido: {error}"))?;
    let image = reader
        .decode()
        .map_err(|error| format!("MediaError: no se pudo decodificar el GIF: {error}"))?;
    if image.width() == 0 || image.height() == 0 {
        return Err("MediaError: el GIF no tiene dimensiones válidas".into());
    }
    Ok(())
}

fn run_native_surface(
    bounds: MonitorBounds,
    media_path: std::path::PathBuf,
    variant: ZOrderVariant,
    desktop_host_target: Option<desktop_host::RawDesktopHost>,
    surface_key: Option<String>,
    ready: mpsc::Sender<Result<isize, String>>,
) {
    let com_initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok() };
    let result = (|| {
        let mut lifecycle = RecoveryLifecycle::new();
        let mut surface_lifecycle = super::recovery::AnimatedSurfaceLifecycle::new();
        surface_lifecycle
            .begin_create()
            .map_err(|error| error.to_string())?;
        if !com_initialized {
            return Err("CoInitializeEx no pudo inicializar COM".into());
        }
        if let Err(error) = validate_native_media(&media_path) {
            let decision = surface_lifecycle.fail(super::recovery::SurfaceFailureKind::Media);
            eprintln!(
                "[NativeAnimated] MEDIA_FAILURE state={:?} attempts={} decision={decision:?} error={error}",
                surface_lifecycle.state(),
                surface_lifecycle.failures()
            );
            return Err(error);
        }
        let hwnd = create_native_window(bounds, variant)?;
        if let Err(error) = apply_z_order(hwnd, variant) {
            unsafe {
                DestroyWindow(hwnd).ok();
            }
            return Err(error);
        }
        let attached_to_desktop = if let Some(host) = desktop_host_target {
            if let Err(error) = desktop_host::attach_native_window(hwnd.0 as isize, host, bounds) {
                unsafe {
                    DestroyWindow(hwnd).ok();
                }
                return Err(error);
            }
            true
        } else {
            false
        };
        let controller = match create_webview(hwnd, bounds, &media_path) {
            Ok(controller) => {
                surface_lifecycle
                    .webview_ready()
                    .map_err(|error| error.to_string())?;
                controller
            }
            Err(error) => {
                let decision = surface_lifecycle.fail(super::recovery::SurfaceFailureKind::WebView);
                eprintln!(
                    "[NativeAnimated] WEBVIEW_FAILURE state={:?} attempts={} decision={decision:?} error={error}",
                    surface_lifecycle.state(),
                    surface_lifecycle.failures()
                );
                unsafe {
                    if attached_to_desktop {
                        if let Err(detach_error) =
                            desktop_host::detach_native_window(hwnd.0 as isize)
                        {
                            eprintln!(
                                "[NativeAnimated] error al separar tras fallo de WebView2: {detach_error}"
                            );
                        }
                    }
                    DestroyWindow(hwnd).ok();
                }
                return Err(error);
            }
        };
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = UpdateWindow(hwnd);
        }
        surface_lifecycle
            .playing()
            .map_err(|error| error.to_string())?;
        lifecycle
            .create_renderer()
            .map_err(|error| error.to_string())?;
        if let Some(key) = surface_key.as_ref() {
            multi_surfaces()
                .lock()
                .expect("multi surface mutex poisoned")
                .insert(key.clone(), hwnd.0 as isize);
            eprintln!(
                "[MultiMonitorPOC] SURFACE_ACTIVE monitor_id={} hwnd={:?} bounds=({},{} {}x{})",
                key, hwnd, bounds.x, bounds.y, bounds.width, bounds.height
            );
        } else {
            *native_surface()
                .lock()
                .expect("native surface mutex poisoned") = Some(hwnd.0 as isize);
        }
        eprintln!("[NativeAnimated] ventana mostrada");
        log_window_diagnostics(hwnd, bounds, variant);
        ready
            .send(Ok(hwnd.0 as isize))
            .map_err(|_| "El llamador cerró el POC".to_string())?;

        let mut message = MSG::default();
        loop {
            let status = unsafe { GetMessageW(&mut message, None, 0, 0).0 };
            if status == -1 {
                return Err("GetMessageW falló durante el ciclo de vida nativo".into());
            }
            if status == 0 || message.message == WM_QUIT {
                break;
            }
            if message.message == WM_NATIVE_MEDIA_ERROR {
                let decision = surface_lifecycle.fail(super::recovery::SurfaceFailureKind::Media);
                eprintln!(
                    "[NativeAnimated] MEDIA_FAILURE state={:?} attempts={} decision={decision:?}",
                    surface_lifecycle.state(),
                    surface_lifecycle.failures()
                );
                surface_lifecycle.begin_close();
                break;
            }
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        eprintln!("[NativeAnimated] cierre solicitado");
        surface_lifecycle.begin_close();
        lifecycle.begin_cleanup();
        unsafe {
            if let Err(error) = controller.Close() {
                eprintln!("[NativeAnimated] WEBVIEW2_CLOSE_FAILED error={error}");
            }
        }
        eprintln!("[NativeAnimated] WebView2 destruido");
        eprintln!("[RecoveryPOC] WEBVIEW2_CLOSED");
        let detach_error = if attached_to_desktop {
            desktop_host::detach_native_window(hwnd.0 as isize).err()
        } else {
            None
        };
        if let Some(error) = &detach_error {
            eprintln!("[NativeAnimated] error al restaurar el parent del HWND: {error}");
        } else if attached_to_desktop {
            eprintln!("[RecoveryPOC] DETACHED");
        }
        unsafe {
            DestroyWindow(hwnd).ok();
        }
        eprintln!("[NativeAnimated] HWND destruido");
        eprintln!("[RecoveryPOC] HWND_DESTROYED");
        if let Some(key) = surface_key.as_ref() {
            multi_surfaces()
                .lock()
                .expect("multi surface mutex poisoned")
                .remove(key);
            eprintln!("[MultiMonitorPOC] SURFACE_DESTROYED monitor_id={key}");
        } else {
            *native_surface()
                .lock()
                .expect("native surface mutex poisoned") = None;
        }
        lifecycle.finish_cleanup();
        surface_lifecycle.finish_close();
        let _ = std::fs::remove_file(&media_path);
        active_multi_temporaries()
            .lock()
            .expect("active temporary mutex poisoned")
            .remove(&media_path);
        eprintln!("[NativeAnimated] temporal eliminado");
        if std::env::var_os("LUMINA_ANIMATED_DESKTOP_POC_FINAL_SNAPSHOT").is_some() {
            match desktop_host::snapshot_current_desktop() {
                Ok(snapshot) => eprintln!(
                    "[NativeAnimated] snapshot final del escritorio: {}",
                    serde_json::to_string(&snapshot)
                        .unwrap_or_else(|error| format!("<snapshot no serializable: {error}>"))
                ),
                Err(error) => {
                    eprintln!("[NativeAnimated] no se pudo obtener el snapshot final: {error}")
                }
            }
        }
        detach_error.map_or(Ok(()), Err)
    })();

    if let Err(error) = result {
        let _ = ready.send(Err(error.clone()));
        eprintln!("[NativeAnimated] error: {error}");
    }
    if com_initialized {
        unsafe {
            CoUninitialize();
        }
    }
}

fn open_internal(
    app: &AppHandle,
    monitor: MonitorBounds,
    variant: ZOrderVariant,
    desktop_host: Option<desktop_host::RawDesktopHost>,
) -> Result<(), String> {
    let active = native_surface()
        .lock()
        .expect("native surface mutex poisoned");
    if active.is_some() {
        return Err("Ya existe una superficie HWND nativa; ciérrala antes de abrir otra".into());
    }
    drop(active);
    if let Some(previous_thread) = native_thread()
        .lock()
        .expect("native thread mutex poisoned")
        .take()
    {
        previous_thread.join().map_err(|_| {
            "El hilo anterior del renderer nativo terminó de forma inesperada".to_string()
        })?;
    }
    let path =
        prototype_directory(app)?.join(format!("native-diagnostic-{}.gif", uuid::Uuid::new_v4()));
    create_diagnostic_gif(&path)?;
    if MAX_NATIVE_SURFACES != 1 {
        return Err("Configuración inválida del POC nativo".into());
    }
    let (ready_tx, ready_rx) = mpsc::channel();
    let thread_path = path.clone();
    eprintln!(
        "[NativeAnimated] iniciando variante Z-order={}",
        variant.label()
    );
    let handle = thread::spawn(move || {
        run_native_surface(monitor, thread_path, variant, desktop_host, None, ready_tx)
    });
    *native_thread()
        .lock()
        .expect("native thread mutex poisoned") = Some(handle);
    match ready_rx.recv_timeout(Duration::from_secs(15)) {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(error)) => {
            let _ = std::fs::remove_file(&path);
            if let Some(handle) = native_thread()
                .lock()
                .expect("native thread mutex poisoned")
                .take()
            {
                let _ = handle.join();
            }
            Err(error)
        }
        Err(_) => {
            let _ = std::fs::remove_file(&path);
            Err(
                "El renderer nativo no confirmó su inicialización dentro del tiempo esperado"
                    .into(),
            )
        }
    }
}

pub fn open(app: &AppHandle, monitor: MonitorBounds) -> Result<(), String> {
    let variant = ZOrderVariant::from_environment()?;
    open_internal(app, monitor, variant, None)
}

pub fn open_desktop_poc(app: &AppHandle, monitor: MonitorBounds) -> Result<(), String> {
    let (host, experiment) = desktop_host::prepare_modern_host()?;
    eprintln!(
        "[NativeAnimated] desktop POC experiment: {}",
        serde_json::to_string(&experiment).map_err(|error| format!(
            "No se pudo registrar el experimento de escritorio: {error}"
        ))?
    );
    register_desktop_host(host);
    open_internal(app, monitor, ZOrderVariant::ClickThrough, Some(host))
}

pub fn open_physical_surface(
    app: &AppHandle,
    monitor_id: String,
    monitor: MonitorBounds,
    dpi: u32,
    desktop_host: desktop_host::RawDesktopHost,
) -> Result<(), String> {
    open_physical_surface_internal(app, monitor_id, monitor, dpi, desktop_host, true)
}

fn open_physical_surface_internal(
    app: &AppHandle,
    monitor_id: String,
    monitor: MonitorBounds,
    dpi: u32,
    desktop_host: desktop_host::RawDesktopHost,
    remember_desired: bool,
) -> Result<(), String> {
    register_desktop_host(desktop_host);
    if multi_surfaces()
        .lock()
        .expect("multi surface mutex poisoned")
        .contains_key(&monitor_id)
    {
        return Err(format!(
            "Ya existe una superficie nativa para el monitor {monitor_id}"
        ));
    }
    if multi_threads()
        .lock()
        .expect("multi thread mutex poisoned")
        .contains_key(&monitor_id)
    {
        return Err(format!(
            "El monitor {monitor_id} todavía está cerrando su superficie anterior"
        ));
    }

    let path = prototype_directory(app)?.join(format!(
        "native-multimonitor-{}-{}.gif",
        sanitize_monitor_id(&monitor_id),
        uuid::Uuid::new_v4()
    ));
    create_diagnostic_gif(&path)?;
    active_multi_temporaries()
        .lock()
        .expect("active temporary mutex poisoned")
        .insert(path.clone());
    if remember_desired {
        desired_surfaces()
            .lock()
            .expect("desired surfaces mutex poisoned")
            .insert(
                monitor_id.clone(),
                DesiredSurface {
                    app: app.clone(),
                    monitor_id: monitor_id.clone(),
                    bounds: monitor,
                    dpi,
                },
            );
    }
    let (ready_tx, ready_rx) = mpsc::channel();
    let thread_path = path.clone();
    let key = monitor_id.clone();
    let handle = thread::spawn(move || {
        run_native_surface(
            monitor,
            thread_path,
            ZOrderVariant::ClickThrough,
            Some(desktop_host),
            Some(key),
            ready_tx,
        )
    });
    multi_threads()
        .lock()
        .expect("multi thread mutex poisoned")
        .insert(monitor_id.clone(), handle);
    match ready_rx.recv_timeout(Duration::from_secs(15)) {
        Ok(Ok(hwnd)) => {
            eprintln!(
                "[MultiMonitorPOC] RENDERER_CREATED monitor_id={} hwnd=0x{:x} host=0x{:x}",
                monitor_id, hwnd, desktop_host.1
            );
            start_host_watcher();
            Ok(())
        }
        Ok(Err(error)) => {
            if remember_desired {
                desired_surfaces()
                    .lock()
                    .expect("desired surfaces mutex poisoned")
                    .remove(&monitor_id);
            }
            let _ = std::fs::remove_file(&path);
            active_multi_temporaries()
                .lock()
                .expect("active temporary mutex poisoned")
                .remove(&path);
            if let Some(handle) = multi_threads()
                .lock()
                .expect("multi thread mutex poisoned")
                .remove(&monitor_id)
            {
                let _ = handle.join();
            }
            Err(error)
        }
        Err(_) => {
            if remember_desired {
                desired_surfaces()
                    .lock()
                    .expect("desired surfaces mutex poisoned")
                    .remove(&monitor_id);
            }
            let _ = std::fs::remove_file(&path);
            active_multi_temporaries()
                .lock()
                .expect("active temporary mutex poisoned")
                .remove(&path);
            Err(format!(
                "La superficie del monitor {monitor_id} no confirmó su inicialización"
            ))
        }
    }
}

pub fn validate_registered_desktop_host() -> Result<desktop_host::HostValidation, String> {
    let host = registered_desktop_host()
        .lock()
        .expect("registered host mutex poisoned")
        .ok_or("No hay un host Explorer registrado")?;
    Ok(desktop_host::validate_registered_host(host))
}

fn start_host_watcher() {
    let mut slot = host_watcher().lock().expect("host watcher mutex poisoned");
    if slot.is_some() {
        return;
    }
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let wake = Arc::new((Mutex::new(false), Condvar::new()));
    let thread_wake = Arc::clone(&wake);
    let handle = thread::spawn(move || {
        let mut recovery_policy = RecoveryPolicy::new();
        loop {
            let interval = match recovery_policy.probe() {
                RecoveryProbe::Immediate => Duration::from_secs(2),
                RecoveryProbe::LowFrequency => Duration::from_secs(30),
            };
            let (wake_lock, wake_condition) = &*thread_wake;
            let guard = wake_lock.lock().expect("host watcher wake mutex poisoned");
            let (_guard, timeout) = wake_condition
                .wait_timeout_while(guard, interval, |requested| {
                    !*requested && !thread_stop.load(Ordering::Acquire)
                })
                .expect("host watcher wait failed");
            if thread_stop.load(Ordering::Acquire) {
                break;
            }
            if desired_surfaces()
                .lock()
                .expect("desired surfaces mutex poisoned")
                .is_empty()
            {
                break;
            }
            if recovery_policy.exhausted() && timeout.timed_out() {
                eprintln!("[HostMonitor] RECOVERY_PROBE low_frequency=true");
                if recover_desired_surfaces() {
                    recovery_policy.on_success();
                }
                continue;
            }
            match validate_registered_desktop_host() {
                Ok(desktop_host::HostValidation::Stable) => {
                    recovery_policy.on_success();
                }
                Ok(desktop_host::HostValidation::Transient(reason)) => {
                    eprintln!("[HostMonitor] HOST_TRANSIENT_ERROR reason={reason}");
                }
                Ok(desktop_host::HostValidation::Changed(reason)) => {
                    eprintln!("[HostMonitor] HOST_CHANGED reason={reason}");
                    if recover_desired_surfaces() {
                        recovery_policy.on_success();
                    } else {
                        recovery_policy.on_failure();
                        if recovery_policy.exhausted() {
                            eprintln!(
                                "[HostMonitor] RECOVERY_DEGRADED retry_limit={} desired_surfaces_preserved=true next_probe_seconds=30",
                                recovery_policy.attempts()
                            );
                        }
                    }
                }
                Err(error) => {
                    eprintln!("[HostMonitor] watcher validation failed: {error}");
                }
            }
        }
        eprintln!("[HostMonitor] WATCHER_STOPPED");
        eprintln!("[HostMonitor] WATCHER_TERMINATED");
    });
    eprintln!("[HostMonitor] WATCHER_STARTED");
    *slot = Some(HostWatcher {
        stop,
        wake,
        thread: handle,
    });
}

fn stop_host_watcher() {
    let watcher = host_watcher()
        .lock()
        .expect("host watcher mutex poisoned")
        .take();
    if let Some(watcher) = watcher {
        watcher.stop.store(true, Ordering::Release);
        let (wake_lock, wake_condition) = &*watcher.wake;
        if let Ok(mut requested) = wake_lock.lock() {
            *requested = true;
            wake_condition.notify_one();
        }
        let _ = watcher.thread.join();
    }
}

fn recover_desired_surfaces() -> bool {
    eprintln!("[HostMonitor] RECOVERY_STARTED");
    let desired = desired_surfaces()
        .lock()
        .expect("desired surfaces mutex poisoned")
        .values()
        .cloned()
        .collect::<Vec<_>>();
    let ids = multi_surfaces()
        .lock()
        .expect("multi surface mutex poisoned")
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    for id in ids {
        if let Err(error) = stop_physical_surface(&id) {
            eprintln!("[HostMonitor] RECOVERY_CLEANUP_FAILED monitor_id={id} error={error}");
            return false;
        }
    }

    let (host, experiment) = match desktop_host::prepare_modern_host() {
        Ok(value) if value.1.host_is_unambiguous => value,
        Ok((_, experiment)) => {
            eprintln!(
                "[HostMonitor] HOST_UNAVAILABLE candidate_count={}",
                experiment.candidate_count
            );
            return false;
        }
        Err(error) => {
            eprintln!("[HostMonitor] HOST_UNAVAILABLE error={error}");
            return false;
        }
    };
    register_desktop_host(host);
    eprintln!(
        "[HostMonitor] HOST_REDISCOVERED workerw=0x{:x} candidate_count={}",
        host.1, experiment.candidate_count
    );
    let mut recovered = 0usize;
    let desired_count = desired.len();
    for surface in desired {
        let still_desired = desired_surfaces()
            .lock()
            .expect("desired surfaces mutex poisoned")
            .contains_key(&surface.monitor_id);
        if !still_desired {
            eprintln!(
                "[HostMonitor] SURFACE_RECOVERY_SKIPPED monitor_id={} reason=explicit_close",
                surface.monitor_id
            );
            continue;
        }
        match open_physical_surface_internal(
            &surface.app,
            surface.monitor_id.clone(),
            surface.bounds,
            surface.dpi,
            host,
            false,
        ) {
            Ok(()) => {
                recovered += 1;
                eprintln!(
                    "[HostMonitor] SURFACE_RECOVERED monitor_id={}",
                    surface.monitor_id
                );
            }
            Err(error) => eprintln!(
                "[HostMonitor] SURFACE_RECOVERY_FAILED monitor_id={} error={error}",
                surface.monitor_id
            ),
        }
    }
    eprintln!(
        "[HostMonitor] RECOVERY_COMPLETED recovered={} desired={}",
        recovered, desired_count
    );
    recovered == desired_count
}

fn sanitize_monitor_id(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

pub fn close_physical_surface(monitor_id: &str) -> Result<(), String> {
    desired_surfaces()
        .lock()
        .expect("desired surfaces mutex poisoned")
        .remove(monitor_id);
    let result = stop_physical_surface(monitor_id);
    if desired_surfaces()
        .lock()
        .expect("desired surfaces mutex poisoned")
        .is_empty()
    {
        stop_host_watcher();
    }
    result
}

fn stop_physical_surface(monitor_id: &str) -> Result<(), String> {
    let hwnd = multi_surfaces()
        .lock()
        .expect("multi surface mutex poisoned")
        .get(monitor_id)
        .copied();
    if let Some(hwnd) = hwnd {
        unsafe {
            PostMessageW(Some(HWND(hwnd as *mut _)), WM_CLOSE, WPARAM(0), LPARAM(0))
                .map_err(|error| format!("No se pudo cerrar {monitor_id}: {error}"))?;
        }
    }
    if let Some(thread) = multi_threads()
        .lock()
        .expect("multi thread mutex poisoned")
        .remove(monitor_id)
    {
        thread
            .join()
            .map_err(|_| format!("El hilo de {monitor_id} terminó inesperadamente"))?;
    }
    Ok(())
}

pub fn close_all_physical_surfaces() {
    desired_surfaces()
        .lock()
        .expect("desired surfaces mutex poisoned")
        .clear();
    stop_host_watcher();
    let ids: Vec<String> = multi_surfaces()
        .lock()
        .expect("multi surface mutex poisoned")
        .keys()
        .cloned()
        .collect();
    for id in ids {
        if let Err(error) = close_physical_surface(&id) {
            eprintln!("[MultiMonitorPOC] cleanup monitor_id={id} error={error}");
        }
    }
}

pub fn open_desktop_recovery_poc(app: &AppHandle, monitor: MonitorBounds) -> Result<(), String> {
    let (host, experiment) = desktop_host::prepare_modern_host()?;
    let candidate_count = experiment.candidate_count;
    eprintln!(
        "[RecoveryPOC] HOST_DISCOVERED progman={} workerw={} shell_view={} candidate_count={candidate_count}",
        host.0, host.1, host.2
    );
    if !experiment.host_is_unambiguous {
        eprintln!("[RecoveryPOC] HOST_UNAVAILABLE");
        return Err("El host inicial del POC de recuperación no es inequívoco".into());
    }
    eprintln!("[RecoveryPOC] HOST_VALIDATED");
    open_internal(app, monitor, ZOrderVariant::ClickThrough, Some(host))?;
    let mut lifecycle = RecoveryLifecycle::new();
    lifecycle
        .create_renderer()
        .map_err(|error| format!("No se pudo registrar el renderer inicial: {error}"))?;
    eprintln!(
        "[RecoveryPOC] RENDERER_CREATED renderer_count={} renderer_phase=initial state={:?}",
        lifecycle.renderer_count(),
        lifecycle.state()
    );
    eprintln!("[RecoveryPOC] ATTACHED");

    let app = app.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(3));
        eprintln!("[RecoveryPOC] RECOVERY_TRIGGERED");
        eprintln!("[RecoveryPOC] RENDERER_STOPPING");
        lifecycle.begin_cleanup();
        if let Err(error) = close() {
            eprintln!("[RecoveryPOC] RECOVERY_ABORTED cleanup_error={error}");
            return;
        }
        lifecycle.finish_cleanup();
        eprintln!("[RecoveryPOC] HOST_INVALIDATED");
        if let Err(error) = close() {
            eprintln!("[RecoveryPOC] RECOVERY_ABORTED idempotent_cleanup_error={error}");
            return;
        }
        eprintln!(
            "[RecoveryPOC] CLEANUP_IDEMPOTENT renderer_count={} state={:?}",
            lifecycle.renderer_count(),
            lifecycle.state()
        );

        let scenario = RecoveryScenario::from_environment();
        if let Some(candidate_count) = scenario.simulated_candidate_count() {
            eprintln!(
                "[RecoveryPOC] HOST_UNAVAILABLE candidate_count={candidate_count} simulated=true"
            );
            eprintln!("[RecoveryPOC] RECOVERY_ABORTED");
            return;
        }

        let (new_host, new_experiment) = match desktop_host::prepare_modern_host() {
            Ok(value) => value,
            Err(error) => {
                eprintln!("[RecoveryPOC] HOST_UNAVAILABLE error={error}");
                eprintln!("[RecoveryPOC] RECOVERY_ABORTED");
                return;
            }
        };
        eprintln!(
            "[RecoveryPOC] HOST_REDISCOVERED progman={} workerw={} shell_view={} candidate_count={}",
            new_host.0, new_host.1, new_host.2, new_experiment.candidate_count
        );
        if !new_experiment.host_is_unambiguous
            || scenario == RecoveryScenario::SecondValidationFails
        {
            eprintln!(
                "[RecoveryPOC] HOST_UNAVAILABLE candidate_count={} simulated={}",
                new_experiment.candidate_count,
                scenario == RecoveryScenario::SecondValidationFails
            );
            eprintln!("[RecoveryPOC] RECOVERY_ABORTED");
            return;
        }
        eprintln!("[RecoveryPOC] HOST_REVALIDATED");
        if let Err(error) = lifecycle.create_renderer() {
            eprintln!("[RecoveryPOC] RECOVERY_ABORTED renderer_guard={error}");
            return;
        }
        if let Err(error) =
            open_internal(&app, monitor, ZOrderVariant::ClickThrough, Some(new_host))
        {
            lifecycle.finish_cleanup();
            eprintln!("[RecoveryPOC] RECOVERY_ABORTED renderer_error={error}");
            return;
        }
        eprintln!(
            "[RecoveryPOC] RENDERER_RECREATED renderer_count={} renderer_phase=recovered state={:?}",
            lifecycle.renderer_count(),
            lifecycle.state()
        );
        eprintln!("[RecoveryPOC] RECOVERY_COMPLETED");
    });
    Ok(())
}

pub fn close() -> Result<(), String> {
    let handle = *native_surface()
        .lock()
        .expect("native surface mutex poisoned");
    if let Some(handle) = handle {
        unsafe {
            PostMessageW(Some(HWND(handle as *mut _)), WM_CLOSE, WPARAM(0), LPARAM(0)).map_err(
                |error| format!("No se pudo solicitar el cierre del HWND nativo: {error}"),
            )?;
        }
    }
    if let Some(thread) = native_thread()
        .lock()
        .expect("native thread mutex poisoned")
        .take()
    {
        thread
            .join()
            .map_err(|_| "El hilo del renderer nativo terminó de forma inesperada".to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        active_multi_temporaries, classify_multi_temporary, cleanup_orphaned_multi_temporaries,
        create_diagnostic_gif, validate_native_media, TemporaryClassification,
    };
    use std::{
        collections::BTreeSet,
        fs,
        path::{Path, PathBuf},
    };

    fn test_directory() -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "lumina-animated-temp-test-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[test]
    fn temporary_classification_is_conservative() {
        let directory = test_directory();
        let owned = directory.join("native-multimonitor-display-1.gif");
        let active_path = directory.join("native-multimonitor-display-2.gif");
        let foreign = directory.join("native-diagnostic-old.gif");
        let active = BTreeSet::from([active_path.clone()]);

        assert_eq!(
            classify_multi_temporary(&owned, &directory, &active),
            TemporaryClassification::OwnedOrphan
        );
        assert_eq!(
            classify_multi_temporary(&active_path, &directory, &active),
            TemporaryClassification::Active
        );
        assert_eq!(
            classify_multi_temporary(&foreign, &directory, &active),
            TemporaryClassification::Foreign
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn cleanup_removes_only_orphaned_owned_temporaries_and_is_idempotent() {
        let directory = test_directory();
        let orphan = directory.join("native-multimonitor-orphan.gif");
        let active_path = directory.join("native-multimonitor-active.gif");
        let foreign = directory.join("native-diagnostic-legacy.gif");
        fs::write(&orphan, b"gif").unwrap();
        fs::write(&active_path, b"gif").unwrap();
        fs::write(&foreign, b"gif").unwrap();
        active_multi_temporaries()
            .lock()
            .unwrap()
            .insert(active_path.clone());

        cleanup_orphaned_multi_temporaries(Path::new(&directory)).unwrap();
        cleanup_orphaned_multi_temporaries(Path::new(&directory)).unwrap();

        assert!(!orphan.exists());
        assert!(active_path.exists());
        assert!(foreign.exists());
        active_multi_temporaries()
            .lock()
            .unwrap()
            .remove(&active_path);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn native_media_validation_rejects_missing_empty_wrong_format_and_large_files() {
        let directory = test_directory();
        let missing = directory.join("missing.gif");
        assert!(validate_native_media(&missing)
            .unwrap_err()
            .contains("MediaError"));

        let empty = directory.join("empty.gif");
        fs::write(&empty, []).unwrap();
        assert!(validate_native_media(&empty)
            .unwrap_err()
            .contains("MediaError"));

        let wrong_format = directory.join("wrong.txt");
        fs::write(&wrong_format, b"not media").unwrap();
        assert!(validate_native_media(&wrong_format)
            .unwrap_err()
            .contains("MediaError"));

        let large = directory.join("large.gif");
        let file = fs::File::create(&large).unwrap();
        file.set_len(super::MAX_NATIVE_MEDIA_BYTES + 1).unwrap();
        assert!(validate_native_media(&large)
            .unwrap_err()
            .contains("límite"));

        let valid = directory.join("valid.gif");
        create_diagnostic_gif(&valid).unwrap();
        assert!(validate_native_media(&valid).is_ok());
        fs::remove_dir_all(directory).unwrap();
    }
}
