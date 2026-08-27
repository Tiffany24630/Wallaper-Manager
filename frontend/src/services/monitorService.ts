import { tauriInvoke } from "./tauri";
import type {Monitor} from "../types/monitor";

export const monitorService = {
  async getAll(): Promise<Monitor[]> {
    return tauriInvoke<Monitor[]>(
      "get_monitors"
    );
  },

  async assignWallpaper(
    monitorId: string,
    wallpaperId: string
  ): Promise<void> {
    return tauriInvoke(
      "assign_wallpaper_to_monitor",
      {
        monitorId,
        wallpaperId,
      }
    );
  },

};
