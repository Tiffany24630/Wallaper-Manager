import { tauriInvoke } from "./tauri";
import type {Monitor} from "../types/monitor";
import type {MonitorPlaylist} from "../types/monitor";

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

  async getPlaylists(): Promise<MonitorPlaylist[]> {
    return tauriInvoke<MonitorPlaylist[]>("get_monitor_playlists");
  },

  async assignPlaylist(monitorId: string, playlistId: string): Promise<void> {
    return tauriInvoke("assign_playlist_to_monitor", { monitorId, playlistId });
  },

  async removePlaylist(monitorId: string): Promise<void> {
    return tauriInvoke("remove_playlist_from_monitor", { monitorId });
  },

};
