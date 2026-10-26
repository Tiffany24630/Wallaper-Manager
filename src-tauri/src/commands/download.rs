use crate::{
    commands::{
        cursor::{import_cursor, import_cursor_with_id},
        secure_storage::{
            get_secure_session_token, get_secure_session_token_unlocked, session_lock,
        },
        wallpaper::{import_file, import_file_with_id},
    },
    db::{app_data_dir, get_connection},
};
use reqwest::{blocking::Client, redirect::Policy, Url};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};
use uuid::Uuid;

const MAX_DOWNLOAD_BYTES: u64 = 100 * 1024 * 1024;
const CHUNK_SIZE: usize = 1024 * 1024;
static IMPORT_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct TempFile(PathBuf);
impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[tauri::command]
pub async fn download_and_import_synced_resource(
    api_url: String,
    device_id: String,
    file_id: String,
    kind: String,
    name: String,
    extension: String,
    expected_hash: String,
    hotspot_x: Option<u16>,
    hotspot_y: Option<u16>,
    local_id: Option<String>,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        download_and_import_sync(
            api_url,
            device_id,
            file_id,
            kind,
            name,
            extension,
            expected_hash,
            hotspot_x,
            hotspot_y,
            local_id,
        )
    })
    .await
    .map_err(|error| format!("La descarga nativa terminó inesperadamente: {error}"))?
}

fn find_existing_resource(kind: &str, hash: &str) -> Result<Option<String>, String> {
    let conn = get_connection().map_err(|e| e.to_string())?;
    let table = match kind {
        "wallpaper" => "wallpapers",
        "cursor" => "cursor_resources",
        _ => return Ok(None),
    };
    let sql = format!("SELECT id FROM {table} WHERE hash = ?1 LIMIT 1");
    conn.query_row(&sql, [hash.to_ascii_lowercase()], |row| row.get(0))
        .map(Some)
        .or_else(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other.to_string()),
        })
}

fn attach_pending_playlists(wallpaper_id: &str) -> Result<(), String> {
    let mut conn = get_connection().map_err(|error| error.to_string())?;
    let tx = conn.transaction().map_err(|error| error.to_string())?;
    tx.execute(
        "INSERT OR IGNORE INTO playlist_wallpapers(playlist_id, wallpaper_id, position)
         SELECT pending.playlist_id, ?1,
           COALESCE((SELECT MAX(existing.position) + 1 FROM playlist_wallpapers AS existing WHERE existing.playlist_id = pending.playlist_id), 0)
         FROM sync_pending_playlist_items AS pending WHERE pending.wallpaper_id = ?1",
        [wallpaper_id],
    )
    .map_err(|error| error.to_string())?;
    tx.execute(
        "DELETE FROM sync_pending_playlist_items WHERE wallpaper_id=?1",
        [wallpaper_id],
    )
    .map_err(|error| error.to_string())?;
    tx.commit().map_err(|error| error.to_string())
}

fn clear_sync_tombstone(kind: &str, record_id: &str) -> Result<(), String> {
    if !matches!(kind, "wallpaper" | "cursor") {
        return Err("El tipo de recurso sincronizado no es compatible".into());
    }
    let conn = get_connection().map_err(|error| error.to_string())?;
    conn.execute(
        "DELETE FROM sync_tombstones WHERE entity_id=?1 AND kind=?2",
        rusqlite::params![record_id, kind],
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        net::TcpListener,
        thread,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn rejects_insecure_remote_http_and_arbitrary_urls() {
        assert!(validate_base_url("http://127.0.0.1:8765").is_ok());
        assert!(validate_base_url("https://sync.example.test").is_ok());
        assert!(validate_base_url("http://sync.example.test").is_err());
        assert!(validate_base_url("https://sync.example.test/?next=https://other.test").is_err());
    }

    #[test]
    fn validates_supported_content_signatures() {
        let path = std::env::temp_dir().join(format!(
            "lumina-download-test-{}.png",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(&path, b"\x89PNG\r\n\x1a\nvalid").unwrap();
        assert!(validate_content(&path, "wallpaper", "png").is_ok());
        assert!(validate_content(&path, "cursor", "cur").is_err());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn validates_file_ids_and_hashes() {
        assert!(valid_file_id(&"a".repeat(32)));
        assert!(!valid_file_id("../file"));
        assert!(valid_hash(&"a".repeat(64)));
        assert!(!valid_hash("not-a-hash"));
    }

    #[test]
    fn streams_chunked_response_and_cleans_temp_file() {
        let payload = b"\x89PNG\r\n\x1a\nsmall-payload".to_vec();
        let expected = format!("{:x}", Sha256::digest(&payload));
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server_payload = payload.clone();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 512];
            let _ = std::io::Read::read(&mut stream, &mut request);
            let split = server_payload.len() / 2;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n"
            )
            .unwrap();
            write!(stream, "{:x}\r\n", split).unwrap();
            stream.write_all(&server_payload[..split]).unwrap();
            write!(stream, "\r\n{:x}\r\n", server_payload.len() - split).unwrap();
            stream.write_all(&server_payload[split..]).unwrap();
            write!(stream, "\r\n0\r\n\r\n").unwrap();
        });
        let response = Client::new()
            .get(format!("http://{address}/v1/files/test"))
            .send()
            .unwrap();
        let path =
            std::env::temp_dir().join(format!("lumina-download-stream-{}.part", Uuid::new_v4()));
        download_response_to_file(response, &path, &expected).unwrap();
        assert_eq!(fs::read(&path).unwrap(), payload);
        fs::remove_file(&path).unwrap();
        server.join().unwrap();
    }

    #[test]
    fn rejects_wrong_hash_and_temp_file_can_be_removed() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 256];
            let _ = std::io::Read::read(&mut stream, &mut request);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\ntest")
                .unwrap();
        });
        let response = Client::new()
            .get(format!("http://{address}/"))
            .send()
            .unwrap();
        let path =
            std::env::temp_dir().join(format!("lumina-download-hash-{}.part", Uuid::new_v4()));
        assert!(download_response_to_file(response, &path, &"0".repeat(64)).is_err());
        assert!(path.is_file());
        fs::remove_file(&path).unwrap();
        server.join().unwrap();
    }
}

