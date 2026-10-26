use chrono::Utc;
use image::{imageops::FilterType, DynamicImage, GenericImageView, Rgba, RgbaImage};
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

use crate::{
    db::{app_data_dir, ensure_physical_operations_allowed, get_connection},
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

fn cleanup_import_files(
    managed: &Path,
    preview: Option<&Path>,
    source: Option<&Path>,
) -> Vec<String> {
    let mut errors = Vec::new();
    for (kind, path) in [
        ("recurso", Some(managed)),
        ("preview", preview),
        ("fuente", source),
    ]
    .into_iter()
    .filter_map(|(kind, path)| path.map(|path| (kind, path)))
    {
        if let Err(error) = fs::remove_file(path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                errors.push(format!("{kind} {path:?}: {error}"));
            }
        }
    }
    errors
}

fn import_error(error: String, cleanup_errors: Vec<String>) -> String {
    if cleanup_errors.is_empty() {
        error
    } else {
        format!(
            "{error}; la limpieza de archivos creados fallÃ³: {}",
            cleanup_errors.join("; ")
        )
    }
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
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("No se pudieron leer los cursores: {error}"))
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
        if let Err(error) = fs::write(&managed_path, bytes) {
            let cleanup_errors = cleanup_import_files(&managed_path, None, None);
            return Err(import_error(error.to_string(), cleanup_errors));
        }
        let copy = root
            .join("cursors/sources")
            .join(format!("{id}.{source_ext}"));
        if let Err(error) = fs::copy(source, &copy) {
            let cleanup_errors = cleanup_import_files(&managed_path, None, Some(&copy));
            return Err(import_error(error.to_string(), cleanup_errors));
        }
        Some(copy)
    } else {
        validate_cursor_file(source, detected)?;
        if let Err(error) = fs::copy(source, &managed_path) {
            let cleanup_errors = cleanup_import_files(&managed_path, None, None);
            return Err(import_error(error.to_string(), cleanup_errors));
        }
        None
    };
    if let Err(error) = validate_cursor_file(&managed_path, detected) {
        let cleanup_errors = cleanup_import_files(&managed_path, None, source_copy.as_deref());
        return Err(import_error(error, cleanup_errors));
    }
    let preview_path = if source_ext == "cur" {
        let preview = root.join("cursors/previews").join(format!("{id}.jpg"));
        write_cur_preview(&managed_path, &preview).ok();
        preview.is_file().then_some(preview)
    } else if source_ext != "ani" {
        let preview = root.join("cursors/previews").join(format!("{id}.jpg"));
        if let Err(error) =
            generate_thumbnail(&source.to_string_lossy(), &preview.to_string_lossy())
        {
            let cleanup_errors =
                cleanup_import_files(&managed_path, Some(&preview), source_copy.as_deref());
            return Err(import_error(error, cleanup_errors));
        }
        Some(preview)
    } else {
        None
    };
    let conn = match get_connection() {
        Ok(conn) => conn,
        Err(error) => {
            let cleanup_errors = cleanup_import_files(
                &managed_path,
                preview_path.as_deref(),
                source_copy.as_deref(),
            );
            return Err(import_error(error.to_string(), cleanup_errors));
        }
    };
    let duplicate: Option<String> = conn
        .query_row(
            "SELECT id FROM cursor_resources WHERE path = ?1 OR hash = ?2 LIMIT 1",
            rusqlite::params![managed_path.to_string_lossy(), source_hash],
            |row| row.get(0),
        )
        .ok();
    if duplicate.is_some() {
        let cleanup_errors = cleanup_import_files(
            &managed_path,
            preview_path.as_deref(),
            source_copy.as_deref(),
        );
        if !cleanup_errors.is_empty() {
            return Err(import_error(
                "El cursor duplicado no pudo limpiarse completamente".into(),
                cleanup_errors,
            ));
        }
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
        let cleanup_errors = cleanup_import_files(
            &managed_path,
            preview_path.as_deref(),
            source_copy.as_deref(),
        );
        if !cleanup_errors.is_empty() {
            return Err(import_error(error.to_string(), cleanup_errors));
        }
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

pub(crate) fn replace_cursor_resource_in_connection(
    conn: &mut Connection,
    root: &Path,
    cursor_id: &str,
    source: &Path,
) -> Result<(), String> {
    if cursor_id.trim().is_empty() || !source.is_file() {
        return Err("El recurso de reemplazo del cursor no es válido".into());
    }
    let detected = detect_kind(source)?;
    validate_cursor_file(source, detected)?;
    let source_hash = {
        let bytes = fs::read(source).map_err(|error| error.to_string())?;
        format!("{:x}", Sha256::digest(bytes))
    };
    let (path, preview, resource_type): (String, Option<String>, String) = conn
        .query_row(
            "SELECT path,preview,resource_type FROM cursor_resources WHERE id=?1",
            [cursor_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|_| "El cursor ya no existe en la biblioteca".to_string())?;
    if resource_type != detected.as_str() {
        return Err("El tipo del cursor entrante no coincide con el recurso existente".into());
    }
    let target = PathBuf::from(path);
    if !path_is_under(root, &target) || !target.is_file() {
        return Err("La ruta administrada del cursor no cumple el contrato de reemplazo".into());
    }
    let preview_target = if detected == CursorKind::Static {
        Some(preview.map(PathBuf::from).unwrap_or_else(|| {
            root.join("cursors/previews")
                .join(format!("{cursor_id}.jpg"))
        }))
    } else {
        preview.map(PathBuf::from)
    };
    if preview_target
        .as_ref()
        .is_some_and(|candidate| !path_is_under(root, candidate))
    {
        return Err("La ruta de vista previa del cursor no cumple el contrato de reemplazo".into());
    }
    let token = Uuid::new_v4().to_string();
    let staged = target.with_file_name(format!(".lumina-replace-{token}"));
    let staged_preview = preview_target
        .as_ref()
        .map(|path| path.with_file_name(format!(".lumina-replace-{token}.jpg")));
    let backup = target.with_file_name(format!(".lumina-backup-{token}"));
    let preview_backup = preview_target
        .as_ref()
        .map(|path| path.with_file_name(format!(".lumina-backup-{token}.jpg")));
    let had_preview = preview_target.as_ref().is_some_and(|path| path.is_file());
    fs::copy(source, &staged)
        .map_err(|error| format!("No se pudo preparar el reemplazo del cursor: {error}"))?;
    if let Err(error) = validate_cursor_file(&staged, detected) {
        let _ = fs::remove_file(&staged);
        return Err(error);
    }
    if let Some(preview) = &staged_preview {
        if let Err(error) = write_cur_preview(&staged, preview) {
            let _ = fs::remove_file(&staged);
            return Err(format!(
                "No se pudo preparar la vista previa del cursor: {error}"
            ));
        }
    }
    let operation = (|| {
        fs::rename(&target, &backup)
            .map_err(|error| format!("No se pudo respaldar el cursor actual: {error}"))?;
        if let (Some(path), Some(backup)) = (&preview_target, &preview_backup) {
            if path.is_file() {
                fs::rename(path, backup).map_err(|error| {
                    format!("No se pudo respaldar la vista previa actual: {error}")
                })?;
            }
        }
        fs::rename(&staged, &target)
            .map_err(|error| format!("No se pudo colocar el cursor nuevo: {error}"))?;
        if let Some(preview) = &staged_preview {
            fs::rename(preview, preview_target.as_ref().unwrap())
                .map_err(|error| format!("No se pudo colocar la vista previa nueva: {error}"))?;
        } else if let Some(preview) = &preview_target {
            if preview.exists() {
                fs::remove_file(preview).map_err(|error| {
                    format!("No se pudo retirar la vista previa anterior: {error}")
                })?;
            }
        }
        let tx = conn
            .unchecked_transaction()
            .map_err(|error| error.to_string())?;
        tx.execute(
            "UPDATE cursor_resources SET hash=?1, preview=?2 WHERE id=?3",
            rusqlite::params![
                source_hash,
                staged_preview
                    .as_ref()
                    .and_then(|_| preview_target.as_ref())
                    .map(|path| path.to_string_lossy().into_owned()),
                cursor_id
            ],
        )
        .map_err(|error| format!("No se pudo persistir el reemplazo del cursor: {error}"))?;
        tx.commit().map_err(|error| error.to_string())?;
        Ok::<(), String>(())
    })();
    if let Err(error) = operation {
        let mut rollback = Vec::new();
        if target.exists() {
            let _ = fs::remove_file(&target);
        }
        if let Err(restore) = fs::rename(&backup, &target) {
            rollback.push(format!("no se pudo restaurar el cursor: {restore}"));
        }
        if let (Some(path), Some(backup)) = (&preview_target, &preview_backup) {
            if had_preview && backup.exists() {
                if path.exists() {
                    let _ = fs::remove_file(path);
                }
                if let Err(restore) = fs::rename(backup, path) {
                    rollback.push(format!("no se pudo restaurar la vista previa: {restore}"));
                }
            } else if !had_preview && path.exists() {
                let _ = fs::remove_file(path);
            }
        }
        let _ = fs::remove_file(&staged);
        if let Some(preview) = &staged_preview {
            let _ = fs::remove_file(preview);
        }
        if rollback.is_empty() {
            Err(error)
        } else {
            Err(format!(
                "{error}; el rollback quedó incompleto: {}",
                rollback.join("; ")
            ))
        }
    } else {
        let mut cleanup = Vec::new();
        if backup.exists() {
            if let Err(error) = fs::remove_file(&backup) {
                cleanup.push(format!("{}: {error}", backup.display()));
            }
        }
        if let Some(backup) = preview_backup {
            if backup.exists() {
                if let Err(error) = fs::remove_file(backup) {
                    cleanup.push(format!("vista previa: {error}"));
                }
            }
        }
        if cleanup.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "El reemplazo se persistió, pero la limpieza quedó incompleta: {}",
                cleanup.join("; ")
            ))
        }
    }
}

