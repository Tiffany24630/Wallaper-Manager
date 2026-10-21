use super::{
    create_diagnostic_gif, desktop_host, prototype_directory,
    recovery::{
        DesktopLifecycle, DesktopLifecycleAction, DesktopLifecycleEvent, RecoveryLifecycle,
        RecoveryPolicy, RecoveryProbe, RecoveryScenario, StartupRecoveryPlan,
    },
    validate_animated_gif, MonitorBounds,
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

use crate::models::monitor_assignment::MonitorIdentity;
use crate::models::monitor_assignment::WallpaperBackend;
use crate::models::wallpaper_coordinator::{self, AssignmentOperation};
use tauri::AppHandle;
use webview2_com::{
    CoTaskMemPWSTR, CoreWebView2EnvironmentOptions, CreateCoreWebView2ControllerCompletedHandler,
    CreateCoreWebView2EnvironmentCompletedHandler, Microsoft::Web::WebView2::Win32::*,
    NavigationCompletedEventHandler, WebMessageReceivedEventHandler,
};
use windows_webview2::{
    core::{w, Interface, HSTRING, PCWSTR, PWSTR},
    Win32::{
        Foundation::{
            GetLastError, E_POINTER, HINSTANCE, HWND, LPARAM, LRESULT, WIN32_ERROR, WPARAM,
        },
        Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromWindow, UpdateWindow, MONITORINFO, MONITOR_DEFAULTTONULL,
        },
        System::{
            Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED},
            LibraryLoader::GetModuleHandleW,
        },
        UI::HiDpi::GetDpiForWindow,
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
            GetParent, GetWindow, GetWindowLongPtrW, GetWindowRect, IsWindow, PostMessageW,
            PostQuitMessage, RegisterClassW, SetWindowPos, ShowWindow, TranslateMessage,
            CS_HREDRAW, CS_VREDRAW, GWL_EXSTYLE, GW_OWNER, HWND_BOTTOM, HWND_TOP, HWND_TOPMOST,
            MSG, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SW_SHOW,
            WM_APP, WM_CLOSE, WM_DESTROY, WM_NCDESTROY, WM_POWERBROADCAST, WM_QUIT, WNDCLASSW,
            WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_POPUP,
        },
    },
};

const NATIVE_WINDOW_CLASS: PCWSTR = w!("LuminaNativeAnimatedPrototype");
const MAX_NATIVE_SURFACES: usize = 1;
const MAX_NATIVE_MEDIA_BYTES: u64 = 512 * 1024 * 1024;
const WM_NATIVE_MEDIA_ERROR: u32 = WM_APP + 59;
const WM_DISPLAYCHANGE: u32 = 0x007E;
const WM_DPICHANGED: u32 = 0x02E0;
const WM_WTSSESSION_CHANGE: u32 = 0x02B1;
const PBT_APMSUSPEND: usize = 4;
const PBT_APMRESUMEAUTOMATIC: usize = 18;
const PBT_APMRESUMESUSPEND: usize = 7;
const PBT_APMRESUMECRITICAL: usize = 6;
const WTS_SESSION_LOCK: usize = 0x7;
const WTS_SESSION_UNLOCK: usize = 0x8;
const NOTIFY_FOR_THIS_SESSION: u32 = 0;

#[link(name = "Wtsapi32")]
unsafe extern "system" {
    fn WTSRegisterSessionNotification(hwnd: HWND, flags: u32) -> i32;
    fn WTSUnRegisterSessionNotification(hwnd: HWND) -> i32;
}

fn is_native_media_error_payload(json: &str) -> bool {
    serde_json::from_str::<String>(json)
        .map(|message| message == "lumina-media-error")
        .unwrap_or(false)
}

static NATIVE_SURFACE: OnceLock<Mutex<Option<isize>>> = OnceLock::new();
static NATIVE_THREAD: OnceLock<Mutex<Option<thread::JoinHandle<()>>>> = OnceLock::new();
static MULTI_SURFACES: OnceLock<Mutex<BTreeMap<MonitorIdentity, isize>>> = OnceLock::new();
static MULTI_THREADS: OnceLock<Mutex<BTreeMap<MonitorIdentity, thread::JoinHandle<()>>>> =
    OnceLock::new();
static REGISTERED_DESKTOP_HOST: OnceLock<Mutex<Option<desktop_host::RawDesktopHost>>> =
    OnceLock::new();