fn valid_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn valid_file_id(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn valid_extension(kind: &str, extension: &str) -> bool {
    match kind {
        "wallpaper" => ["png", "jpg", "jpeg", "webp", "bmp"].contains(&extension),
        "cursor" => ["cur", "ani", "png", "jpg", "jpeg", "webp", "bmp"].contains(&extension),
        _ => false,
    }
}
fn validate_base_url(value: &str) -> Result<Url, String> {
    let url = Url::parse(value.trim())
        .map_err(|_| "La URL de sincronización no es válida".to_string())?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("La URL de sincronización no es segura".into());
    }
    if url.scheme() == "http"
        && !matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))
    {
        return Err("La sincronización remota requiere HTTPS".into());
    }
    Ok(url)
}
fn endpoint(base: &Url, file_id: &str) -> Result<Url, String> {
    let mut url = base.clone();
    url.path_segments_mut()
        .map_err(|_| "La URL de sincronización no permite rutas".to_string())?
        .extend(["v1", "files", file_id]);
    Ok(url)
}
fn validate_content(path: &Path, kind: &str, extension: &str) -> Result<(), String> {
    let mut file = File::open(path).map_err(|e| format!("No se pudo leer la descarga: {e}"))?;
    let mut header = [0u8; 16];
    let count = file.read(&mut header).map_err(|e| e.to_string())?;
    let valid = match extension {
        "png" => header[..count].starts_with(b"\x89PNG\r\n\x1a\n"),
        "jpg" | "jpeg" => header[..count].starts_with(b"\xff\xd8\xff"),
        "webp" => count >= 12 && &header[..4] == b"RIFF" && &header[8..12] == b"WEBP",
        "bmp" => header[..count].starts_with(b"BM"),
        "cur" => header[..count].starts_with(b"\x00\x00\x02\x00"),
        "ani" => count >= 4 && &header[..4] == b"RIFF",
        _ => false,
    };
    if !valid || !valid_extension(kind, extension) {
        return Err("El contenido no coincide con el formato esperado".into());
    }
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
fn download_response_to_file(
    mut response: reqwest::blocking::Response,
    path: &Path,
    expected_hash: &str,
) -> Result<(), String> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_DOWNLOAD_BYTES)
    {
        return Err("El recurso supera el límite de tamaño".into());
    }
    let mut output = File::create(path).map_err(|e| e.to_string())?;
    let mut buffer = vec![0u8; CHUNK_SIZE];
    let mut total = 0u64;
    let mut hash = Sha256::new();
    loop {
        let read = response
            .read(&mut buffer)
            .map_err(|e| format!("La transferencia fue interrumpida: {e}"))?;
        if read == 0 {
            break;
        }
        total += read as u64;
        if total > MAX_DOWNLOAD_BYTES {
            return Err("El recurso supera el límite de tamaño".into());
        }
        output
            .write_all(&buffer[..read])
            .map_err(|e| e.to_string())?;
        hash.update(&buffer[..read]);
    }
    output.flush().map_err(|e| e.to_string())?;
    drop(output);
    if total == 0 || format!("{:x}", hash.finalize()) != expected_hash.to_ascii_lowercase() {
        return Err("La descarga no coincide con el SHA-256 esperado".into());
    }
    Ok(())
}