fn path_is_under(root: &Path, candidate: &Path) -> bool {
    let Ok(root) = root.canonicalize() else {
        return false;
    };
    let candidate = if candidate.exists() {
        candidate.canonicalize().ok()
    } else {
        candidate
            .parent()
            .and_then(|parent| parent.canonicalize().ok())
            .map(|parent| parent.join(candidate.file_name().unwrap_or_default()))
    };
    candidate.is_some_and(|candidate| candidate.starts_with(root))
}

#[tauri::command]
pub fn replace_cursor_resource(cursor_id: String, source_path: String) -> Result<(), String> {
    let root = app_data_dir().map_err(|error| error.to_string())?;
    let mut conn = get_connection().map_err(|error| error.to_string())?;
    replace_cursor_resource_in_connection(&mut conn, &root, &cursor_id, Path::new(&source_path))
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
    ensure_physical_operations_allowed()?;
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
pub fn activate_cursor(cursor_id: String) -> Result<(), String> {
    ensure_physical_operations_allowed()?;
    let conn = get_connection().map_err(|e| e.to_string())?;
    let path: String = conn
        .query_row(
            "SELECT path FROM cursor_resources WHERE id = ?1",
            [&cursor_id],
            |row| row.get(0),
        )
        .map_err(|_| "El cursor ya no existe".to_string())?;
    apply_system_cursor(Path::new(&path))?;
    conn.execute("UPDATE settings SET cursor_enabled = 1, active_cursor_id = ?1 WHERE id = (SELECT id FROM settings LIMIT 1)", [&cursor_id]).map_err(|error| format!("El cursor se aplicÃ³ en Windows, pero no se pudo persistir la selecciÃ³n: {error}"))?;
    Ok(())
}

#[tauri::command]
pub fn restore_default_cursor() -> Result<(), String> {
    ensure_physical_operations_allowed()?;
    restore_system_cursor()?;
    let conn = get_connection().map_err(|e| e.to_string())?;
    conn.execute("UPDATE settings SET cursor_enabled = 0, active_cursor_id = NULL WHERE id = (SELECT id FROM settings LIMIT 1)", []).map_err(|error| format!("Windows restaurÃ³ el cursor predeterminado, pero no se pudo persistir el estado: {error}"))?;
    Ok(())
}

#[tauri::command]
pub fn delete_cursor(cursor_id: String) -> Result<(), String> {
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
        ensure_physical_operations_allowed()?;
        restore_system_cursor()?;
        conn.execute("UPDATE settings SET cursor_enabled = 0, active_cursor_id = NULL WHERE id = (SELECT id FROM settings LIMIT 1)", []).map_err(|error| format!("Windows restaurÃ³ el cursor predeterminado, pero no se pudo persistir la eliminaciÃ³n: {error}"))?;
    }
    conn.execute("DELETE FROM cursor_resources WHERE id = ?1", [&cursor_id])
        .map_err(|e| e.to_string())?;
    let mut cleanup_errors = Vec::new();
    for (kind, path) in [
        ("recurso", Some(path)),
        ("preview", preview),
        ("fuente", source),
    ]
    .into_iter()
    .filter_map(|(kind, path)| path.map(|path| (kind, path)))
    {
        if let Err(error) = fs::remove_file(&path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                cleanup_errors.push(format!("{kind} {path}: {error}"));
            }
        }
    }
    if cleanup_errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Cursor eliminado, pero la limpieza física quedó incompleta: {}",
            cleanup_errors.join("; ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        detect_kind, encode_cur, replace_cursor_resource_in_connection, validate_cursor_file,
        validate_image_dimensions, CursorKind,
    };
    use image::RgbaImage;
    use rusqlite::Connection;
    use sha2::Digest;
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

    #[test]
    fn replaces_cursor_content_preserving_identity_and_preview() {
        let root =
            std::env::temp_dir().join(format!("lumina-replace-cursor-{}", uuid::Uuid::new_v4()));
        let resources = root.join("cursors/resources");
        let previews = root.join("cursors/previews");
        fs::create_dir_all(&resources).unwrap();
        fs::create_dir_all(&previews).unwrap();
        let target = resources.join("cursor.cur");
        let preview = previews.join("cursor.jpg");
        let old = encode_cur(
            &image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(
                8,
                8,
                image::Rgba([1, 2, 3, 255]),
            )),
            0,
            0,
        )
        .unwrap();
        fs::write(&target, &old).unwrap();
        image::RgbImage::from_pixel(10, 10, image::Rgb([1, 2, 3]))
            .save(&preview)
            .unwrap();
        let source = root.join("incoming.cur");
        let new_bytes = encode_cur(
            &image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(
                12,
                6,
                image::Rgba([200, 30, 40, 255]),
            )),
            1,
            1,
        )
        .unwrap();
        fs::write(&source, &new_bytes).unwrap();
        let old_hash = format!("{:x}", sha2::Sha256::digest(&old));
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE cursor_resources(id TEXT PRIMARY KEY, name TEXT, path TEXT, preview TEXT, resource_type TEXT, source_path TEXT, hash TEXT, hotspot_x INTEGER, hotspot_y INTEGER);").unwrap();
        conn.execute("INSERT INTO cursor_resources VALUES ('shared-id','Original',?1,?2,'static','source.cur',?3,0,0)", rusqlite::params![target.to_string_lossy(), preview.to_string_lossy(), old_hash]).unwrap();

        replace_cursor_resource_in_connection(&mut conn, &root, "shared-id", &source).unwrap();

        let (name, path, hash, hotspot): (String, String, String, i64) = conn
            .query_row(
                "SELECT name,path,hash,hotspot_x FROM cursor_resources WHERE id='shared-id'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(name, "Original");
        assert_eq!(path, target.to_string_lossy());
        assert_ne!(hash, old_hash);
        assert_eq!(hotspot, 0);
        assert_eq!(fs::read(&target).unwrap(), new_bytes);
        assert!(image::open(&preview).is_ok());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_cursor_replacement_with_wrong_kind_without_touching_the_target() {
        let dir = std::env::temp_dir().join(format!(
            "lumina-replace-cursor-invalid-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).unwrap();
        let target = dir.join("target.cur");
        let source = dir.join("source.png");
        fs::write(
            &target,
            encode_cur(
                &image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(
                    4,
                    4,
                    image::Rgba([1, 2, 3, 255]),
                )),
                0,
                0,
            )
            .unwrap(),
        )
        .unwrap();
        image::RgbImage::from_pixel(4, 4, image::Rgb([9, 9, 9]))
            .save(&source)
            .unwrap();
        let original = fs::read(&target).unwrap();
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE cursor_resources(id TEXT PRIMARY KEY, name TEXT, path TEXT, preview TEXT, resource_type TEXT);").unwrap();
        conn.execute(
            "INSERT INTO cursor_resources VALUES ('id','name',?1,NULL,'static')",
            [&target.to_string_lossy()],
        )
        .unwrap();
        assert!(replace_cursor_resource_in_connection(&mut conn, &dir, "id", &source).is_err());
        assert_eq!(fs::read(&target).unwrap(), original);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn failed_cursor_persistence_restores_cursor_and_preview() {
        let dir = std::env::temp_dir().join(format!(
            "lumina-replace-cursor-rollback-{}",
            uuid::Uuid::new_v4()
        ));
        let resources = dir.join("resources");
        let previews = dir.join("previews");
        fs::create_dir_all(&resources).unwrap();
        fs::create_dir_all(&previews).unwrap();
        let target = resources.join("cursor.cur");
        let preview = previews.join("cursor.jpg");
        let old = encode_cur(
            &image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(
                4,
                4,
                image::Rgba([1, 2, 3, 255]),
            )),
            0,
            0,
        )
        .unwrap();
        fs::write(&target, &old).unwrap();
        fs::write(&preview, b"old-preview").unwrap();
        let source = dir.join("new.cur");
        fs::write(
            &source,
            encode_cur(
                &image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(
                    6,
                    6,
                    image::Rgba([9, 8, 7, 255]),
                )),
                0,
                0,
            )
            .unwrap(),
        )
        .unwrap();
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE cursor_resources(id TEXT PRIMARY KEY, name TEXT, path TEXT, preview TEXT, resource_type TEXT);").unwrap();
        conn.execute(
            "INSERT INTO cursor_resources VALUES ('id','name',?1,?2,'static')",
            rusqlite::params![target.to_string_lossy(), preview.to_string_lossy()],
        )
        .unwrap();
        assert!(replace_cursor_resource_in_connection(&mut conn, &dir, "id", &source).is_err());
        assert_eq!(fs::read(&target).unwrap(), old);
        assert_eq!(fs::read(&preview).unwrap(), b"old-preview");
        let _ = fs::remove_dir_all(dir);
    }
}
