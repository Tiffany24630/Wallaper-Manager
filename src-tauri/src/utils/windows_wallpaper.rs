#[cfg(target_os = "windows")]
use windows::{
    core::*,
    Win32::{Foundation::E_INVALIDARG, System::Com::*, UI::Shell::*},
};

#[cfg(target_os = "windows")]
pub fn available_monitor_ids() -> Result<Vec<String>> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let result = (|| {
            let desktop: IDesktopWallpaper = CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL)?;
            let monitor_count = desktop.GetMonitorDevicePathCount()?;
            let mut ids = Vec::with_capacity(monitor_count as usize);
            for index in 0..monitor_count {
                let path = desktop.GetMonitorDevicePathAt(index)?;
                let id = path.to_string()?;
                CoTaskMemFree(Some(path.0 as _));
                ids.push(id);
            }
            Ok(ids)
        })();
        CoUninitialize();
        result
    }
}

#[cfg(not(target_os = "windows"))]
pub fn available_monitor_ids() -> Result<Vec<String>, String> {
    Err("Los identificadores nativos de monitor solo están disponibles en Windows".into())
}

#[cfg(target_os = "windows")]
pub fn set_monitor_wallpaper(monitor_id: &str, wallpaper_path: &str) -> Result<()> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let result = (|| {
            let desktop: IDesktopWallpaper = CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL)?;
            let monitor_count = desktop.GetMonitorDevicePathCount()?;
            let mut known_monitor = false;
            for index in 0..monitor_count {
                let path = desktop.GetMonitorDevicePathAt(index)?;
                let matches = path
                    .to_string()
                    .map(|value| value == monitor_id)
                    .unwrap_or(false);
                CoTaskMemFree(Some(path.0 as _));
                if matches {
                    known_monitor = true;
                    break;
                }
            }
            if !known_monitor {
                return Err(Error::new(E_INVALIDARG, "El monitor ya no está disponible"));
            }
            let monitor_hstring = HSTRING::from(monitor_id);
            let wallpaper_hstring = HSTRING::from(wallpaper_path);
            desktop.SetWallpaper(&monitor_hstring, &wallpaper_hstring)
        })();
        CoUninitialize();
        result?;
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn set_monitor_wallpaper(_monitor_id: &str, _wallpaper_path: &str) -> Result<(), String> {
    Err("La asignación por monitor solo está disponible en Windows".into())
}
