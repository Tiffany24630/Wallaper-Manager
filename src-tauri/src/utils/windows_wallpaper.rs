#[cfg(target_os = "windows")]
use windows::{
    core::*,
    Win32::{System::Com::*, UI::Shell::*},
};

#[cfg(target_os = "windows")]
pub fn set_monitor_wallpaper(monitor_id: &str, wallpaper_path: &str) -> Result<()> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let result = (|| {
            let desktop: IDesktopWallpaper = CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL)?;
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
