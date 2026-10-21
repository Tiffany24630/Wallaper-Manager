#[cfg(target_os = "windows")]
use windows::{
    core::*,
    Win32::{
        Devices::Display::{
            DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QueryDisplayConfig,
            DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
            DISPLAYCONFIG_DEVICE_INFO_HEADER, DISPLAYCONFIG_PATH_INFO,
            DISPLAYCONFIG_SOURCE_DEVICE_NAME, DISPLAYCONFIG_TARGET_DEVICE_NAME,
            QDC_ONLY_ACTIVE_PATHS,
        },
        Foundation::{ERROR_SUCCESS, E_INVALIDARG},
        System::Com::*,
        UI::Shell::*,
    },
};

use crate::models::monitor_assignment::{MonitorGeometry, NativeMonitorDescriptor};

#[cfg(target_os = "windows")]
fn wide_string(value: &[u16]) -> Option<String> {
    let length = value
        .iter()
        .position(|item| *item == 0)
        .unwrap_or(value.len());
    if length == 0 {
        return None;
    }
    String::from_utf16(&value[..length]).ok()
}

#[cfg(target_os = "windows")]
pub fn native_monitor_descriptors() -> Result<Vec<NativeMonitorDescriptor>> {
    unsafe {
        let mut path_count = 0;
        let mut mode_count = 0;
        let status =
            GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count);
        if status != ERROR_SUCCESS {
            return Err(Error::from_win32());
        }
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
        let mut modes = vec![
            windows::Win32::Devices::Display::DISPLAYCONFIG_MODE_INFO::default();
            mode_count as usize
        ];
        let status = QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut path_count,
            paths.as_mut_ptr(),
            &mut mode_count,
            modes.as_mut_ptr(),
            None,
        );
        if status != ERROR_SUCCESS {
            return Err(Error::from_win32());
        }

        let mut descriptors = Vec::with_capacity(path_count as usize);
        for path in paths.into_iter().take(path_count as usize) {
            if !path.targetInfo.targetAvailable.as_bool() {
                continue;
            }
            let source_header = DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                size: std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                adapterId: path.sourceInfo.adapterId,
                id: path.sourceInfo.id,
            };
            let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
                header: source_header,
                ..Default::default()
            };
            if DisplayConfigGetDeviceInfo(&mut source.header) != ERROR_SUCCESS.0 as i32 {
                continue;
            }

            let target_header = DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
                size: std::mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32,
                adapterId: path.targetInfo.adapterId,
                id: path.targetInfo.id,
            };
            let mut target = DISPLAYCONFIG_TARGET_DEVICE_NAME {
                header: target_header,
                ..Default::default()
            };
            if DisplayConfigGetDeviceInfo(&mut target.header) != ERROR_SUCCESS.0 as i32 {
                continue;
            }

            let source_name = wide_string(&source.viewGdiDeviceName);
            let device_path = wide_string(&target.monitorDevicePath);
            let Some(device_path) = device_path else {
                continue;
            };
            let geometry = match path.sourceInfo.Anonymous.modeInfoIdx {
                index if (index as usize) < modes.len() => {
                    let mode = modes[index as usize];
                    if mode.infoType
                        == windows::Win32::Devices::Display::DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE
                    {
                        let source_mode = mode.Anonymous.sourceMode;
                        MonitorGeometry::new(
                            source_mode.position.x,
                            source_mode.position.y,
                            source_mode.width,
                            source_mode.height,
                        )
                        .ok()
                    } else {
                        None
                    }
                }
                _ => None,
            };
            descriptors.push(NativeMonitorDescriptor {
                device_path,
                source_name,
                geometry,
            });
        }
        Ok(descriptors)
    }
}

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
pub fn native_monitor_descriptors() -> Result<Vec<NativeMonitorDescriptor>, String> {
    Err("Los descriptores nativos de monitor solo estÃ¡n disponibles en Windows".into())
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
