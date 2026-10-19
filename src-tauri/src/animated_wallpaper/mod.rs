use std::{
    fs,
    path::{Path, PathBuf},
};

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use uuid::Uuid;

const MAX_PROTOTYPE_MEDIA_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MonitorBounds {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
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
    let mut builder = WebviewWindowBuilder::new(&app, &label, url)
        .title("Lumina animated wallpaper prototype")
        .decorations(diagnostic)
        .resizable(false)
        .focused(diagnostic)
        .focusable(diagnostic)
        .always_on_top(diagnostic)
        .position(f64::from(bounds.x), f64::from(bounds.y))
        .inner_size(f64::from(bounds.width), f64::from(bounds.height))
        .visible(!diagnostic)
        .on_navigation(|url| {
            url.scheme() == "tauri"
                || (url.scheme() == "http" && url.host_str() == Some("tauri.localhost"))
                || (cfg!(dev)
                    && url.scheme() == "http"
                    && matches!(url.host_str(), Some("127.0.0.1") | Some("localhost")))
        });
    if !diagnostic {
        builder = builder.skip_taskbar(true).always_on_bottom(true);
    }
    let window = builder.build().map_err(|e| {
        let _ = fs::remove_file(&cleanup_path);
        format!("No se pudo crear la superficie WebView2: {e}")
    })?;

    if let Err(error) = window.set_ignore_cursor_events(!diagnostic) {
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
    Ok(())
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
    for (label, window) in app.webview_windows() {
        if label.starts_with("animated-prototype-") {
            let _ = window.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{bounds_from_geometry, encode_query_component};

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
}
