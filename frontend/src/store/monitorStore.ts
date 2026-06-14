import { create } from "zustand";
import type {Monitor} from "../types/monitor";
import {monitorService} from "../services/monitorService";

interface MonitorState {
  monitors: Monitor[];
  loading: boolean;
  error: string | null;

  loadMonitors: () => Promise<void>;
  assignWallpaper: (monitorId: string, wallpaperId: string) => Promise<void>;
}

export const useMonitorStore =
  create<MonitorState>((set) => ({
    monitors: [],
    loading: false,
    error: null,

    loadMonitors: async () => {
      try {
        set({
          loading: true,
        });

        const monitors = await monitorService.getAll();

        set({
          monitors,
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

    assignWallpaper: async (
      monitorId,
      wallpaperId
    ) => {
      await monitorService.assignWallpaper(monitorId, wallpaperId);
      const monitors = await monitorService.getAll();
      set({monitors,});
    },
}));