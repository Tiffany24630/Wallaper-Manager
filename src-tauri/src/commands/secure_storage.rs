use std::sync::{Mutex, MutexGuard, OnceLock};

const TARGET: &str = "Lumina/session-token";
static SESSION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn credential_target() -> String {
    match crate::db::smoke_token() {
        Ok(Some(token)) => format!("{TARGET}/smoke/{token}"),
        _ => TARGET.to_string(),
    }
}

pub(crate) fn session_lock() -> Result<MutexGuard<'static, ()>, String> {
    SESSION_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "El estado de sesión está bloqueado".to_string())
}

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[tauri::command]
pub fn get_secure_session_token() -> Result<Option<String>, String> {
    let _lock = session_lock()?;
    get_secure_session_token_unlocked()
}

pub(crate) fn get_secure_session_token_unlocked() -> Result<Option<String>, String> {
    #[cfg(windows)]
    unsafe {
        use std::{ptr::null_mut, slice};
        use windows::core::PCWSTR;
        use windows::Win32::Security::Credentials::{
            CredFree, CredReadW, CREDENTIALW, CRED_TYPE_GENERIC,
        };
        let target = wide(&credential_target());
        let mut credential: *mut CREDENTIALW = null_mut();
        if CredReadW(
            PCWSTR(target.as_ptr()),
            CRED_TYPE_GENERIC,
            0,
            &mut credential,
        )
        .is_err()
        {
            return Ok(None);
        }
        let result =
            if (*credential).CredentialBlob.is_null() || (*credential).CredentialBlobSize == 0 {
                None
            } else {
                let bytes = slice::from_raw_parts(
                    (*credential).CredentialBlob,
                    (*credential).CredentialBlobSize as usize,
                );
                Some(
                    String::from_utf8(bytes.to_vec())
                        .map_err(|_| "El token almacenado no es válido".to_string())?,
                )
            };
        CredFree(credential as _);
        return Ok(result);
    }
    #[cfg(not(windows))]
    {
        Err("El almacenamiento seguro de sesiones solo está disponible en Windows".into())
    }
}

#[tauri::command]
pub fn set_secure_session_token(token: String) -> Result<(), String> {
    let _lock = session_lock()?;
    if token.is_empty() || token.len() > 4096 {
        return Err("Token de sesión no válido".into());
    }
    #[cfg(windows)]
    unsafe {
        use std::ptr::null_mut;
        use windows::core::PWSTR;
        use windows::Win32::Foundation::FILETIME;
        use windows::Win32::Security::Credentials::{
            CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
        };
        let mut target = wide(&credential_target());
        let mut bytes = token.into_bytes();
        let credential = CREDENTIALW {
            Flags: windows::Win32::Security::Credentials::CRED_FLAGS(0),
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target.as_mut_ptr()),
            Comment: PWSTR::null(),
            LastWritten: FILETIME::default(),
            CredentialBlobSize: bytes.len() as u32,
            CredentialBlob: bytes.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            AttributeCount: 0,
            Attributes: null_mut(),
            TargetAlias: PWSTR::null(),
            UserName: PWSTR::null(),
        };
        CredWriteW(&credential, 0)
            .map_err(|e| format!("No se pudo guardar la sesión de forma segura: {e}"))?;
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let _ = token;
        Err("El almacenamiento seguro de sesiones solo está disponible en Windows".into())
    }
}

#[tauri::command]
pub fn delete_secure_session_token() -> Result<(), String> {
    let _lock = session_lock()?;
    #[cfg(windows)]
    unsafe {
        use windows::core::PCWSTR;
        use windows::Win32::Security::Credentials::{CredDeleteW, CRED_TYPE_GENERIC};
        let target = wide(&credential_target());
        let _ = CredDeleteW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, 0);
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        Err("El almacenamiento seguro de sesiones solo está disponible en Windows".into())
    }
}
