import { create } from "zustand";
import type { Settings } from "../types/settings";
import { settingsService } from "../services/settingsService";

interface SettingsState {
  settings: Settings | null;
  loading: boolean;
  error: string | null;
  
  loadSettings: () => Promise<void>;
  saveSettings: (settings: Settings) => Promise<void>;
}

export const useSettingsStore = create<SettingsState>((set) => ({
  settings: null,
  loading: false,
  error: null,

  loadSettings: async () => {
    try {
      set({
        loading: true,
      });

      const settings = await settingsService.get();

      set({
        settings,
        loading: false,
        error: null,
      });
    } catch (error) {
      set({
        loading: false,
        error: String(error),
      });
    }
  },

  saveSettings: async (settings) => {
    try {
      await settingsService.save(settings);

      set({
        settings,
      });
    } catch (error) {
      set({
        error: String(error),
      });
      throw error;
    }
  },
}));
