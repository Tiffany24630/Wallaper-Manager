use super::MonitorBounds;
use serde::Serialize;
use tauri::WebviewWindow;
use windows::{
    core::w,
    Win32::{
        Foundation::{BOOL, HWND, LPARAM, RECT, WPARAM},
        Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
        },
        UI::WindowsAndMessaging::{
            EnumChildWindows, EnumWindows, FindWindowExW, FindWindowW, GetClassNameW, GetParent,
            GetWindow, GetWindowLongPtrW, GetWindowRect, IsWindowVisible, SendMessageTimeoutW,
            SetParent, SetWindowLongPtrW, SetWindowPos, GWL_STYLE, GW_OWNER, HWND_TOP,
            SMTO_ABORTIFHUNG, SWP_NOACTIVATE, SWP_SHOWWINDOW, WS_CHILD,
        },
    },
};

const DESKTOP_CREATE_WORKER_W_MESSAGE: u32 = 0x052C;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DesktopHost {
    progman: HWND,
    worker_w: HWND,
    shell_view: HWND,
}

#[derive(Debug, Clone, Serialize)]
pub struct DesktopRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct DesktopMonitor {
    pub handle: u64,
    pub bounds: DesktopRect,
    pub work_area: DesktopRect,
    pub primary: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DesktopWindowSnapshot {
    pub hwnd: u64,
    pub class_name: String,
    pub parent: u64,
    pub owner: u64,
    pub visible: bool,
    pub rect: Option<DesktopRect>,
    pub monitor: Option<DesktopMonitor>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DesktopStructureInspection {
    pub progman: u64,
    pub message: u32,
    pub message_result: u64,
    pub before: Vec<DesktopWindowSnapshot>,
    pub after: Vec<DesktopWindowSnapshot>,
}

unsafe extern "system" fn find_desktop_worker_w(window: HWND, state: LPARAM) -> BOOL {
    let shell_view = FindWindowExW(window, None, w!("SHELLDLL_DefView"), None).unwrap_or_default();
    if shell_view == HWND::default() {
        return BOOL(1);
    }

    let worker_w = FindWindowExW(HWND::default(), window, w!("WorkerW"), None).unwrap_or_default();
    if worker_w != HWND::default() {
        *(state.0 as *mut Option<HWND>) = Some(worker_w);
        return BOOL(0);
    }

    BOOL(1)
}

fn class_name(window: HWND) -> String {
    let mut buffer = [0u16; 64];
    let length = unsafe { GetClassNameW(window, &mut buffer) };
    String::from_utf16_lossy(&buffer[..length.max(0) as usize])
}

fn hwnd_value(window: HWND) -> u64 {
    window.0 as usize as u64
}

fn rect_value(rect: RECT) -> DesktopRect {
    DesktopRect {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
    }
}

fn monitor_value(window: HWND) -> Option<DesktopMonitor> {
    let monitor = unsafe { MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST) };
    if monitor.0.is_null() {
        return None;
    }
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let success = unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool();
    success.then(|| DesktopMonitor {
        handle: monitor.0 as usize as u64,
        bounds: rect_value(info.rcMonitor),
        work_area: rect_value(info.rcWork),
        primary: info.dwFlags != 0,
    })
}

fn snapshot_window(window: HWND) -> DesktopWindowSnapshot {
    let parent = unsafe { GetParent(window).unwrap_or_default() };
    let owner = unsafe { GetWindow(window, GW_OWNER).unwrap_or_default() };
    let mut rect = RECT::default();
    let rect = unsafe {
        GetWindowRect(window, &mut rect)
            .is_ok()
            .then_some(rect_value(rect))
    };
    DesktopWindowSnapshot {
        hwnd: hwnd_value(window),
        class_name: class_name(window),
        parent: hwnd_value(parent),
        owner: hwnd_value(owner),
        visible: unsafe { IsWindowVisible(window).as_bool() },
        rect,
        monitor: monitor_value(window),
    }
}

fn is_relevant_class(name: &str) -> bool {
    matches!(
        name,
        "Progman" | "WorkerW" | "SHELLDLL_DefView" | "SysListView32"
    )
}

unsafe extern "system" fn collect_desktop_child(window: HWND, state: LPARAM) -> BOOL {
    let name = class_name(window);
    if is_relevant_class(&name) {
        (*(state.0 as *mut Vec<DesktopWindowSnapshot>)).push(snapshot_window(window));
    }
    BOOL(1)
}

unsafe extern "system" fn collect_desktop_window(window: HWND, state: LPARAM) -> BOOL {
    let name = class_name(window);
    if is_relevant_class(&name) {
        (*(state.0 as *mut Vec<DesktopWindowSnapshot>)).push(snapshot_window(window));
        let _ = EnumChildWindows(window, Some(collect_desktop_child), state);
    }
    BOOL(1)
}

fn inspect_windows() -> Result<Vec<DesktopWindowSnapshot>, String> {
    let mut windows = Vec::new();
    unsafe {
        EnumWindows(
            Some(collect_desktop_window),
            LPARAM((&mut windows as *mut Vec<DesktopWindowSnapshot>) as isize),
        )
        .map_err(|error| format!("No se pudo enumerar la estructura del escritorio: {error}"))?;
    }
    Ok(windows)
}

pub fn inspect_desktop_structure() -> Result<DesktopStructureInspection, String> {
    let progman = unsafe { FindWindowW(w!("Progman"), None) }
        .map_err(|error| format!("No se pudo localizar Progman: {error}"))?;
    if progman == HWND::default() {
        return Err("Progman no está disponible".into());
    }
    let before = inspect_windows()?;
    let mut message_result = 0usize;
    let _ = unsafe {
        SendMessageTimeoutW(
            progman,
            DESKTOP_CREATE_WORKER_W_MESSAGE,
            WPARAM(0),
            LPARAM(0),
            SMTO_ABORTIFHUNG,
            1000,
            Some(&mut message_result),
        )
    };
    let after = inspect_windows()?;
    Ok(DesktopStructureInspection {
        progman: hwnd_value(progman),
        message: DESKTOP_CREATE_WORKER_W_MESSAGE,
        message_result: message_result as u64,
        before,
        after,
    })
}

fn locate_desktop_host() -> Result<DesktopHost, String> {
    let progman = unsafe { FindWindowW(w!("Progman"), None) }
        .map_err(|error| format!("No se pudo localizar Progman: {error}"))?;
    if progman == HWND::default() {
        return Err("Progman no está disponible".into());
    }

    let mut result = 0usize;
    let _ = unsafe {
        SendMessageTimeoutW(
            progman,
            DESKTOP_CREATE_WORKER_W_MESSAGE,
            WPARAM(0),
            LPARAM(0),
            SMTO_ABORTIFHUNG,
            1000,
            Some(&mut result),
        )
    };

    let mut worker_w = None;
    unsafe {
        EnumWindows(
            Some(find_desktop_worker_w),
            LPARAM((&mut worker_w as *mut Option<HWND>) as isize),
        )
        .map_err(|error| format!("No se pudo enumerar las ventanas del escritorio: {error}"))?;
    }
    let worker_w = worker_w.ok_or("No se encontró un WorkerW detrás de SHELLDLL_DefView")?;
    let shell_view =
        unsafe { FindWindowExW(worker_w, None, w!("SHELLDLL_DefView"), None).unwrap_or_default() };

    eprintln!(
        "Animated WorkerW host: Progman={:?}, WorkerW={:?}, SHELLDLL_DefView={:?}, classes=({}, {}, {})",
        progman,
        worker_w,
        shell_view,
        class_name(progman),
        class_name(worker_w),
        class_name(shell_view),
    );
    Ok(DesktopHost {
        progman,
        worker_w,
        shell_view,
    })
}

pub fn attach_to_worker_w(window: &WebviewWindow, bounds: MonitorBounds) -> Result<(), String> {
    let host = locate_desktop_host()?;
    let tauri_hwnd = window
        .hwnd()
        .map_err(|error| format!("No se pudo obtener el HWND del prototipo: {error}"))?;
    let hwnd = HWND(tauri_hwnd.0);
    let mut host_rect = RECT::default();
    unsafe {
        GetWindowRect(host.worker_w, &mut host_rect)
            .map_err(|error| format!("No se pudo obtener la geometría de WorkerW: {error}"))?;
        SetParent(hwnd, host.worker_w)
            .map_err(|error| format!("No se pudo asociar el prototipo a WorkerW: {error}"))?;
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        SetWindowLongPtrW(hwnd, GWL_STYLE, (style | WS_CHILD.0) as isize);
        SetWindowPos(
            hwnd,
            HWND_TOP,
            bounds.x - host_rect.left,
            bounds.y - host_rect.top,
            bounds.width as i32,
            bounds.height as i32,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )
        .map_err(|error| {
            format!("No se pudo posicionar el prototipo dentro de WorkerW: {error}")
        })?;
    }
    eprintln!(
        "Animated WorkerW attached: window={:?}, parent={:?}, monitor_bounds=({}, {}, {}, {})",
        hwnd, host.worker_w, bounds.x, bounds.y, bounds.width, bounds.height
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn workerw_protocol_is_explicitly_scoped() {
        assert_eq!(super::DESKTOP_CREATE_WORKER_W_MESSAGE, 0x052C);
    }
}
