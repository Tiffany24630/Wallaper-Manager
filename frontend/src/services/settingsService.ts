import { tauriInvoke } from "./tauri";
import type { Settings } from "../types/settings";

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
    return tauriInvoke<Settings>("get_settings");
  },

  async save(settings: Settings): Promise<void> {
    return tauriInvoke("save_settings", {
      settings,
    });
  },

  async diagnostic(): Promise<DynamicConfigurationDiagnostic> {
    return tauriInvoke<DynamicConfigurationDiagnostic>("get_dynamic_configuration_diagnostic");
  },
};
