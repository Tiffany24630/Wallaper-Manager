import { tauriInvoke } from "./tauri";
import type { Playlist } from "../types/playlist";

export const playlistService = {
  async getAll(): Promise<Playlist[]> {
    return tauriInvoke<Playlist[]>("get_playlists");
  },

  async create(name: string): Promise<void> {
    return tauriInvoke("create_playlist", {
      name,
    });
  },

  async delete(id: string): Promise<void> {
    return tauriInvoke("delete_playlist", {
      playlistId: id,
    });
  },

  addWallpaper(playlistId: string, wallpaperId: string): Promise<void> {
    return tauriInvoke("add_wallpaper_to_playlist", {
      playlistId,
      wallpaperId,
    });
  },

  getPlaylistWallpapers(playlistId: string): Promise<string[]> {
    return tauriInvoke<string[]>("get_playlist_wallpapers", {
      playlistId,
    });
  },
};