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
let queuedSaves = 0;
let saveQueue: Promise<void> = Promise.resolve();

export const useSettingsStore = create<SettingsState>((set) => ({
  settings: null,
  loading: false,
  error: null,

  loadSettings: async () => {
    if (saveInFlight || queuedSaves > 0) return;
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
    const version = ++requestVersion;
    queuedSaves += 1;
    const operation = saveQueue.catch(() => undefined).then(async () => {
      saveInFlight = true;
      set({ loading: true, error: null });
      try {
        await settingsService.save(settings);
        const persistedSettings = await settingsService.get();

        if (version === requestVersion) set({ settings: persistedSettings, loading: false, error: null });
      } catch (error) {
        if (version === requestVersion) set({ loading: false, error: String(error) });
        throw error;
      } finally {
        saveInFlight = false;
        queuedSaves -= 1;
        if (version === requestVersion) set({ loading: false });
      }
    });
    saveQueue = operation.catch(() => undefined);
    return operation;
  },
}));
