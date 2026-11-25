import { tauriInvoke } from "./tauri";
import type { Settings } from "../types/settings";
import { isTauri } from "./tauri";

const webDefaults: Settings = { launch_at_startup: false, minimize_to_tray: false, hardware_acceleration: true, pause_on_battery: false, pause_when_maximized: false, scaling_mode: "fill", accent_color: "violet", ui_scale: 100, dynamic_enabled: false, dynamic_playlist_id: null, dynamic_interval_minutes: 5, dynamic_current_wallpaper_id: null, dynamic_last_error: null, cursor_enabled: false, active_cursor_id: null };

export type DynamicConfigurationDiagnostic = {
  consistent: boolean;
  requires_decision: boolean;
  global_enabled: boolean;
  global_playlist_id: string | null;
  monitor_assignment_count: number;
  issues: { code: string; message: string; blocking: boolean }[];
};

export const settingsService = {
  async get(): Promise<Settings> {
    if (!isTauri()) return { ...webDefaults, ...JSON.parse(localStorage.getItem("lumina.web.settings") || "{}") };
    return tauriInvoke<Settings>("get_settings");
  },

  async save(settings: Settings): Promise<void> {
    if (!isTauri()) { localStorage.setItem("lumina.web.settings", JSON.stringify(settings)); document.documentElement.dataset.accent = settings.accent_color; document.documentElement.style.fontSize = `${settings.ui_scale}%`; return; }
    return tauriInvoke("save_settings", {
      settings,
    });
  },

  async diagnostic(): Promise<DynamicConfigurationDiagnostic> {
    if (!isTauri()) return { consistent: true, requires_decision: false, global_enabled: false, global_playlist_id: null, monitor_assignment_count: 0, issues: [] };
    return tauriInvoke<DynamicConfigurationDiagnostic>("get_dynamic_configuration_diagnostic");
  },
};
