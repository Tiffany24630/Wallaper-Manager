const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const VALUE_NAME: &str = "Lumina";

#[cfg(target_os = "windows")]
pub fn set_enabled(enabled: bool) -> Result<(), String> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegSetValueExW, HKEY_CURRENT_USER,
        KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
    };

    let key_path: Vec<u16> = RUN_KEY.encode_utf16().chain([0]).collect();
    let value_name: Vec<u16> = VALUE_NAME.encode_utf16().chain([0]).collect();
    let mut key = HKEY_CURRENT_USER;
    let result = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key_path.as_ptr()),
            0,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut key,
            None,
        )
    };
    if result.is_err() {
        return Err(format!(
            "No se pudo abrir la configuración de inicio de Windows: {result:?}"
        ));
    }

    let operation = if enabled {
        let exe = std::env::current_exe().map_err(|error| {
            format!("No se pudo localizar Lumina para el inicio automático: {error}")
        })?;
        let command = format!("\"{}\"", exe.display());
        let data: Vec<u16> = command.encode_utf16().chain([0]).collect();
        let result = unsafe {
            RegSetValueExW(
                key,
                PCWSTR(value_name.as_ptr()),
                0,
                REG_SZ,
                Some(std::slice::from_raw_parts(
                    data.as_ptr() as *const u8,
                    data.len() * std::mem::size_of::<u16>(),
                )),
            )
        };
        result.ok().map_err(|error| format!("{error:?}"))
    } else {
        let result = unsafe { RegDeleteValueW(key, PCWSTR(value_name.as_ptr())) };
        if result.is_ok() || result.0 == 2 {
            Ok(())
        } else {
            Err(format!("{result:?}"))
        }
    };
    unsafe {
        let _ = RegCloseKey(key);
    };
    operation
        .map_err(|error| format!("No se pudo actualizar el inicio automático de Lumina: {error}"))
}

#[cfg(target_os = "windows")]
pub fn is_enabled() -> Result<bool, String> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE,
    };

    let key_path: Vec<u16> = RUN_KEY.encode_utf16().chain([0]).collect();
    let value_name: Vec<u16> = VALUE_NAME.encode_utf16().chain([0]).collect();
    let mut key = HKEY::default();
    let result = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key_path.as_ptr()),
            0,
            KEY_QUERY_VALUE,
            &mut key,
        )
    };
    if result.0 == 2 {
        return Ok(false);
    }
    if result.is_err() {
        return Err(format!(
            "No se pudo comprobar el inicio automático de Lumina: {result:?}"
        ));
    }

    let mut size = 0u32;
    let query = unsafe {
        RegQueryValueExW(
            key,
            PCWSTR(value_name.as_ptr()),
            None,
            None,
            None,
            Some(&mut size),
        )
    };
    if query.0 == 2 {
        unsafe {
            let _ = RegCloseKey(key);
        }
        return Ok(false);
    }
    if query.is_err() || size == 0 {
        unsafe {
            let _ = RegCloseKey(key);
        }
        return Err(format!(
            "No se pudo comprobar el inicio automático de Lumina: {query:?}"
        ));
    }
    let mut data = vec![0u8; size as usize];
    let query = unsafe {
        RegQueryValueExW(
            key,
            PCWSTR(value_name.as_ptr()),
            None,
            None,
            Some(data.as_mut_ptr()),
            Some(&mut size),
        )
    };
    unsafe {
        let _ = RegCloseKey(key);
    }
    if query.is_err() {
        return Err(format!(
            "No se pudo comprobar el inicio automático de Lumina: {query:?}"
        ));
    }
    Ok(size > 0)
}

#[cfg(not(target_os = "windows"))]
pub fn set_enabled(_enabled: bool) -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn is_enabled() -> Result<bool, String> {
    Ok(false)
}
