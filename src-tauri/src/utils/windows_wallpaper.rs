use windows::{
    core::*,
    Win32::{System::Com::*, UI::Shell::*},
};

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
