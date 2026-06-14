import { create } from "zustand";
import type {SystemInfo} from "../types/system";
import {systemService} from "../services/systemService";

interface SystemState {
  systemInfo: SystemInfo | null;
  loading: boolean;
  error: string | null;

  refresh: () => Promise<void>;
  startPolling: () => void;
}

export const useSystemStore =
  create<SystemState>((set, get) => ({
    systemInfo: null,
    loading: false,
    error: null,

    refresh: async () => {
      try {
        set({
          loading: true,
        });

        const info = await systemService.getInfo();

        set({
          systemInfo: info,
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

    startPolling: () => {
      get().refresh();

      setInterval(() => {
        get().refresh();
      }, 5000);
    },
}));