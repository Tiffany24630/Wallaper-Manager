use super::{create_diagnostic_gif, prototype_directory, MonitorBounds};
use std::{
    path::Path,
    sync::{mpsc, Mutex, OnceLock},
    thread,
    time::Duration,
};

use tauri::AppHandle;
use webview2_com::{
    CoreWebView2EnvironmentOptions, CreateCoreWebView2ControllerCompletedHandler,
    CreateCoreWebView2EnvironmentCompletedHandler, Microsoft::Web::WebView2::Win32::*,
    NavigationCompletedEventHandler,
};
use windows_webview2::{
    core::{w, HSTRING, Interface, PCWSTR},
    Win32::{
        Foundation::{E_POINTER, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
        Graphics::Gdi::UpdateWindow,
        System::{
            Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED},
            LibraryLoader::GetModuleHandleW,
        },
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
            PostMessageW, PostQuitMessage, RegisterClassW, ShowWindow, TranslateMessage,
            CS_HREDRAW, CS_VREDRAW, MSG, SW_SHOW, WM_CLOSE, WM_DESTROY, WM_NCDESTROY, WM_QUIT,
            WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
        },
    },
};

const NATIVE_WINDOW_CLASS: PCWSTR = w!("LuminaNativeAnimatedPrototype");
const MAX_NATIVE_SURFACES: usize = 1;

static NATIVE_SURFACE: OnceLock<Mutex<Option<isize>>> = OnceLock::new();

fn native_surface() -> &'static Mutex<Option<isize>> {
    NATIVE_SURFACE.get_or_init(|| Mutex::new(None))
}

unsafe extern "system" fn native_window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_CLOSE => {
            let _ = DestroyWindow(window);
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
        return Err("RegisterClassW no pudo registrar la clase nativa".into());
    }
    Ok(instance.into())
}

fn create_native_window(bounds: MonitorBounds) -> Result<HWND, String> {
    let instance = register_native_window_class()?;
    let window = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
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
    let uri = html_escape(&format!("https://lumina.local/{}", encode_uri_component(filename)));
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><style>
        html,body{{margin:0;width:100%;height:100%;overflow:hidden;background:#111827}}
        body{{display:grid;place-items:center;font-family:Segoe UI, sans-serif}}
        img{{width:100%;height:100%;object-fit:cover;display:block}}
        #status{{position:fixed;top:18px;left:18px;padding:10px 14px;border-radius:10px;
          color:#fff;background:rgba(15,23,42,.9);font-size:16px;z-index:2}}
        </style></head><body>
        <div id="status">Native HWND WebView2 · cargando GIF…</div>
        <img src="{uri}" alt="GIF de diagnóstico" onload="status.textContent='Native HWND WebView2 · GIF cargado'" onerror="status.textContent='Native HWND WebView2 · error de carga'">
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
        controller
            .CoreWebView2()
            .map_err(|error| format!("No se pudo obtener ICoreWebView2: {error}"))?
    };
    let webview3 = webview
        .cast::<ICoreWebView2_3>()
        .map_err(|error| format!("WebView2 no expone el mapeo de host local: {error}"))?;
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
            .map_err(|error| format!("No se pudo mapear el medio local en WebView2: {error}"))?;
    }
    let rect = windows_webview2::Win32::Foundation::RECT {
        left: 0,
        top: 0,
        right: bounds.width as i32,
        bottom: bounds.height as i32,
    };
    unsafe {
        controller
            .SetBounds(rect)
            .map_err(|error| format!("No se pudo ajustar el tamaño de WebView2: {error}"))?;
        controller
            .SetIsVisible(true)
            .map_err(|error| format!("No se pudo mostrar WebView2: {error}"))?;
        let navigation_handler = NavigationCompletedEventHandler::create(Box::new(|_, _| {
            eprintln!("[NativeAnimated] URL cargada");
            Ok(())
        }));
        let mut token = 0i64;
        webview
            .add_NavigationCompleted(&navigation_handler, &mut token)
            .map_err(|error| format!("No se pudo registrar NavigationCompleted: {error}"))?;
        let html = HSTRING::from(native_html(media_path));
        webview
            .NavigateToString(&html)
            .map_err(|error| format!("No se pudo cargar el HTML del GIF: {error}"))?;
    }
    eprintln!("[NativeAnimated] GIF cargado en la ruta local del WebView2");
    Ok(controller)
}

fn run_native_surface(
    bounds: MonitorBounds,
    media_path: std::path::PathBuf,
    ready: mpsc::Sender<Result<isize, String>>,
) {
    let com_initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok() };
    let result = (|| {
        if !com_initialized {
            return Err("CoInitializeEx no pudo inicializar COM".into());
        }
        let hwnd = create_native_window(bounds)?;
        let controller = match create_webview(hwnd, bounds, &media_path) {
            Ok(controller) => controller,
            Err(error) => {
                unsafe {
                    DestroyWindow(hwnd).ok();
                }
                return Err(error);
            }
        };
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = UpdateWindow(hwnd);
        }
        *native_surface()
            .lock()
            .expect("native surface mutex poisoned") = Some(hwnd.0 as isize);
        eprintln!("[NativeAnimated] ventana mostrada");
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
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        eprintln!("[NativeAnimated] cierre solicitado");
        unsafe {
            controller.Close().ok();
        }
        eprintln!("[NativeAnimated] WebView2 destruido");
        unsafe {
            DestroyWindow(hwnd).ok();
        }
        eprintln!("[NativeAnimated] HWND destruido");
        *native_surface()
            .lock()
            .expect("native surface mutex poisoned") = None;
        let _ = std::fs::remove_file(&media_path);
        eprintln!("[NativeAnimated] temporal eliminado");
        Ok(())
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

pub fn open(app: &AppHandle, monitor: MonitorBounds) -> Result<(), String> {
    let active = native_surface()
        .lock()
        .expect("native surface mutex poisoned");
    if active.is_some() {
        return Err("Ya existe una superficie HWND nativa; ciérrala antes de abrir otra".into());
    }
    let path =
        prototype_directory(app)?.join(format!("native-diagnostic-{}.gif", uuid::Uuid::new_v4()));
    create_diagnostic_gif(&path)?;
    if MAX_NATIVE_SURFACES != 1 {
        return Err("Configuración inválida del POC nativo".into());
    }
    let (ready_tx, ready_rx) = mpsc::channel();
    let thread_path = path.clone();
    thread::spawn(move || run_native_surface(monitor, thread_path, ready_tx));
    drop(active);
    match ready_rx.recv_timeout(Duration::from_secs(15)) {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(error)) => {
            let _ = std::fs::remove_file(path);
            Err(error)
        }
        Err(_) => {
            let _ = std::fs::remove_file(path);
            Err(
                "El renderer nativo no confirmó su inicialización dentro del tiempo esperado"
                    .into(),
            )
        }
    }
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
    Ok(())
}
