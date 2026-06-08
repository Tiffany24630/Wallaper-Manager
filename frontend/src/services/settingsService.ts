import { tauriInvoke } from "./tauri";
import type { Settings } from "../types/settings";

export const settingsService = {
  async get(): Promise<Settings> {
    return tauriInvoke<Settings>("get_settings");
  },

  async save(settings: Settings): Promise<void> {
    return tauriInvoke("save_settings", {
      settings,
    });
  },
};