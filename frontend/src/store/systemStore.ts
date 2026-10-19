import { create } from "zustand";
import type {SystemInfo} from "../types/system";
import {systemService} from "../services/systemService";

let pollingId: ReturnType<typeof setInterval> | null = null;
let refreshInFlight = false;
let refreshGeneration = 0;

interface SystemState {
  systemInfo: SystemInfo | null;
  loading: boolean;
  error: string | null;

  refresh: () => Promise<void>;
  startPolling: () => void;
  stopPolling: () => void;
}

export const useSystemStore =
  create<SystemState>((set, get) => ({
    systemInfo: null,
    loading: false,
    error: null,

    refresh: async () => {
      if (refreshInFlight) return;
      refreshInFlight = true;
      const generation = ++refreshGeneration;
      try {
        set({
          loading: true,
        });

        const info = await systemService.getInfo();

        if (generation === refreshGeneration) {
          set({
            systemInfo: info,
            loading: false,
            error: null,
          });
        }

      } catch (error) {
        if (generation === refreshGeneration) {
          set({
            loading: false,
            error: String(error),
          });
        }
      } finally {
        refreshInFlight = false;
      }
    },

    startPolling: () => {
      if (pollingId)
        return;

      get().refresh();

      pollingId = setInterval(() => {
          get().refresh();
        }, 5000);
    },

    stopPolling: () => {
      if (pollingId) {
        clearInterval(pollingId);

        pollingId = null;
      }
    },
}));