static DESIRED_SURFACES: OnceLock<Mutex<BTreeMap<MonitorIdentity, DesiredSurface>>> =
    OnceLock::new();
static HOST_WATCHER: OnceLock<Mutex<Option<HostWatcher>>> = OnceLock::new();
static ACTIVE_MULTI_TEMPORARIES: OnceLock<Mutex<BTreeSet<PathBuf>>> = OnceLock::new();
static DESKTOP_LIFECYCLE: OnceLock<Mutex<LifecycleRuntime>> = OnceLock::new();
static CLOSING_SURFACES: OnceLock<Mutex<BTreeSet<MonitorIdentity>>> = OnceLock::new();
static NATIVE_SURFACE_CLOSING: OnceLock<AtomicBool> = OnceLock::new();

#[derive(Clone)]
struct DesiredSurface {
    app: AppHandle,
    monitor_id: MonitorIdentity,
    bounds: MonitorBounds,
    dpi: u32,
}

struct HostWatcher {
    stop: Arc<AtomicBool>,
    wake: Arc<(Mutex<bool>, Condvar)>,
    thread: thread::JoinHandle<()>,
}

struct LifecycleRuntime {
    controller: DesktopLifecycle,
    operation_active: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct NotificationRegistration {
    session_registered: bool,
}

impl NotificationRegistration {
    fn mark_registered(&mut self) -> Result<(), &'static str> {
        if self.session_registered {
            return Err("las notificaciones de sesión ya están registradas");
        }
        self.session_registered = true;
        Ok(())
    }

    fn take_registered(&mut self) -> bool {
        let was_registered = self.session_registered;
        self.session_registered = false;
        was_registered
    }
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

fn multi_surfaces() -> &'static Mutex<BTreeMap<MonitorIdentity, isize>> {
    MULTI_SURFACES.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn multi_threads() -> &'static Mutex<BTreeMap<MonitorIdentity, thread::JoinHandle<()>>> {
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

fn desired_surfaces() -> &'static Mutex<BTreeMap<MonitorIdentity, DesiredSurface>> {
    DESIRED_SURFACES.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn host_watcher() -> &'static Mutex<Option<HostWatcher>> {
    HOST_WATCHER.get_or_init(|| Mutex::new(None))
}

fn active_multi_temporaries() -> &'static Mutex<BTreeSet<PathBuf>> {
    ACTIVE_MULTI_TEMPORARIES.get_or_init(|| Mutex::new(BTreeSet::new()))
}

fn closing_surfaces() -> &'static Mutex<BTreeSet<MonitorIdentity>> {
    CLOSING_SURFACES.get_or_init(|| Mutex::new(BTreeSet::new()))
}

fn mark_surface_closing(monitor_id: &MonitorIdentity) {
    closing_surfaces()
        .lock()
        .expect("closing surfaces mutex poisoned")
        .insert(monitor_id.clone());
}

fn clear_surface_closing(monitor_id: &MonitorIdentity) {
    closing_surfaces()
        .lock()
        .expect("closing surfaces mutex poisoned")
        .remove(monitor_id);
}

fn is_surface_closing(monitor_id: &MonitorIdentity) -> bool {
    closing_surfaces()
        .lock()
        .expect("closing surfaces mutex poisoned")
        .contains(monitor_id)
}

fn is_current_surface_window(monitor_id: Option<&MonitorIdentity>, hwnd: HWND) -> bool {
    if !unsafe { IsWindow(Some(hwnd)).as_bool() } {
        return false;
    }
    monitor_id.is_none_or(|monitor_id| {
        multi_surfaces()
            .lock()
            .expect("multi surface mutex poisoned")
            .get(monitor_id)
            .is_some_and(|current| *current == hwnd.0 as isize)
    })
}

fn native_surface_closing() -> &'static AtomicBool {
    NATIVE_SURFACE_CLOSING.get_or_init(|| AtomicBool::new(false))
}

fn update_desired_surface_geometry(monitor_id: &MonitorIdentity, bounds: MonitorBounds, dpi: u32) {
    if let Some(surface) = desired_surfaces()
        .lock()
        .expect("desired surfaces mutex poisoned")
        .get_mut(monitor_id)
    {
        surface.bounds = bounds;
        surface.dpi = dpi;
    }
}

