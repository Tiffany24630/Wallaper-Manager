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

let requestVersion = 0;
let saveInFlight = false;

export const useSettingsStore = create<SettingsState>((set) => ({
  settings: null,
  loading: false,
  error: null,

  loadSettings: async () => {
    if (saveInFlight) return;
    const version = ++requestVersion;
    try {
      set({ loading: true });

      const settings = await settingsService.get();

      if (version !== requestVersion || saveInFlight) return;
      set({ settings, loading: false, error: null });
    } catch (error) {
      if (version !== requestVersion || saveInFlight) return;
      set({ loading: false, error: String(error) });
    }
  },

  saveSettings: async (settings) => {
    saveInFlight = true;
    const version = ++requestVersion;
    set({ loading: true, error: null });
    try {
      await settingsService.save(settings);

      if (version === requestVersion) set({ settings, loading: false, error: null });
    } catch (error) {
      if (version === requestVersion) set({ loading: false, error: String(error) });
      throw error;
    } finally {
      saveInFlight = false;
      if (version === requestVersion) set({ loading: false });
    }
  },
}));
