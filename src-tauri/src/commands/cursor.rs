use chrono::Utc;
use image::{imageops::FilterType, DynamicImage, GenericImageView, Rgba, RgbaImage};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};
use tauri::State;
use uuid::Uuid;

use crate::{
    db::{app_data_dir, get_connection},
    dynamic::DynamicScheduler,
    models::cursor::CursorResource,
    utils::thumbnail::generate_thumbnail,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CursorKind {
    Static,
    Animated,
}

impl CursorKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Animated => "animated",
        }
    }
}

fn detect_kind(path: &Path) -> Result<CursorKind, String> {
    match path.extension().and_then(|value| value.to_str()).map(|value| value.to_ascii_lowercase()).as_deref() {
        Some("cur") => Ok(CursorKind::Static),
        Some("ani") => Ok(CursorKind::Animated),
        Some("png") | Some("jpg") | Some("jpeg") | Some("webp") | Some("bmp") => Ok(CursorKind::Static),
        Some("gif") => Err("Las animaciones GIF no se pueden convertir automáticamente en un cursor de Windows; usa un archivo .ani".into()),
        _ => Err("Formato no compatible. Usa CUR, ANI, PNG, JPG, WEBP o BMP".into()),
    }
}

fn validate_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("El nombre del cursor no puede estar vacío".into());
    }
    if name.chars().any(char::is_control) {
        return Err("El nombre contiene caracteres de control no válidos".into());
    }
    Ok(name.to_owned())
}

fn validate_image_dimensions(image: &DynamicImage) -> Result<(), String> {
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 || width > 256 || height > 256 {
        return Err("Una imagen de cursor debe medir entre 1 y 256 píxeles por lado".into());
    }
    Ok(())
}

fn push_u16(buffer: &mut Vec<u8>, value: u16) {
    buffer.extend_from_slice(&value.to_le_bytes());
}
fn push_u32(buffer: &mut Vec<u8>, value: u32) {
    buffer.extend_from_slice(&value.to_le_bytes());
}
fn push_i32(buffer: &mut Vec<u8>, value: i32) {
    buffer.extend_from_slice(&value.to_le_bytes());
}

fn encode_cur(image: &DynamicImage, hotspot_x: u16, hotspot_y: u16) -> Result<Vec<u8>, String> {
    validate_image_dimensions(image)?;
    let (width, height) = image.dimensions();
    if hotspot_x >= width as u16 || hotspot_y >= height as u16 {
        return Err("El punto caliente debe estar dentro de la imagen".into());
    }
    let rgba = image.to_rgba8();
    let row_stride = width * 4;
    let image_size = row_stride * height;
    let mask_stride = ((width + 31) / 32) * 4;
    let mask_size = mask_stride * height;
    let bytes_in_res = 40 + image_size + mask_size;
    let mut out = Vec::with_capacity(22 + bytes_in_res as usize);
    push_u16(&mut out, 0);
    push_u16(&mut out, 2);
    push_u16(&mut out, 1);
    out.push(if width >= 256 { 0 } else { width as u8 });
    out.push(if height >= 256 { 0 } else { height as u8 });
    out.push(0);
    out.push(0);
    push_u16(&mut out, hotspot_x);
    push_u16(&mut out, hotspot_y);
    push_u32(&mut out, bytes_in_res);
    push_u32(&mut out, 22);
    push_u32(&mut out, 40);
    push_i32(&mut out, width as i32);
    push_i32(&mut out, (height * 2) as i32);
    push_u16(&mut out, 1);
    push_u16(&mut out, 32);
    push_u32(&mut out, 0);
    push_u32(&mut out, image_size);
    push_i32(&mut out, 0);
    push_i32(&mut out, 0);
    push_u32(&mut out, 0);
    push_u32(&mut out, 0);
    for y in (0..height).rev() {
        for x in 0..width {
            let pixel = rgba.get_pixel(x, y);
            out.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
        }
    }
    for y in (0..height).rev() {
        let row_start = out.len();
        out.resize(row_start + mask_stride as usize, 0);
        for x in 0..width {
            if rgba.get_pixel(x, y)[3] < 128 {
                out[row_start + (x / 8) as usize] |= 1 << (7 - (x % 8));
            }
        }
    }
    Ok(out)
}