fn current_monitor_geometry(hwnd: HWND) -> Result<(MonitorBounds, u32), String> {
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONULL) };
    if monitor.0.is_null() {
        return Err("MonitorFromWindow no devolvió un monitor válido".into());
    }
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        return Err(format!(
            "GetMonitorInfoW falló: {:?}",
            std::io::Error::last_os_error()
        ));
    }
    let rect = info.rcMonitor;
    let width = rect
        .right
        .checked_sub(rect.left)
        .ok_or("el ancho del monitor desborda i32")?;
    let height = rect
        .bottom
        .checked_sub(rect.top)
        .ok_or("el alto del monitor desborda i32")?;
    if width <= 0 || height <= 0 {
        return Err("el monitor actual no tiene dimensiones válidas".into());
    }
    let bounds = MonitorBounds {
        x: rect.left,
        y: rect.top,
        width: width as u32,
        height: height as u32,
    };
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    if dpi == 0 {
        return Err("GetDpiForWindow no devolvió un DPI válido".into());
    }
    Ok((bounds, dpi))
}

fn geometry_changed(
    previous: MonitorBounds,
    previous_dpi: u32,
    next: MonitorBounds,
    next_dpi: u32,
) -> bool {
    previous != next || previous_dpi != next_dpi
}

fn apply_native_geometry_change(
    hwnd: HWND,
    current_bounds: &mut MonitorBounds,
    current_dpi: &mut u32,
    surface_key: Option<&MonitorIdentity>,
    attached_to_desktop: bool,
    controller: &ICoreWebView2Controller,
) -> Result<(), String> {
    let (next_bounds, next_dpi) = current_monitor_geometry(hwnd)?;
    if !geometry_changed(*current_bounds, *current_dpi, next_bounds, next_dpi) {
        eprintln!(
            "[GeometryNative] GEOMETRY_UNCHANGED hwnd={:?} bounds=({},{} {}x{}) dpi={}",
            hwnd, next_bounds.x, next_bounds.y, next_bounds.width, next_bounds.height, next_dpi
        );
        return Ok(());
    }
    if attached_to_desktop
        && !matches!(
            validate_registered_desktop_host()?,
            desktop_host::HostValidation::Stable
        )
    {
        return Err("el host Explorer dejó de ser estable durante el cambio de geometría".into());
    }
    unsafe {
        SetWindowPos(
            hwnd,
            None,
            next_bounds.x,
            next_bounds.y,
            next_bounds.width as i32,
            next_bounds.height as i32,
            SWP_NOACTIVATE | SWP_NOZORDER | SWP_SHOWWINDOW,
        )
        .map_err(|error| format!("SetWindowPos para geometría falló: {error}"))?;
    }
    let rect = windows_webview2::Win32::Foundation::RECT {
        left: 0,
        top: 0,
        right: next_bounds.width as i32,
        bottom: next_bounds.height as i32,
    };
    unsafe {
        controller
            .SetBounds(rect)
            .map_err(|error| format!("WebView2 SetBounds para geometría falló: {error}"))?;
    }
    *current_bounds = next_bounds;
    *current_dpi = next_dpi;
    if let Some(key) = surface_key {
        update_desired_surface_geometry(key, next_bounds, next_dpi);
    }
    eprintln!(
        "[GeometryNative] GEOMETRY_APPLIED hwnd={:?} bounds=({},{} {}x{}) dpi={}",
        hwnd, next_bounds.x, next_bounds.y, next_bounds.width, next_bounds.height, next_dpi
    );
    Ok(())
}

fn desktop_lifecycle() -> &'static Mutex<LifecycleRuntime> {
    DESKTOP_LIFECYCLE.get_or_init(|| {
        Mutex::new(LifecycleRuntime {
            controller: DesktopLifecycle::new(),
            operation_active: false,
        })
    })
}

fn register_lifecycle_notifications(hwnd: HWND) -> Result<NotificationRegistration, String> {
    let registered = unsafe { WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) };
    if registered == 0 {
        return Err(format!(
            "WTSRegisterSessionNotification falló para {:?}: {:?}",
            hwnd,
            std::io::Error::last_os_error()
        ));
    }
    eprintln!(
        "[LifecycleNative] SESSION_NOTIFICATIONS_REGISTERED hwnd={:?}",
        hwnd
    );
    eprintln!(
        "[LifecycleNative] POWER_NOTIFICATIONS_READY hwnd={:?}",
        hwnd
    );
    let mut registration = NotificationRegistration::default();
    registration
        .mark_registered()
        .map_err(|error| error.to_string())?;
    Ok(registration)
}

