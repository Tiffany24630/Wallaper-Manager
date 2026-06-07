use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use tauri::Manager;

const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "bmp", "gif", "webp"];
const DESKTOP_IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "bmp"];
const VIDEO_EXTENSIONS: &[&str] = &["mp4", "webm", "mov", "mkv", "avi"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum WallpaperKind {
    Image,
    Video,
    Animated,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Wallpaper {
    id: String,
    title: String,
    kind: WallpaperKind,
    path: String,
    relative_path: String,
    size_bytes: u64,
    favorite: bool,
    active: bool,
    can_apply_to_desktop: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BackendStatus {
    ready: bool,
    backend_root: String,
    wallpaper_root: String,
    config_path: String,
    cache_root: String,
    log_root: String,
    total_wallpapers: usize,
    total_playlists: usize,
    active_wallpaper: Option<Wallpaper>,
    uptime_seconds: u64,
    supported_images: Vec<&'static str>,
    supported_videos: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Playlist {
    id: String,
    name: String,
    wallpaper_ids: Vec<String>,
    created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
struct AppSettings {
    launch_at_startup: bool,
    minimize_to_tray: bool,
    hardware_acceleration: bool,
    pause_on_battery: bool,
    pause_when_maximized: bool,
    fps_limit: u16,
    quality: String,
    scaling_mode: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            launch_at_startup: false,
            minimize_to_tray: true,
            hardware_acceleration: true,
            pause_on_battery: true,
            pause_when_maximized: false,
            fps_limit: 60,
            quality: "Ultra".to_string(),
            scaling_mode: "Fill".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ActiveResult {
    wallpaper: Wallpaper,
    desktop_applied: bool,
    message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct StoredConfig {
    active_wallpaper_id: Option<String>,
    favorites: Vec<String>,
    last_imported_path: Option<String>,
    playlists: Vec<Playlist>,
    settings: AppSettings,
}

struct RuntimeState {
    started_at: Instant,
    config: Mutex<StoredConfig>,
}

#[tauri::command]
fn get_backend_status(state: tauri::State<'_, RuntimeState>) -> Result<BackendStatus, String> {
    let backend_root = backend_root();
    ensure_directories(&backend_root)?;
    let config = state.config.lock().map_err(|_| "Config lock poisoned".to_string())?;
    let wallpapers = scan_wallpapers(&backend_root, &config)?;

    Ok(build_status(
        &backend_root,
        state.started_at.elapsed().as_secs(),
        wallpapers,
        &config,
    ))
}

#[tauri::command]
fn list_wallpapers(state: tauri::State<'_, RuntimeState>) -> Result<Vec<Wallpaper>, String> {
    let backend_root = backend_root();
    ensure_directories(&backend_root)?;
    let config = state.config.lock().map_err(|_| "Config lock poisoned".to_string())?;
    scan_wallpapers(&backend_root, &config)
}

#[tauri::command]
fn import_wallpaper(
    source_path: String,
    state: tauri::State<'_, RuntimeState>,
) -> Result<Wallpaper, String> {
    let backend_root = backend_root();
    ensure_directories(&backend_root)?;

    let source = PathBuf::from(source_path.trim());
    if !source.is_file() {
        return Err("The source path must point to an existing file.".to_string());
    }

    let kind = kind_from_path(&source, WallpaperKind::Unknown);
    if kind == WallpaperKind::Unknown {
        return Err(format!(
            "Unsupported file type. Images: {}. Videos: {}.",
            IMAGE_EXTENSIONS.join(", "),
            VIDEO_EXTENSIONS.join(", ")
        ));
    }

    let destination_dir = match kind {
        WallpaperKind::Image => wallpapers_root(&backend_root).join("images"),
        WallpaperKind::Video => wallpapers_root(&backend_root).join("videos"),
        WallpaperKind::Animated => wallpapers_root(&backend_root).join("animated"),
        WallpaperKind::Unknown => unreachable!(),
    };
    fs::create_dir_all(&destination_dir).map_err(to_error)?;

    let destination = unique_destination(&destination_dir, &source)?;
    fs::copy(&source, &destination).map_err(to_error)?;

    let mut config = state.config.lock().map_err(|_| "Config lock poisoned".to_string())?;
    config.last_imported_path = Some(path_to_string(&destination));
    save_config(&backend_root, &config)?;

    let wallpapers = scan_wallpapers(&backend_root, &config)?;
    let id = id_for_path(&destination, &wallpapers_root(&backend_root));
    wallpapers
        .into_iter()
        .find(|wallpaper| wallpaper.id == id)
        .ok_or_else(|| "Imported wallpaper was copied but could not be scanned.".to_string())
}

#[tauri::command]
fn set_active_wallpaper(
    wallpaper_id: String,
    state: tauri::State<'_, RuntimeState>,
) -> Result<ActiveResult, String> {
    let backend_root = backend_root();
    ensure_directories(&backend_root)?;

    let mut config = state.config.lock().map_err(|_| "Config lock poisoned".to_string())?;
    let wallpapers = scan_wallpapers(&backend_root, &config)?;
    let wallpaper = wallpapers
        .into_iter()
        .find(|wallpaper| wallpaper.id == wallpaper_id)
        .ok_or_else(|| "Wallpaper not found.".to_string())?;

    let mut desktop_applied = false;
    let message = if wallpaper.can_apply_to_desktop {
        apply_desktop_wallpaper(Path::new(&wallpaper.path))?;
        desktop_applied = true;
        "Wallpaper applied to the desktop.".to_string()
    } else {
        "Wallpaper selected in Lumina. This type cannot be applied directly as a static desktop wallpaper.".to_string()
    };

    config.active_wallpaper_id = Some(wallpaper.id.clone());
    save_config(&backend_root, &config)?;

    let mut wallpaper = wallpaper;
    wallpaper.active = true;

    Ok(ActiveResult {
        wallpaper,
        desktop_applied,
        message,
    })
}

#[tauri::command]
fn toggle_favorite(
    wallpaper_id: String,
    state: tauri::State<'_, RuntimeState>,
) -> Result<Wallpaper, String> {
    let backend_root = backend_root();
    ensure_directories(&backend_root)?;

    let mut config = state.config.lock().map_err(|_| "Config lock poisoned".to_string())?;
    let mut favorites: HashSet<String> = config.favorites.iter().cloned().collect();

    if favorites.contains(&wallpaper_id) {
        favorites.remove(&wallpaper_id);
    } else {
        favorites.insert(wallpaper_id.clone());
    }

    config.favorites = favorites.into_iter().collect();
    config.favorites.sort();
    save_config(&backend_root, &config)?;

    scan_wallpapers(&backend_root, &config)?
        .into_iter()
        .find(|wallpaper| wallpaper.id == wallpaper_id)
        .ok_or_else(|| "Wallpaper not found.".to_string())
}

#[tauri::command]
fn list_playlists(state: tauri::State<'_, RuntimeState>) -> Result<Vec<Playlist>, String> {
    let config = state
        .config
        .lock()
        .map_err(|_| "Config lock poisoned".to_string())?;
    Ok(config.playlists.clone())
}

#[tauri::command]
fn create_playlist(
    name: String,
    state: tauri::State<'_, RuntimeState>,
) -> Result<Playlist, String> {
    let backend_root = backend_root();
    ensure_directories(&backend_root)?;

    let name = name.trim();
    if name.is_empty() {
        return Err("Playlist name cannot be empty.".to_string());
    }

    let mut config = state
        .config
        .lock()
        .map_err(|_| "Config lock poisoned".to_string())?;
    let playlist = Playlist {
        id: playlist_id(name),
        name: name.to_string(),
        wallpaper_ids: Vec::new(),
        created_at: unix_timestamp(),
    };

    config.playlists.push(playlist.clone());
    save_config(&backend_root, &config)?;

    Ok(playlist)
}

#[tauri::command]
fn delete_playlist(
    playlist_id: String,
    state: tauri::State<'_, RuntimeState>,
) -> Result<Vec<Playlist>, String> {
    let backend_root = backend_root();
    ensure_directories(&backend_root)?;

    let mut config = state
        .config
        .lock()
        .map_err(|_| "Config lock poisoned".to_string())?;
    let initial_len = config.playlists.len();
    config.playlists.retain(|playlist| playlist.id != playlist_id);

    if config.playlists.len() == initial_len {
        return Err("Playlist not found.".to_string());
    }

    save_config(&backend_root, &config)?;
    Ok(config.playlists.clone())
}

#[tauri::command]
fn add_wallpaper_to_playlist(
    playlist_id: String,
    wallpaper_id: String,
    state: tauri::State<'_, RuntimeState>,
) -> Result<Playlist, String> {
    let backend_root = backend_root();
    ensure_directories(&backend_root)?;

    let mut config = state
        .config
        .lock()
        .map_err(|_| "Config lock poisoned".to_string())?;
    let wallpapers = scan_wallpapers(&backend_root, &config)?;

    if !wallpapers.iter().any(|wallpaper| wallpaper.id == wallpaper_id) {
        return Err("Wallpaper not found.".to_string());
    }

    let playlist = config
        .playlists
        .iter_mut()
        .find(|playlist| playlist.id == playlist_id)
        .ok_or_else(|| "Playlist not found.".to_string())?;

    if !playlist.wallpaper_ids.contains(&wallpaper_id) {
        playlist.wallpaper_ids.push(wallpaper_id);
    }

    let playlist = playlist.clone();
    save_config(&backend_root, &config)?;
    Ok(playlist)
}

#[tauri::command]
fn get_settings(state: tauri::State<'_, RuntimeState>) -> Result<AppSettings, String> {
    let config = state
        .config
        .lock()
        .map_err(|_| "Config lock poisoned".to_string())?;
    Ok(config.settings.clone())
}

#[tauri::command]
fn save_settings(
    settings: AppSettings,
    state: tauri::State<'_, RuntimeState>,
) -> Result<AppSettings, String> {
    let backend_root = backend_root();
    ensure_directories(&backend_root)?;

    let mut config = state
        .config
        .lock()
        .map_err(|_| "Config lock poisoned".to_string())?;
    config.settings = settings;
    save_config(&backend_root, &config)?;
    Ok(config.settings.clone())
}

fn build_status(
    backend_root: &Path,
    uptime_seconds: u64,
    wallpapers: Vec<Wallpaper>,
    config: &StoredConfig,
) -> BackendStatus {
    let active_wallpaper = config
        .active_wallpaper_id
        .as_ref()
        .and_then(|active_id| wallpapers.iter().find(|wallpaper| &wallpaper.id == active_id))
        .cloned();

    BackendStatus {
        ready: true,
        backend_root: path_to_string(backend_root),
        wallpaper_root: path_to_string(&wallpapers_root(backend_root)),
        config_path: path_to_string(&config_path(backend_root)),
        cache_root: path_to_string(&backend_root.join("cache")),
        log_root: path_to_string(&backend_root.join("logs")),
        total_wallpapers: wallpapers.len(),
        total_playlists: config.playlists.len(),
        active_wallpaper,
        uptime_seconds,
        supported_images: IMAGE_EXTENSIONS.to_vec(),
        supported_videos: VIDEO_EXTENSIONS.to_vec(),
    }
}

fn scan_wallpapers(backend_root: &Path, config: &StoredConfig) -> Result<Vec<Wallpaper>, String> {
    let root = wallpapers_root(backend_root);
    let favorite_ids: HashSet<&str> = config.favorites.iter().map(String::as_str).collect();
    let active_id = config.active_wallpaper_id.as_deref();
    let mut wallpapers = Vec::new();

    scan_folder(
        &root.join("images"),
        &root,
        WallpaperKind::Image,
        &favorite_ids,
        active_id,
        &mut wallpapers,
    )?;
    scan_folder(
        &root.join("videos"),
        &root,
        WallpaperKind::Video,
        &favorite_ids,
        active_id,
        &mut wallpapers,
    )?;
    scan_folder(
        &root.join("animated"),
        &root,
        WallpaperKind::Animated,
        &favorite_ids,
        active_id,
        &mut wallpapers,
    )?;

    wallpapers.sort_by(|left, right| left.title.to_lowercase().cmp(&right.title.to_lowercase()));
    Ok(wallpapers)
}

fn scan_folder(
    folder: &Path,
    wallpapers_root: &Path,
    default_kind: WallpaperKind,
    favorite_ids: &HashSet<&str>,
    active_id: Option<&str>,
    wallpapers: &mut Vec<Wallpaper>,
) -> Result<(), String> {
    if !folder.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(folder).map_err(to_error)? {
        let entry = entry.map_err(to_error)?;
        let path = entry.path();

        if path.is_dir() {
            scan_folder(
                &path,
                wallpapers_root,
                default_kind.clone(),
                favorite_ids,
                active_id,
                wallpapers,
            )?;
            continue;
        }

        if !path.is_file() {
            continue;
        }

        let kind = kind_from_path(&path, default_kind.clone());
        if kind == WallpaperKind::Unknown {
            continue;
        }

        let id = id_for_path(&path, wallpapers_root);
        let metadata = entry.metadata().map_err(to_error)?;
        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();

        wallpapers.push(Wallpaper {
            title: title_from_path(&path),
            relative_path: id.clone(),
            favorite: favorite_ids.contains(id.as_str()),
            active: active_id == Some(id.as_str()),
            can_apply_to_desktop: kind == WallpaperKind::Image
                && DESKTOP_IMAGE_EXTENSIONS.contains(&extension.as_str()),
            path: path_to_string(&path),
            size_bytes: metadata.len(),
            kind,
            id,
        });
    }

    Ok(())
}

fn kind_from_path(path: &Path, default_kind: WallpaperKind) -> WallpaperKind {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if IMAGE_EXTENSIONS.contains(&extension.as_str()) {
        WallpaperKind::Image
    } else if VIDEO_EXTENSIONS.contains(&extension.as_str()) {
        match default_kind {
            WallpaperKind::Animated => WallpaperKind::Animated,
            _ => WallpaperKind::Video,
        }
    } else {
        WallpaperKind::Unknown
    }
}

fn title_from_path(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("Untitled");
    let words = stem.replace(['_', '-'], " ");

    words
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn unique_destination(destination_dir: &Path, source: &Path) -> Result<PathBuf, String> {
    let file_name = source
        .file_name()
        .ok_or_else(|| "Source file has no file name.".to_string())?;
    let mut destination = destination_dir.join(file_name);

    if !destination.exists() {
        return Ok(destination);
    }

    let stem = source
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("wallpaper");
    let extension = source
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();

    for index in 2..1000 {
        let candidate = if extension.is_empty() {
            destination_dir.join(format!("{stem}-{index}"))
        } else {
            destination_dir.join(format!("{stem}-{index}.{extension}"))
        };

        if !candidate.exists() {
            destination = candidate;
            break;
        }
    }

    Ok(destination)
}

fn ensure_directories(backend_root: &Path) -> Result<(), String> {
    let directories = [
        backend_root.join("config"),
        backend_root.join("database"),
        backend_root.join("database").join("backups"),
        backend_root.join("logs"),
        backend_root.join("cache"),
        backend_root.join("cache").join("images"),
        backend_root.join("cache").join("previews"),
        backend_root.join("cache").join("temp"),
        backend_root.join("cache").join("thumbnails"),
        backend_root.join("cache").join("videos"),
        wallpapers_root(backend_root),
        wallpapers_root(backend_root).join("animated"),
        wallpapers_root(backend_root).join("images"),
        wallpapers_root(backend_root).join("playlists"),
        wallpapers_root(backend_root).join("videos"),
    ];

    for directory in directories {
        fs::create_dir_all(directory).map_err(to_error)?;
    }

    Ok(())
}

fn load_config(backend_root: &Path) -> Result<StoredConfig, String> {
    let path = config_path(backend_root);
    if !path.exists() {
        return Ok(StoredConfig::default());
    }

    let content = fs::read_to_string(path).map_err(to_error)?;
    serde_json::from_str(&content).map_err(to_error)
}

fn save_config(backend_root: &Path, config: &StoredConfig) -> Result<(), String> {
    let path = config_path(backend_root);
    let content = serde_json::to_string_pretty(config).map_err(to_error)?;
    fs::write(path, content).map_err(to_error)
}

fn backend_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn wallpapers_root(backend_root: &Path) -> PathBuf {
    backend_root.join("wallpapers")
}

fn config_path(backend_root: &Path) -> PathBuf {
    backend_root.join("config").join("state.json")
}

fn id_for_path(path: &Path, wallpapers_root: &Path) -> String {
    path.strip_prefix(wallpapers_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn playlist_id(name: &str) -> String {
    let slug = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    format!("{}-{}", slug, unix_timestamp())
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn to_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(target_os = "windows")]
fn apply_desktop_wallpaper(path: &Path) -> Result<(), String> {
    use std::{ffi::c_void, os::windows::ffi::OsStrExt};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_SETDESKWALLPAPER, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE,
    };

    let mut wide_path: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide_path.push(0);

    let result = unsafe {
        SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            wide_path.as_ptr() as *mut c_void,
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        )
    };

    if result == 0 {
        Err(format!(
            "Windows could not apply the wallpaper: {}",
            std::io::Error::last_os_error()
        ))
    } else {
        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
fn apply_desktop_wallpaper(_path: &Path) -> Result<(), String> {
    Err("Applying desktop wallpapers is only implemented for Windows.".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let backend_root = backend_root();
            ensure_directories(&backend_root)
                .map_err(|error| Box::<dyn std::error::Error>::from(error))?;
            let config = load_config(&backend_root).unwrap_or_default();

            app.manage(RuntimeState {
                started_at: Instant::now(),
                config: Mutex::new(config),
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_backend_status,
            list_wallpapers,
            import_wallpaper,
            set_active_wallpaper,
            toggle_favorite,
            list_playlists,
            create_playlist,
            delete_playlist,
            add_wallpaper_to_playlist,
            get_settings,
            save_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
