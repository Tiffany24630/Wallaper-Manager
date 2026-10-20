use super::MonitorBounds;
use serde::Serialize;
use tauri::WebviewWindow;
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::{BOOL, HWND, LPARAM, RECT, WPARAM},
        Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
        },
        UI::WindowsAndMessaging::{
            EnumChildWindows, EnumWindows, FindWindowExW, FindWindowW, GetClassNameW, GetParent,
            GetWindow, GetWindowLongPtrW, GetWindowRect, IsWindowVisible, SendMessageTimeoutW,
            SetParent, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, GWL_STYLE, GW_OWNER, HWND_TOP,
            SMTO_ABORTIFHUNG, SWP_NOACTIVATE, SWP_SHOWWINDOW, WS_CHILD, WS_POPUP,
        },
    },
};

const DESKTOP_CREATE_WORKER_W_MESSAGE: u32 = 0x052C;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DesktopHost {
    pub(crate) progman: HWND,
    pub(crate) worker_w: HWND,
    pub(crate) shell_view: HWND,
}

pub(crate) type RawDesktopHost = (isize, isize, isize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HostValidation {
    Stable,
    Changed(String),
    Transient(String),
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
    pub style: i64,
    pub exstyle: i64,
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

#[derive(Debug, Clone, Serialize)]
pub struct DesktopHostExperiment {
    pub message: u32,
    pub wparam: u64,
    pub lparam: i64,
    pub message_result: u64,
    pub before: Vec<DesktopWindowSnapshot>,
    pub after: Vec<DesktopWindowSnapshot>,
    pub candidate_worker_w: Option<u64>,
    pub candidate_shell_view: Option<u64>,
    pub candidate_count: usize,
    pub host_is_unambiguous: bool,
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
        style: unsafe { GetWindowLongPtrW(window, GWL_STYLE) as i64 },
        exstyle: unsafe { GetWindowLongPtrW(window, GWL_EXSTYLE) as i64 },
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

fn direct_child(parent: HWND, class: PCWSTR) -> Option<HWND> {
    unsafe { FindWindowExW(parent, None, class, None).ok() }
}

fn direct_worker_candidates(progman: HWND) -> Vec<HWND> {
    let mut result = Vec::new();
    let mut after = None;
    while let Ok(worker) = unsafe { FindWindowExW(progman, after.as_ref(), w!("WorkerW"), None) } {
        result.push(worker);
        after = Some(worker);
    }
    result
}

fn find_progman() -> Result<HWND, String> {
    let window = unsafe { FindWindowW(w!("Progman"), None) }
        .map_err(|error| format!("No se pudo localizar Progman: {error}"))?;
    if window == HWND::default() {
        return Err("Progman no está disponible".into());
    }
    Ok(window)
}

fn modern_host_from_current_tree(progman: HWND) -> Option<DesktopHost> {
    let shell_view = direct_child(progman, w!("SHELLDLL_DefView"))?;
    let workers = direct_worker_candidates(progman)
        .into_iter()
        .filter(|worker| direct_child(*worker, w!("SHELLDLL_DefView")).is_none())
        .collect::<Vec<_>>();
    (workers.len() == 1).then(|| DesktopHost {
        progman,
        worker_w: workers[0],
        shell_view,
    })
}

pub fn prepare_modern_host() -> Result<(RawDesktopHost, DesktopHostExperiment), String> {
    let progman = find_progman()?;
    let before = inspect_windows()?;
    let mut message_result = 0usize;
    unsafe {
        SendMessageTimeoutW(
            progman,
            DESKTOP_CREATE_WORKER_W_MESSAGE,
            WPARAM(0xD),
            LPARAM(1),
            SMTO_ABORTIFHUNG,
            1000,
            Some(&mut message_result),
        );
    }
    let after = inspect_windows()?;
    let host = modern_host_from_current_tree(progman);
    let candidate_count = direct_worker_candidates(progman)
        .into_iter()
        .filter(|worker| direct_child(*worker, w!("SHELLDLL_DefView")).is_none())
        .count();
    let experiment = DesktopHostExperiment {
        message: DESKTOP_CREATE_WORKER_W_MESSAGE,
        wparam: 0xD,
        lparam: 1,
        message_result: message_result as u64,
        before,
        after,
        candidate_worker_w: host.map(|value| hwnd_value(value.worker_w)),
        candidate_shell_view: host.map(|value| hwnd_value(value.shell_view)),
        candidate_count,
        host_is_unambiguous: host.is_some(),
    };
    let host = host.ok_or_else(|| {
        "La variante moderna no produjo exactamente un WorkerW hijo directo de Progman sin SHELLDLL_DefView; no se hará SetParent".to_string()
    })?;
    eprintln!(
        "Animated desktop POC host validated: Progman={:?}, WorkerW={:?}, SHELLDLL_DefView={:?}",
        host.progman, host.worker_w, host.shell_view
    );
    Ok((
        (
            host.progman.0 as isize,
            host.worker_w.0 as isize,
            host.shell_view.0 as isize,
        ),
        experiment,
    ))
}

pub(crate) fn validate_registered_host(registered: RawDesktopHost) -> HostValidation {
    let progman = match find_progman() {
        Ok(value) => value,
        Err(error) => return HostValidation::Transient(error),
    };
    if progman.0 as isize != registered.0 {
        return HostValidation::Changed(format!(
            "Progman cambió: registrado=0x{:x}, actual=0x{:x}",
            registered.0, progman.0 as usize
        ));
    }

    let Some(shell_view) = direct_child(progman, w!("SHELLDLL_DefView")) else {
        return HostValidation::Changed("SHELLDLL_DefView ya no es hijo directo de Progman".into());
    };
    let candidates = direct_worker_candidates(progman)
        .into_iter()
        .filter(|worker| direct_child(*worker, w!("SHELLDLL_DefView")).is_none())
        .collect::<Vec<_>>();
    if candidates.len() != 1 {
        return HostValidation::Changed(format!(
            "la jerarquía tiene {} WorkerW candidatos válidos",
            candidates.len()
        ));
    }
    let worker = candidates[0];
    if shell_view.0 as isize != registered.2 || worker.0 as isize != registered.1 {
        return HostValidation::Changed(format!(
            "cambió la jerarquía: worker=0x{:x}, shell_view=0x{:x}",
            worker.0 as usize, shell_view.0 as usize
        ));
    }
    HostValidation::Stable
}

pub(crate) fn snapshot_current_desktop() -> Result<Vec<DesktopWindowSnapshot>, String> {
    inspect_windows()
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

pub fn attach_native_window(
    window: isize,
    host: RawDesktopHost,
    bounds: MonitorBounds,
) -> Result<(), String> {
    let window = HWND(window as *mut _);
    let host = DesktopHost {
        progman: HWND(host.0 as *mut _),
        worker_w: HWND(host.1 as *mut _),
        shell_view: HWND(host.2 as *mut _),
    };
    let mut host_rect = RECT::default();
    unsafe {
        GetWindowRect(host.progman, &mut host_rect)
            .map_err(|error| format!("No se pudo obtener la geometría de Progman: {error}"))?;
        SetParent(window, host.progman)
            .map_err(|error| format!("No se pudo asociar el renderer al host validado: {error}"))?;
        let style = GetWindowLongPtrW(window, GWL_STYLE) as u32;
        SetWindowLongPtrW(
            window,
            GWL_STYLE,
            ((style & !WS_POPUP.0) | WS_CHILD.0) as isize,
        );
        if let Err(error) = SetWindowPos(
            window,
            host.shell_view,
            bounds.x - host_rect.left,
            bounds.y - host_rect.top,
            bounds.width as i32,
            bounds.height as i32,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        ) {
            let _ = SetParent(window, None);
            let _ = SetWindowLongPtrW(
                window,
                GWL_STYLE,
                ((style & !WS_CHILD.0) | WS_POPUP.0) as isize,
            );
            return Err(format!(
                "No se pudo colocar el renderer bajo SHELLDLL_DefView: {error}"
            ));
        }
    }
    eprintln!(
        "Animated desktop POC attached: renderer={:?}, parent=Progman {:?}, below=SHELLDLL_DefView {:?}, WorkerW {:?}, bounds=({}, {}, {}, {})",
        window,
        host.progman,
        host.shell_view,
        host.worker_w,
        bounds.x,
        bounds.y,
        bounds.width,
        bounds.height
    );
    Ok(())
}

pub fn detach_native_window(window: isize) -> Result<(), String> {
    let window = HWND(window as *mut _);
    unsafe {
        SetParent(window, None).map_err(|error| {
            format!("No se pudo separar el renderer del host del escritorio: {error}")
        })?;
        let style = GetWindowLongPtrW(window, GWL_STYLE) as u32;
        SetWindowLongPtrW(
            window,
            GWL_STYLE,
            ((style & !WS_CHILD.0) | WS_POPUP.0) as isize,
        );
    }
    Ok(())
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