pub(crate) fn with_downloaded_synced_resource<F, T>(
    api_url: String,
    device_id: String,
    file_id: String,
    kind: String,
    extension: String,
    expected_hash: String,
    condition: Option<(&str, i64)>,
    callback: F,
) -> Result<T, String>
where
    F: FnOnce(&Path) -> Result<T, String>,
{
    let _import_lock = IMPORT_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "El flujo de importación está ocupado".to_string())?;
    if !valid_file_id(&file_id) || !valid_hash(&expected_hash) || device_id.trim().is_empty() {
        return Err("Metadatos de descarga no válidos".into());
    }
    let extension = extension.trim_start_matches('.').to_ascii_lowercase();
    if !valid_extension(&kind, &extension) {
        return Err("El formato remoto no está permitido".into());
    }
    let base = validate_base_url(&api_url)?;
    let token =
        get_secure_session_token()?.ok_or("La sesión segura no está disponible".to_string())?;
    let url = endpoint(&base, &file_id)?;
    let client = Client::builder()
        .redirect(Policy::none())
        .build()
        .map_err(|e| format!("No se pudo preparar la descarga: {e}"))?;
    let mut request = client
        .get(url)
        .bearer_auth(&token)
        .header("X-Device-ID", &device_id);
    if let Some((record_id, revision)) = condition {
        request = request
            .header("X-Sync-Kind", &kind)
            .header("X-Sync-Record-ID", record_id)
            .header("X-Sync-Revision", revision.to_string());
    }
    let response = request
        .send()
        .map_err(|e| format!("No se pudo descargar el recurso: {e}"))?;
    if response.status().is_redirection() {
        return Err("La descarga remota no permite redirecciones".into());
    }
    if response.status().as_u16() == 401 || response.status().as_u16() == 403 {
        return Err("La sesión o el dispositivo ya no están autorizados".into());
    }
    if !response.status().is_success() {
        return Err(format!(
            "El servidor rechazó la descarga (HTTP {})",
            response.status()
        ));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_DOWNLOAD_BYTES)
    {
        return Err("El recurso supera el límite de tamaño".into());
    }
    let root = app_data_dir()
        .map_err(|e| e.to_string())?
        .join("sync/downloads");
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let temp = TempFile(root.join(format!("{}.{}.part", Uuid::new_v4(), extension)));
    let mut output = File::create(&temp.0).map_err(|e| e.to_string())?;
    let mut stream = response;
    let mut buffer = vec![0u8; CHUNK_SIZE];
    let mut total = 0u64;
    let mut hash = Sha256::new();
    loop {
        let read = stream
            .read(&mut buffer)
            .map_err(|e| format!("La transferencia fue interrumpida: {e}"))?;
        if read == 0 {
            break;
        }
        total += read as u64;
        if total > MAX_DOWNLOAD_BYTES {
            return Err("El recurso supera el límite de tamaño".into());
        }
        output
            .write_all(&buffer[..read])
            .map_err(|e| e.to_string())?;
        hash.update(&buffer[..read]);
    }
    output.flush().map_err(|e| e.to_string())?;
    drop(output);
    if total == 0 || format!("{:x}", hash.finalize()) != expected_hash.to_ascii_lowercase() {
        return Err("La descarga no coincide con el SHA-256 esperado".into());
    }
    validate_content(&temp.0, &kind, &extension)?;
    let current_token =
        get_secure_session_token()?.ok_or("La sesión terminó durante la descarga".to_string())?;
    if current_token != token {
        return Err("La sesión cambió durante la descarga".into());
    }
    let _session_lock = session_lock()?;
    if get_secure_session_token_unlocked()?.as_deref() != Some(current_token.as_str()) {
        return Err("La sesión cambió antes de importar el recurso".into());
    }
    callback(&temp.0)
}

fn download_and_import_sync(
    api_url: String,
    device_id: String,
    file_id: String,
    kind: String,
    name: String,
    extension: String,
    expected_hash: String,
    hotspot_x: Option<u16>,
    hotspot_y: Option<u16>,
    local_id: Option<String>,
) -> Result<String, String> {
    if name.trim().is_empty() {
        return Err("Metadatos de descarga no vÃ¡lidos".into());
    }
    if let Some(existing_id) = find_existing_resource(&kind, &expected_hash)? {
        if let Some(local_id) = local_id.as_deref() {
            if local_id != existing_id {
                return Err(
                    "El contenido ya existe bajo otra identidad local; no se duplicarÃ¡ el recurso"
                        .into(),
                );
            }
        }
        if let Some(local_id) = local_id.as_deref() {
            if local_id == existing_id {
                clear_sync_tombstone(&kind, local_id)?;
            }
        }
        if kind == "wallpaper" {
            attach_pending_playlists(&existing_id)?;
        }
        return Ok(existing_id);
    }
    let imported_id = with_downloaded_synced_resource(
        api_url,
        device_id,
        file_id,
        kind.clone(),
        extension,
        expected_hash,
        None,
        |path| {
            if kind == "wallpaper" {
                match local_id.as_deref() {
                    Some(id) => import_file_with_id(path, Some(name), id.to_string()),
                    None => import_file(path, Some(name)),
                }
            } else {
                match local_id.as_deref() {
                    Some(id) => import_cursor_with_id(
                        path.to_string_lossy().into_owned(),
                        Some(name),
                        hotspot_x,
                        hotspot_y,
                        id.to_string(),
                    ),
                    None => import_cursor(
                        path.to_string_lossy().into_owned(),
                        Some(name),
                        hotspot_x,
                        hotspot_y,
                    ),
                }
            }
        },
    )?;
    if let Some(local_id) = local_id.as_deref() {
        if local_id == imported_id {
            clear_sync_tombstone(&kind, local_id)?;
        }
    }
    if kind == "wallpaper" {
        attach_pending_playlists(&imported_id)?;
    }
    Ok(imported_id)
}
