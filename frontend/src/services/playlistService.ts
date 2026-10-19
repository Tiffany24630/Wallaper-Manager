import { tauriInvoke } from "./tauri";
import type { Playlist } from "../types/playlist";
import { notifyLocalChange } from "./syncQueueService";

export const playlistService = {
  async getAll(): Promise<Playlist[]> {
    return tauriInvoke<Playlist[]>("get_playlists");
  },

  async create(name: string): Promise<string> {
    const id = await tauriInvoke<string>("create_playlist", {
      name,
    });
    await tauriInvoke("mark_playlist_local_change", { playlistId: id, deleted: false });
    await notifyLocalChange("playlist_update", id);
    return id;
  },

  async delete(id: string): Promise<void> {
    await tauriInvoke("delete_playlist", {
      playlistId: id,
    });
    await tauriInvoke("mark_playlist_local_change", { playlistId: id, deleted: true });
    await notifyLocalChange("playlist_delete", id);
  },

  async addWallpaper(playlistId: string, wallpaperId: string): Promise<void> {
    await tauriInvoke("add_wallpaper_to_playlist", {
      playlistId,
      wallpaperId,
    });
    await tauriInvoke("mark_playlist_local_change", { playlistId, deleted: false });
    await notifyLocalChange("playlist_item_update", playlistId);
  },

  getPlaylistWallpapers(playlistId: string): Promise<string[]> {
    return tauriInvoke<string[]>("get_playlist_wallpapers", {
      playlistId,
    });
  },

  async removeWallpaper(playlistId: string, wallpaperId: string): Promise<void> {
    await tauriInvoke("remove_wallpaper_from_playlist", { playlistId, wallpaperId });
    await tauriInvoke("mark_playlist_local_change", { playlistId, deleted: false });
    await notifyLocalChange("playlist_item_update", playlistId);
  },
};
