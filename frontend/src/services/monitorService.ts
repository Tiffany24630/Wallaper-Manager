import { tauriInvoke } from "./tauri";
import type {Monitor} from "../types/monitor";
import type {MonitorPlaylist} from "../types/monitor";
import { isTauri } from "./tauri";

export interface MonitorWallpaper {
  monitorId: string;
  wallpaperId: string;
}

export const monitorService = {
  async getAll(): Promise<Monitor[]> {
    if (!isTauri()) return [{ id: "browser", name: "Ventana del navegador", width: window.innerWidth, height: window.innerHeight, refreshRate: 60, primary: true }];
    return tauriInvoke<Monitor[]>(
      "get_monitors"
    );
  },

  async assignWallpaper(
    monitorId: string,
    wallpaperId: string
  ): Promise<void> {
    if (!isTauri()) return;
    return tauriInvoke(
      "assign_wallpaper_to_monitor",
      {
        monitorId,
        wallpaperId,
      }
    );
  },

  async getPlaylists(): Promise<MonitorPlaylist[]> {
    if (!isTauri()) return [];
    return tauriInvoke<MonitorPlaylist[]>("get_monitor_playlists");
  },

  async getWallpapers(): Promise<MonitorWallpaper[]> {
    if (!isTauri()) { const id = localStorage.getItem("lumina.web.activeWallpaper"); return id ? [{ monitorId: "browser", wallpaperId: id }] : []; }
    return tauriInvoke<MonitorWallpaper[]>("get_monitor_wallpapers");
  },

  async assignPlaylist(monitorId: string, playlistId: string): Promise<void> {
    if (!isTauri()) return;
    return tauriInvoke("assign_playlist_to_monitor", { monitorId, playlistId });
  },

  async removePlaylist(monitorId: string): Promise<void> {
    if (!isTauri()) return;
    return tauriInvoke("remove_playlist_from_monitor", { monitorId });
  },

};
