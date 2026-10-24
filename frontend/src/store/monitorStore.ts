import { create } from "zustand";
import type {Monitor} from "../types/monitor";
import {monitorService} from "../services/monitorService";

let monitorLoadSequence = 0;

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
      const requestId = ++monitorLoadSequence;
      try {
        set({
          loading: true,
        });

        const monitors = await monitorService.getAll();

        if (requestId !== monitorLoadSequence) return;

        set({
          monitors,
          loading: false,
          error: null,
        });

      } catch (error) {
        if (requestId !== monitorLoadSequence) return;
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
      const requestId = ++monitorLoadSequence;
      const monitors = await monitorService.getAll();
      if (requestId === monitorLoadSequence) set({monitors,});
    },
}));