fn validate_cursor_file(path: &Path, kind: CursorKind) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|e| format!("No se pudo leer el cursor: {e}"))?;
    if bytes.is_empty() {
        return Err("El archivo del cursor está vacío".into());
    }
    match kind {
        CursorKind::Static => {
            if bytes.len() < 22
                || u16::from_le_bytes([bytes[2], bytes[3]]) != 2
                || u16::from_le_bytes([bytes[4], bytes[5]]) == 0
            {
                return Err("El archivo CUR no tiene una estructura válida".into());
            }
            let width = if bytes[6] == 0 { 256 } else { bytes[6] as u32 };
            let height = if bytes[7] == 0 { 256 } else { bytes[7] as u32 };
            if width == 0 || height == 0 || width > 256 || height > 256 {
                return Err("El CUR supera las dimensiones permitidas".into());
            }
        }
        CursorKind::Animated => {
            if bytes.len() < 12
                || &bytes[0..4] != b"RIFF"
                || &bytes[8..12] != b"ACON"
                || !bytes.windows(4).any(|chunk| chunk == b"anih")
            {
                return Err("El archivo ANI no tiene una cabecera RIFF/ACON válida".into());
            }
        }
    }
    Ok(())
}

fn decode_cur_image(path: &Path) -> Result<DynamicImage, String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    if bytes.len() < 22 {
        return Err("El CUR no contiene una imagen".into());
    }
    let offset = u32::from_le_bytes(bytes[18..22].try_into().unwrap()) as usize;
    if offset >= bytes.len() {
        return Err("El CUR apunta fuera del archivo".into());
    }
    if bytes[offset..].starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]) {
        return image::load_from_memory(&bytes[offset..]).map_err(|e| e.to_string());
    }
    if bytes.len() < offset + 40 {
        return Err("El CUR no contiene un bitmap válido".into());
    }
    let width =
        i32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()).unsigned_abs();
    let dib_height =
        i32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().unwrap()).unsigned_abs();
    let height = dib_height / 2;
    let bits = u16::from_le_bytes(bytes[offset + 14..offset + 16].try_into().unwrap());
    if width == 0 || height == 0 || !matches!(bits, 24 | 32) {
        return Err("El bitmap del CUR no es compatible".into());
    }
    let stride = (width * u32::from(bits) / 8 + 3) & !3;
    let pixel_start = offset + 40;
    if pixel_start + (stride * height) as usize > bytes.len() {
        return Err("El bitmap del CUR está incompleto".into());
    }
    let mut image = RgbaImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let source_y = height - 1 - y;
            let index = pixel_start + (source_y * stride + x * u32::from(bits) / 8) as usize;
            let alpha = if bits == 32 { bytes[index + 3] } else { 255 };
            image.put_pixel(
                x,
                y,
                Rgba([bytes[index + 2], bytes[index + 1], bytes[index], alpha]),
            );
        }
    }
    Ok(DynamicImage::ImageRgba8(image))
}

fn write_cur_preview(source: &Path, output: &Path) -> Result<(), String> {
    let image = decode_cur_image(source)?;
    image
        .resize(400, 250, FilterType::Lanczos3)
        .save(output)
        .map_err(|e| e.to_string())?;
    image::open(output).map_err(|e| format!("La miniatura CUR no se puede leer: {e}"))?;
    Ok(())
}