fn unregister_lifecycle_notifications(hwnd: HWND, registration: &mut NotificationRegistration) {
    if !registration.take_registered() {
        return;
    }
    let unregistered = unsafe { WTSUnRegisterSessionNotification(hwnd) };
    if unregistered == 0 {
        eprintln!(
            "[LifecycleNative] SESSION_NOTIFICATIONS_UNREGISTER_FAILED hwnd={:?} error={:?}",
            hwnd,
            std::io::Error::last_os_error()
        );
    } else {
        eprintln!(
            "[LifecycleNative] LIFECYCLE_NOTIFICATIONS_UNREGISTERED hwnd={:?}",
            hwnd
        );
    }
}

fn queue_lifecycle_event(event: DesktopLifecycleEvent, source: &'static str) {
    let action = {
        let mut runtime = desktop_lifecycle()
            .lock()
            .expect("desktop lifecycle mutex poisoned");
        let action = runtime.controller.event(event);
        eprintln!(
            "[LifecycleNative] LIFECYCLE_TRANSITION source={} event={:?} state={:?} action={:?}",
            source,
            event,
            runtime.controller.state(),
            action
        );
        if matches!(
            action,
            DesktopLifecycleAction::SuspendSurfaces | DesktopLifecycleAction::RevalidateHost
        ) {
            if runtime.operation_active {
                eprintln!("[LifecycleNative] LIFECYCLE_EVENT_IGNORED reason=operation_in_progress");
                return;
            }
            runtime.operation_active = true;
        }
        action
    };

    match action {
        DesktopLifecycleAction::SuspendSurfaces => {
            eprintln!("[LifecycleNative] LIFECYCLE_SUSPEND_REQUESTED");
            thread::spawn(|| {
                if let Err(error) = suspend_desired_surfaces_for_lifecycle() {
                    eprintln!("[LifecycleNative] LIFECYCLE_SUSPEND_FAILED error={error}");
                }
                desktop_lifecycle()
                    .lock()
                    .expect("desktop lifecycle mutex poisoned")
                    .operation_active = false;
            });
        }
        DesktopLifecycleAction::RevalidateHost => {
            eprintln!("[LifecycleNative] LIFECYCLE_RECOVERY_REQUESTED");
            thread::spawn(|| {
                let host_valid = matches!(
                    validate_registered_desktop_host(),
                    Ok(desktop_host::HostValidation::Stable)
                );
                let mut runtime = desktop_lifecycle()
                    .lock()
                    .expect("desktop lifecycle mutex poisoned");
                if host_valid {
                    drop(runtime);
                    let recovered = recover_desired_surfaces_for_lifecycle();
                    runtime = desktop_lifecycle()
                        .lock()
                        .expect("desktop lifecycle mutex poisoned");
                    runtime.controller.recovery_finished(recovered);
                    eprintln!(
                        "[LifecycleNative] HOST_REVALIDATION_RESULT valid=true recovered={recovered}"
                    );
                } else {
                    runtime.controller.host_revalidated(false);
                    eprintln!("[LifecycleNative] HOST_REVALIDATION_RESULT valid=false");
                }
                runtime.operation_active = false;
            });
        }
        DesktopLifecycleAction::CloseSurfaces | DesktopLifecycleAction::NoAction => {}
    }
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
        WM_POWERBROADCAST => {
            match wparam.0 {
                PBT_APMSUSPEND => {
                    eprintln!(
                        "[LifecycleNative] POWER_EVENT_RECEIVED source=windows event=suspend"
                    );
                    queue_lifecycle_event(DesktopLifecycleEvent::Suspend, "windows");
                }
                PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND | PBT_APMRESUMECRITICAL => {
                    eprintln!("[LifecycleNative] POWER_EVENT_RECEIVED source=windows event=resume");
                    queue_lifecycle_event(DesktopLifecycleEvent::Resume, "windows");
                }
                _ => {}
            }
            LRESULT(1)
        }
        WM_WTSSESSION_CHANGE => {
            match wparam.0 {
                WTS_SESSION_LOCK => {
                    eprintln!("[LifecycleNative] SESSION_EVENT_RECEIVED source=windows event=lock");
                    queue_lifecycle_event(DesktopLifecycleEvent::SessionLock, "windows");
                }
                WTS_SESSION_UNLOCK => {
                    eprintln!(
                        "[LifecycleNative] SESSION_EVENT_RECEIVED source=windows event=unlock"
                    );
                    queue_lifecycle_event(DesktopLifecycleEvent::SessionUnlock, "windows");
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_DISPLAYCHANGE => {
            eprintln!(
                "[GeometryNative] DISPLAY_CHANGE_RECEIVED source=windows hwnd={:?} width={} height={}",
                window,
                (lparam.0 as u32 & 0xffff),
                ((lparam.0 as u64 >> 16) & 0xffff)
            );
            DefWindowProcW(window, message, wparam, lparam)
        }
        WM_DPICHANGED => {
            eprintln!(
                "[GeometryNative] DPI_CHANGE_RECEIVED source=windows hwnd={:?} dpi={}",
                window,
                (wparam.0 & 0xffff) as u32
            );
            DefWindowProcW(window, message, wparam, lparam)
        }
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
    let diagnostic_error = if std::env::var_os("LUMINA_ANIMATED_NATIVE_MEDIA_ERROR_TEST").is_some()
    {
        "<script>window.addEventListener('load', function() { setTimeout(function() { document.querySelector('img').src = 'https://lumina.local/__lumina_controlled_error__.gif'; }, 700); });</script>"
    } else {
        ""
    };
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
        {diagnostic_error}
        </body></html>"#
    )
}

fn create_webview(
    hwnd: HWND,
    bounds: MonitorBounds,
    media_path: &Path,
    surface_id: &str,
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
        let media_error_surface_id = surface_id.to_string();
        let media_error_handler = WebMessageReceivedEventHandler::create(Box::new(
            move |_webview, args| {
                let mut message = PWSTR::null();
                let valid = args
                    .and_then(|args| args.WebMessageAsJson(&mut message).ok())
                    .is_some_and(|_| {
                        let message = CoTaskMemPWSTR::from(message);
                        is_native_media_error_payload(&message.to_string())
                    });
                if !valid {
                    eprintln!(
                        "[NativeAnimated] MEDIA_ERROR_MESSAGE_IGNORED surface_id={media_error_surface_id} reason=invalid_payload"
                    );
                    return Ok(());
                }
                eprintln!(
                    "[NativeAnimated] MEDIA_ERROR_MESSAGE_RECEIVED surface_id={media_error_surface_id}"
                );
                let _ = PostMessageW(
                    Some(HWND(media_error_hwnd as *mut _)),
                    WM_NATIVE_MEDIA_ERROR,
                    WPARAM(0),
                    LPARAM(0),
                );
                Ok(())
            },
        ));
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

#[allow(unreachable_code)]
fn validate_native_media(media_path: &Path, bounds: MonitorBounds) -> Result<(), String> {
    validate_animated_gif(media_path, bounds.width, bounds.height).map(|resource| {
        eprintln!(
            "[NativeAnimated] RESOURCE_ACCEPTED dimensions={}x{} frames={} duration_ms={} estimated_decoded_frame_bytes={}",
            resource.width,
            resource.height,
            resource.frame_count,
            resource.duration_ms,
            resource.estimated_decoded_frame_bytes
        );
    })?;
    return Ok(());
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
    surface_key: Option<MonitorIdentity>,
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
        if let Err(error) = validate_native_media(&media_path, bounds) {
            let decision = surface_lifecycle.fail(super::recovery::SurfaceFailureKind::Media);
            eprintln!(
                "[NativeAnimated] MEDIA_FAILURE state={:?} attempts={} decision={decision:?} error={error}",
                surface_lifecycle.state(),
                surface_lifecycle.failures()
            );
            return Err(error);
        }
        let hwnd = create_native_window(bounds, variant)?;
        let mut applied_bounds = bounds;
        let mut applied_dpi = unsafe { GetDpiForWindow(hwnd) };
        let mut lifecycle_notifications = match register_lifecycle_notifications(hwnd) {
            Ok(registration) => registration,
            Err(error) => {
                unsafe {
                    DestroyWindow(hwnd).ok();
                }
                return Err(error);
            }
        };
        if let Err(error) = apply_z_order(hwnd, variant) {
            unregister_lifecycle_notifications(hwnd, &mut lifecycle_notifications);
            unsafe {
                DestroyWindow(hwnd).ok();
            }
            return Err(error);
        }
        let attached_to_desktop = if let Some(host) = desktop_host_target {
            if let Err(error) = desktop_host::attach_native_window(hwnd.0 as isize, host, bounds) {
                unregister_lifecycle_notifications(hwnd, &mut lifecycle_notifications);
                unsafe {
                    DestroyWindow(hwnd).ok();
                }
                return Err(error);
            }
            true
        } else {
            false
        };
        let surface_id = surface_key
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "native-single".to_string());
        let controller = match create_webview(hwnd, bounds, &media_path, &surface_id) {
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
                unregister_lifecycle_notifications(hwnd, &mut lifecycle_notifications);
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
            if message.message == WM_DISPLAYCHANGE || message.message == WM_DPICHANGED {
                if surface_key.as_ref().is_some_and(is_surface_closing)
                    || (surface_key.is_none() && native_surface_closing().load(Ordering::Acquire))
                    || !is_current_surface_window(surface_key.as_ref(), hwnd)
                {
                    eprintln!(
                        "[GeometryNative] GEOMETRY_IGNORED reason=closing_or_stale_surface hwnd={:?} surface_id={:?}",
                        hwnd, surface_key
                    );
                    continue;
                }
                if let Err(error) = apply_native_geometry_change(
                    hwnd,
                    &mut applied_bounds,
                    &mut applied_dpi,
                    surface_key.as_ref(),
                    attached_to_desktop,
                    &controller,
                ) {
                    eprintln!(
                        "[GeometryNative] GEOMETRY_REJECTED hwnd={:?} error={error}; closing_surface=true",
                        hwnd
                    );
                    unsafe {
                        let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                    }
                }
                continue;
            }
            if message.message == WM_NATIVE_MEDIA_ERROR {
                let decision = surface_lifecycle.fail(super::recovery::SurfaceFailureKind::Media);
                eprintln!(
                    "[NativeAnimated] MEDIA_FAILURE surface_id={surface_id} state={:?} attempts={} decision={decision:?}",
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
        unregister_lifecycle_notifications(hwnd, &mut lifecycle_notifications);
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
            release_animated_assignment_if_current(key);
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
    native_surface_closing().store(false, Ordering::Release);
    let active = native_surface()
        .lock()
        .expect("native surface mutex poisoned");
    if active.is_some() {
        return Err("Ya existe una superficie HWND nativa; ciérrala antes de abrir otra".into());
    }
    drop(active);
    if multi_surfaces()
        .lock()
        .expect("multi surface mutex poisoned")
        .len()
        >= super::multimonitor::MAX_ACTIVE_ANIMATED_SURFACES
    {
        return Err(
            "ResourceLimitExceeded: se alcanzó el límite global de superficies animadas".into(),
        );
    }
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
    monitor_id: MonitorIdentity,
    monitor: MonitorBounds,
    dpi: u32,
    desktop_host: desktop_host::RawDesktopHost,
) -> Result<(), String> {
    open_physical_surface_internal(app, monitor_id, monitor, dpi, desktop_host, true)
}

fn open_physical_surface_internal(
    app: &AppHandle,
    monitor_id: MonitorIdentity,
    monitor: MonitorBounds,
    dpi: u32,
    desktop_host: desktop_host::RawDesktopHost,
    remember_desired: bool,
) -> Result<(), String> {
    if multi_surfaces()
        .lock()
        .expect("multi surface mutex poisoned")
        .is_empty()
    {
        let mut runtime = desktop_lifecycle()
            .lock()
            .expect("desktop lifecycle mutex poisoned");
        if runtime.controller.state() == super::recovery::DesktopLifecycleState::Closed {
            runtime.controller = DesktopLifecycle::new();
            runtime.operation_active = false;
        }
    }
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
    if multi_surfaces()
        .lock()
        .expect("multi surface mutex poisoned")
        .len()
        >= super::multimonitor::MAX_ACTIVE_ANIMATED_SURFACES
    {
        return Err(
            "ResourceLimitExceeded: se alcanzÃ³ el lÃ­mite global de superficies animadas".into(),
        );
    }

    let assignment = wallpaper_coordinator::global()
        .lock()
        .map_err(|_| "No se pudo bloquear el coordinador de wallpapers".to_string())?
        .reserve(monitor_id.clone(), WallpaperBackend::Animated)
        .map_err(|error| error.to_string())?;

    let path = match prototype_directory(app) {
        Ok(directory) => directory.join(format!(
            "native-multimonitor-{}-{}.gif",
            sanitize_monitor_id(&monitor_id),
            uuid::Uuid::new_v4()
        )),
        Err(error) => {
            if let Ok(mut coordinator) = wallpaper_coordinator::global().lock() {
                coordinator.abort(&assignment);
            }
            return Err(error);
        }
    };
    if let Err(error) = create_diagnostic_gif(&path) {
        if let Ok(mut coordinator) = wallpaper_coordinator::global().lock() {
            coordinator.abort(&assignment);
        }
        return Err(error);
    }
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
            let commit_result = wallpaper_coordinator::global()
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
                        .commit(&assignment)
                        .map_err(|error| error.to_string())
                });
            if let Err(error) = commit_result {
                if let Ok(mut coordinator) = wallpaper_coordinator::global().lock() {
                    coordinator.abort(&assignment);
                }
                let _ = close_physical_surface(&monitor_id);
                return Err(error.to_string());
            }
            eprintln!(
                "[MultiMonitorPOC] RENDERER_CREATED monitor_id={} hwnd=0x{:x} host=0x{:x}",
                monitor_id, hwnd, desktop_host.1
            );
            start_host_watcher();
            Ok(())
        }
        Ok(Err(error)) => {
            if let Ok(mut coordinator) = wallpaper_coordinator::global().lock() {
                coordinator.abort(&assignment);
            }
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
            if let Ok(mut coordinator) = wallpaper_coordinator::global().lock() {
                coordinator.abort(&assignment);
            }
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

fn release_animated_assignment_if_current(monitor_id: &MonitorIdentity) {
    if let Ok(mut coordinator) = wallpaper_coordinator::global().lock() {
        let operation = coordinator.current(monitor_id).and_then(|assignment| {
            (assignment.backend == WallpaperBackend::Animated).then(|| AssignmentOperation {
                assignment: assignment.clone(),
            })
        });
        if let Some(operation) = operation {
            coordinator.release(&operation);
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

pub fn suspend_desired_surfaces_for_lifecycle() -> Result<(), String> {
    let ids = multi_surfaces()
        .lock()
        .expect("multi surface mutex poisoned")
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    let count = ids.len();
    for id in ids {
        stop_physical_surface(&id)?;
    }
    eprintln!("[LifecyclePOC] SURFACES_SUSPENDED count={count}");
    Ok(())
}

pub fn recover_desired_surfaces_for_lifecycle() -> bool {
    recover_desired_surfaces()
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
        let current_surface = desired_surfaces()
            .lock()
            .expect("desired surfaces mutex poisoned")
            .get(&surface.monitor_id)
            .cloned();
        let Some(surface) = current_surface else {
            eprintln!(
                "[HostMonitor] SURFACE_RECOVERY_SKIPPED monitor_id={} reason=explicit_close",
                surface.monitor_id
            );
            continue;
        };
        let geometry = super::resolve_monitor_for_identity(&surface.app, &surface.monitor_id);
        let (bounds, dpi) = match geometry {
            Ok(value) => value,
            Err(error) => {
                eprintln!(
                    "[HostMonitor] SURFACE_RECOVERY_SKIPPED monitor_id={} reason=monitor_unavailable error={error}",
                    surface.monitor_id
                );
                continue;
            }
        };
        match open_physical_surface_internal(
            &surface.app,
            surface.monitor_id.clone(),
            bounds,
            dpi,
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

fn sanitize_monitor_id(value: &MonitorIdentity) -> String {
    value
        .as_str()
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

pub fn close_physical_surface(monitor_id: &MonitorIdentity) -> Result<(), String> {
    desired_surfaces()
        .lock()
        .expect("desired surfaces mutex poisoned")
        .remove(monitor_id);
    let last_surface = multi_surfaces()
        .lock()
        .expect("multi surface mutex poisoned")
        .len()
        <= 1;
    let no_desired = desired_surfaces()
        .lock()
        .expect("desired surfaces mutex poisoned")
        .is_empty();
    if last_surface && no_desired {
        let _ = desktop_lifecycle()
            .lock()
            .expect("desktop lifecycle mutex poisoned")
            .controller
            .event(DesktopLifecycleEvent::Close);
    }
    let result = stop_physical_surface(monitor_id);
    if desired_surfaces()
        .lock()
        .expect("desired surfaces mutex poisoned")
        .is_empty()
    {
        stop_host_watcher();
    }
    let no_surfaces = multi_surfaces()
        .lock()
        .expect("multi surface mutex poisoned")
        .is_empty();
    let no_desired = desired_surfaces()
        .lock()
        .expect("desired surfaces mutex poisoned")
        .is_empty();
    if no_surfaces && no_desired {
        desktop_lifecycle()
            .lock()
            .expect("desktop lifecycle mutex poisoned")
            .controller
            .mark_closed();
    }
    result
}

fn stop_physical_surface(monitor_id: &MonitorIdentity) -> Result<(), String> {
    mark_surface_closing(monitor_id);
    let hwnd = multi_surfaces()
        .lock()
        .expect("multi surface mutex poisoned")
        .get(monitor_id)
        .copied();
    if let Some(hwnd) = hwnd {
        unsafe {
            if let Err(error) =
                PostMessageW(Some(HWND(hwnd as *mut _)), WM_CLOSE, WPARAM(0), LPARAM(0))
            {
                clear_surface_closing(monitor_id);
                return Err(format!("No se pudo cerrar {monitor_id}: {error}"));
            }
        }
    }
    if let Some(thread) = multi_threads()
        .lock()
        .expect("multi thread mutex poisoned")
        .remove(monitor_id)
    {
        let result = thread
            .join()
            .map_err(|_| format!("El hilo de {monitor_id} terminó inesperadamente"));
        clear_surface_closing(monitor_id);
        result?;
    }
    clear_surface_closing(monitor_id);
    Ok(())
}

pub fn close_all_physical_surfaces() {
    desired_surfaces()
        .lock()
        .expect("desired surfaces mutex poisoned")
        .clear();
    stop_host_watcher();
    let ids: Vec<MonitorIdentity> = multi_surfaces()
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
    native_surface_closing().store(true, Ordering::Release);
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
        create_diagnostic_gif, is_native_media_error_payload, validate_native_media,
        NotificationRegistration, TemporaryClassification,
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
    fn notification_registration_is_single_use_and_unregistration_is_idempotent() {
        let mut registration = NotificationRegistration::default();
        assert!(registration.mark_registered().is_ok());
        assert!(registration.mark_registered().is_err());
        assert!(registration.take_registered());
        assert!(!registration.take_registered());
    }

    #[test]
    fn recreated_surface_gets_a_new_logical_notification_registration() {
        let mut first = NotificationRegistration::default();
        first.mark_registered().unwrap();
        assert!(first.take_registered());

        let mut second = NotificationRegistration::default();
        assert!(second.mark_registered().is_ok());
        assert_ne!(first, second);
        assert!(second.take_registered());
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
        assert!(validate_native_media(
            &missing,
            super::MonitorBounds {
                x: 0,
                y: 0,
                width: 2560,
                height: 1440
            }
        )
        .unwrap_err()
        .contains("MediaError"));

        let empty = directory.join("empty.gif");
        fs::write(&empty, []).unwrap();
        assert!(validate_native_media(
            &empty,
            super::MonitorBounds {
                x: 0,
                y: 0,
                width: 2560,
                height: 1440
            }
        )
        .unwrap_err()
        .contains("MediaError"));

        let wrong_format = directory.join("wrong.txt");
        fs::write(&wrong_format, b"not media").unwrap();
        assert!(validate_native_media(
            &wrong_format,
            super::MonitorBounds {
                x: 0,
                y: 0,
                width: 2560,
                height: 1440
            }
        )
        .unwrap_err()
        .contains("MediaError"));

        let large = directory.join("large.gif");
        let file = fs::File::create(&large).unwrap();
        file.set_len(super::MAX_NATIVE_MEDIA_BYTES + 1).unwrap();
        assert!(validate_native_media(
            &large,
            super::MonitorBounds {
                x: 0,
                y: 0,
                width: 2560,
                height: 1440
            }
        )
        .unwrap_err()
        .contains("límite"));

        let valid = directory.join("valid.gif");
        create_diagnostic_gif(&valid).unwrap();
        assert!(validate_native_media(
            &valid,
            super::MonitorBounds {
                x: 0,
                y: 0,
                width: 2560,
                height: 1440
            }
        )
        .is_ok());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn native_media_error_payload_requires_exact_message() {
        assert!(is_native_media_error_payload("\"lumina-media-error\""));
        assert!(!is_native_media_error_payload("\"other-message\""));
        assert!(!is_native_media_error_payload("not-json"));
        assert!(!is_native_media_error_payload(
            "{\"message\":\"lumina-media-error\"}"
        ));
    }
}
