use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingClass {
    Portable,
    DeviceSpecific,
    WindowsSpecific,
    Derived,
}

pub fn classify_setting(name: &str) -> Option<SettingClass> {
    match name {
        "dynamic_enabled" | "dynamic_playlist_id" | "dynamic_interval_minutes" | "scaling_mode"
        | "accent_color" => Some(SettingClass::Portable),
        "hardware_acceleration" | "pause_on_battery" | "ui_scale" => {
            Some(SettingClass::DeviceSpecific)
        }
        "launch_at_startup" | "minimize_to_tray" | "pause_when_maximized"
        | "cursor_enabled" | "active_cursor_id" => Some(SettingClass::WindowsSpecific),
        "dynamic_current_wallpaper_id" | "dynamic_last_error" => Some(SettingClass::Derived),
        _ => None,
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    pub launch_at_startup: bool,
    pub minimize_to_tray: bool,
    pub hardware_acceleration: bool,
    pub pause_on_battery: bool,
    pub pause_when_maximized: bool,
    pub scaling_mode: String,
    pub accent_color: String,
    pub ui_scale: u16,
    pub dynamic_enabled: bool,
    pub dynamic_playlist_id: Option<String>,
    pub dynamic_interval_minutes: u32,
    pub dynamic_current_wallpaper_id: Option<String>,
    pub dynamic_last_error: Option<String>,
    pub cursor_enabled: bool,
    pub active_cursor_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{classify_setting, SettingClass};

    #[test]
    fn settings_contract_classifies_configuration_and_derived_state() {
        assert_eq!(classify_setting("dynamic_interval_minutes"), Some(SettingClass::Portable));
        assert_eq!(classify_setting("launch_at_startup"), Some(SettingClass::WindowsSpecific));
        assert_eq!(classify_setting("dynamic_last_error"), Some(SettingClass::Derived));
        assert_eq!(classify_setting("unknown"), None);
    }
}