#[tauri::command]
pub fn list_cursors() -> Result<Vec<CursorResource>, String> {
    let conn = get_connection().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare("SELECT id, name, path, preview, resource_type, source_path, hash, hotspot_x, hotspot_y, created_at FROM cursor_resources ORDER BY created_at DESC").map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(CursorResource {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                preview: row.get(3)?,
                resource_type: row.get(4)?,
                source_path: row.get(5)?,
                hash: row.get(6)?,
                hotspot_x: row.get(7)?,
                hotspot_y: row.get(8)?,
                created_at: row.get(9)?,
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(Result::ok).collect())
}

#[tauri::command]
pub fn import_cursor(
    source_path: String,
    name: Option<String>,
    hotspot_x: Option<u16>,
    hotspot_y: Option<u16>,
) -> Result<String, String> {
    let source = Path::new(&source_path);
    if !source.is_file() {
        return Err("No se encontró el archivo seleccionado".into());
    }
    if fs::metadata(source).map_err(|e| e.to_string())?.len() == 0 {
        return Err("El archivo del cursor está vacío".into());
    }
    let detected = detect_kind(source)?;
    let source_hash = {
        let mut file = fs::File::open(source).map_err(|e| e.to_string())?;
        let mut hasher = Sha256::new();
        std::io::copy(&mut file, &mut hasher).map_err(|e| e.to_string())?;
        format!("{:x}", hasher.finalize())
    };
    let display_name = validate_name(name.as_deref().unwrap_or_else(|| {
        source
            .file_stem()
            .and_then(|v| v.to_str())
            .unwrap_or("Cursor")
    }))?;
    let id = Uuid::new_v4().to_string();
    let root = app_data_dir().map_err(|e| e.to_string())?;
    let source_ext = source
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("bin")
        .to_ascii_lowercase();
    let managed_path = root.join("cursors/resources").join(format!(
        "{id}.{}",
        if matches!(detected, CursorKind::Static) {
            "cur"
        } else {
            "ani"
        }
    ));
    let source_copy = if matches!(detected, CursorKind::Static) && source_ext != "cur" {
        let image = image::open(source).map_err(|e| format!("La imagen no se puede leer: {e}"))?;
        validate_image_dimensions(&image)?;
        let hx = hotspot_x.unwrap_or(0);
        let hy = hotspot_y.unwrap_or(0);
        let bytes = encode_cur(&image, hx, hy)?;
        fs::write(&managed_path, bytes).map_err(|e| e.to_string())?;
        let copy = root
            .join("cursors/sources")
            .join(format!("{id}.{source_ext}"));
        fs::copy(source, &copy).map_err(|e| e.to_string())?;
        Some(copy)
    } else {
        validate_cursor_file(source, detected)?;
        fs::copy(source, &managed_path).map_err(|e| e.to_string())?;
        None
    };
    validate_cursor_file(&managed_path, detected)?;
    let preview_path = if source_ext == "cur" {
        let preview = root.join("cursors/previews").join(format!("{id}.jpg"));
        write_cur_preview(&managed_path, &preview).ok();
        preview.is_file().then_some(preview)
    } else if source_ext != "ani" {
        let preview = root.join("cursors/previews").join(format!("{id}.jpg"));
        generate_thumbnail(&source.to_string_lossy(), &preview.to_string_lossy())?;
        Some(preview)
    } else {
        None
    };
    let conn = get_connection().map_err(|e| e.to_string())?;
    let duplicate: Option<String> = conn
        .query_row(
            "SELECT id FROM cursor_resources WHERE path = ?1 OR hash = ?2 LIMIT 1",
            rusqlite::params![managed_path.to_string_lossy(), source_hash],
            |row| row.get(0),
        )
        .ok();
    if duplicate.is_some() {
        let _ = fs::remove_file(&managed_path);
        if let Some(path) = &preview_path {
            let _ = fs::remove_file(path);
        }
        if let Some(path) = &source_copy {
            let _ = fs::remove_file(path);
        }
        return Err("Este cursor ya está en la biblioteca".into());
    }
    if let Err(error) = conn.execute("INSERT INTO cursor_resources(id, name, path, preview, resource_type, source_path, hash, hotspot_x, hotspot_y, created_at) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)", rusqlite::params![id, display_name, managed_path.to_string_lossy(), preview_path.as_ref().map(|p| p.to_string_lossy().into_owned()), detected.as_str(), source_copy.as_ref().map(|p| p.to_string_lossy().into_owned()), source_hash, hotspot_x.unwrap_or(0), hotspot_y.unwrap_or(0), Utc::now().to_rfc3339()]) {
        let _ = fs::remove_file(&managed_path);
        if let Some(path) = &preview_path { let _ = fs::remove_file(path); }
        if let Some(path) = &source_copy { let _ = fs::remove_file(path); }
        return Err(error.to_string());
    }
    Ok(id)
}

#[tauri::command]
pub fn import_synced_cursor(
    bytes: Vec<u8>,
    name: String,
    extension: String,
    hotspot_x: Option<u16>,
    hotspot_y: Option<u16>,
    expected_hash: Option<String>,
) -> Result<String, String> {
    let extension = extension.to_ascii_lowercase();
    if bytes.is_empty()
        || !["cur", "ani", "png", "jpg", "jpeg", "webp", "bmp"].contains(&extension.as_str())
    {
        return Err("El cursor remoto no tiene un formato válido".into());
    }
    if let Some(expected) = expected_hash {
        let actual = Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        if actual != expected.to_ascii_lowercase() {
            return Err("La descarga del cursor no coincide con su SHA-256".into());
        }
    }
    let root = app_data_dir().map_err(|e| e.to_string())?;
    let temp = root
        .join("cursors/sources")
        .join(format!("sync-{}.{}", Uuid::new_v4(), extension));
    fs::write(&temp, bytes).map_err(|e| e.to_string())?;
    let result = import_cursor(
        temp.to_string_lossy().into_owned(),
        Some(name),
        hotspot_x,
        hotspot_y,
    );
    let _ = fs::remove_file(temp);
    result
}

#[tauri::command]
pub fn rename_cursor(cursor_id: String, name: String) -> Result<(), String> {
    let name = validate_name(&name)?;
    let conn = get_connection().map_err(|e| e.to_string())?;
    if conn
        .execute(
            "UPDATE cursor_resources SET name = ?1 WHERE id = ?2",
            (&name, &cursor_id),
        )
        .map_err(|e| e.to_string())?
        == 0
    {
        return Err("El cursor ya no existe".into());
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn apply_system_cursor(path: &Path) -> Result<(), String> {
    use windows::{
        core::PCWSTR,
        Win32::UI::WindowsAndMessaging::{LoadCursorFromFileW, SetSystemCursor, OCR_NORMAL},
    };
    let wide: Vec<u16> = path
        .as_os_str()
        .to_string_lossy()
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let cursor = unsafe { LoadCursorFromFileW(PCWSTR(wide.as_ptr())) }
        .map_err(|e| format!("Windows no pudo cargar el cursor: {e}"))?;
    unsafe { SetSystemCursor(cursor, OCR_NORMAL) }
        .map_err(|e| format!("Windows no pudo activar el cursor: {e}"))
}

#[cfg(not(target_os = "windows"))]
fn apply_system_cursor(_path: &Path) -> Result<(), String> {
    Err("La activación de cursores del sistema solo está disponible en Windows".into())
}

#[cfg(target_os = "windows")]
fn restore_system_cursor() -> Result<(), String> {
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPIF_SENDCHANGE, SPI_SETCURSORS,
    };
    unsafe { SystemParametersInfoW(SPI_SETCURSORS, 0, None, SPIF_SENDCHANGE) }
        .map_err(|e| format!("Windows no pudo restaurar el cursor predeterminado: {e}"))
}

pub fn restore_active_cursor() -> Result<(), String> {
    let conn = get_connection().map_err(|e| e.to_string())?;
    let active: Option<String> = conn
        .query_row(
            "SELECT CASE WHEN cursor_enabled = 1 THEN active_cursor_id ELSE NULL END FROM settings LIMIT 1",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    let Some(cursor_id) = active else {
        return Ok(());
    };
    let path: String = conn
        .query_row(
            "SELECT path FROM cursor_resources WHERE id = ?1",
            [&cursor_id],
            |row| row.get(0),
        )
        .map_err(|_| "El cursor activo ya no existe".to_string())?;
    apply_system_cursor(Path::new(&path))
}

#[cfg(not(target_os = "windows"))]
fn restore_system_cursor() -> Result<(), String> {
    Err("La restauración del cursor del sistema solo está disponible en Windows".into())
}

#[tauri::command]
pub fn activate_cursor(
    cursor_id: String,
    scheduler: State<'_, DynamicScheduler>,
) -> Result<(), String> {
    let conn = get_connection().map_err(|e| e.to_string())?;
    let path: String = conn
        .query_row(
            "SELECT path FROM cursor_resources WHERE id = ?1",
            [&cursor_id],
            |row| row.get(0),
        )
        .map_err(|_| "El cursor ya no existe".to_string())?;
    apply_system_cursor(Path::new(&path))?;
    conn.execute("UPDATE settings SET cursor_enabled = 1, active_cursor_id = ?1 WHERE id = (SELECT id FROM settings LIMIT 1)", [&cursor_id]).map_err(|e| e.to_string())?;
    scheduler.stop();
    Ok(())
}

#[tauri::command]
pub fn restore_default_cursor(scheduler: State<'_, DynamicScheduler>) -> Result<(), String> {
    restore_system_cursor()?;
    let conn = get_connection().map_err(|e| e.to_string())?;
    conn.execute("UPDATE settings SET cursor_enabled = 0, active_cursor_id = NULL WHERE id = (SELECT id FROM settings LIMIT 1)", []).map_err(|e| e.to_string())?;
    scheduler.stop();
    Ok(())
}

#[tauri::command]
pub fn delete_cursor(
    cursor_id: String,
    scheduler: State<'_, DynamicScheduler>,
) -> Result<(), String> {
    let conn = get_connection().map_err(|e| e.to_string())?;
    let (path, preview, source): (String, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT path, preview, source_path FROM cursor_resources WHERE id = ?1",
            [&cursor_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|_| "El cursor ya no existe".to_string())?;
    let active: bool = conn
        .query_row(
            "SELECT cursor_enabled = 1 AND active_cursor_id = ?1 FROM settings LIMIT 1",
            [&cursor_id],
            |row| row.get(0),
        )
        .unwrap_or(false);
    if active {
        restore_system_cursor()?;
        conn.execute("UPDATE settings SET cursor_enabled = 0, active_cursor_id = NULL WHERE id = (SELECT id FROM settings LIMIT 1)", []).map_err(|e| e.to_string())?;
        scheduler.stop();
    }
    conn.execute("DELETE FROM cursor_resources WHERE id = ?1", [&cursor_id])
        .map_err(|e| e.to_string())?;
    let _ = fs::remove_file(path);
    if let Some(path) = preview {
        let _ = fs::remove_file(path);
    }
    if let Some(path) = source {
        let _ = fs::remove_file(path);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        detect_kind, encode_cur, validate_cursor_file, validate_image_dimensions, CursorKind,
    };
    use image::RgbaImage;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn detects_cursor_formats() {
        assert_eq!(
            detect_kind(std::path::Path::new("x.cur")).unwrap(),
            CursorKind::Static
        );
        assert_eq!(
            detect_kind(std::path::Path::new("x.ANI")).unwrap(),
            CursorKind::Animated
        );
        assert!(detect_kind(std::path::Path::new("x.gif")).is_err());
    }

    #[test]
    fn converts_and_validates_static_cursor() {
        let dir = std::env::temp_dir().join(format!(
            "lumina-cursor-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let image = image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            12,
            8,
            image::Rgba([10, 20, 30, 255]),
        ));
        let path = dir.join("cursor.cur");
        fs::write(&path, encode_cur(&image, 0, 0).unwrap()).unwrap();
        validate_cursor_file(&path, CursorKind::Static).unwrap();
        assert!(validate_image_dimensions(&image).is_ok());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_invalid_cursor_data() {
        let dir = std::env::temp_dir().join("lumina-invalid-cursor-test");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("bad.cur");
        fs::write(&path, b"bad").unwrap();
        assert!(validate_cursor_file(&path, CursorKind::Static).is_err());
        let _ = fs::remove_dir_all(dir);
    }
}
