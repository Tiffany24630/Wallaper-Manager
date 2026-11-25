import { tauriInvoke } from "./tauri";
import type { Playlist } from "../types/playlist";
import { notifyLocalChange } from "./syncQueueService";
import { isTauri } from "./tauri";

type WebPlaylist = Playlist & { wallpaperIds: string[] };
const webPlaylists = (): WebPlaylist[] => JSON.parse(localStorage.getItem("lumina.web.playlists") || "[]") as WebPlaylist[];
const saveWebPlaylists = (items: WebPlaylist[]) => localStorage.setItem("lumina.web.playlists", JSON.stringify(items));

export const playlistService = {
  async getAll(): Promise<Playlist[]> {
    if (!isTauri()) return webPlaylists();
    return tauriInvoke<Playlist[]>("get_playlists");
  },

  async create(name: string): Promise<string> {
    if (!isTauri()) { const items = webPlaylists(); const id = crypto.randomUUID(); items.push({ id, name: name.trim(), createdAt: new Date().toISOString(), wallpaperCount: 0, wallpaperIds: [] }); saveWebPlaylists(items); return id; }
    const id = await tauriInvoke<string>("create_playlist", {
      name,
    });
    await tauriInvoke("mark_playlist_local_change", { playlistId: id, deleted: false });
    await notifyLocalChange("playlist_update", id);
    return id;
  },

  async rename(id: string, name: string): Promise<void> {
    if (!isTauri()) { const items = webPlaylists(); const item = items.find(value => value.id === id); if (item) item.name = name.trim(); saveWebPlaylists(items); return; }
    await tauriInvoke("rename_playlist", { playlistId: id, name });
    await tauriInvoke("mark_playlist_local_change", { playlistId: id, deleted: false });
    await notifyLocalChange("playlist_update", id);
  },

  async delete(id: string): Promise<void> {
    if (!isTauri()) { saveWebPlaylists(webPlaylists().filter(item => item.id !== id)); return; }
    await tauriInvoke("delete_playlist", {
      playlistId: id,
    });
    await tauriInvoke("mark_playlist_local_change", { playlistId: id, deleted: true });
    await notifyLocalChange("playlist_delete", id);
  },

  async addWallpaper(playlistId: string, wallpaperId: string): Promise<void> {
    if (!isTauri()) { const items = webPlaylists(); const item = items.find(value => value.id === playlistId); if (item && !item.wallpaperIds.includes(wallpaperId)) { item.wallpaperIds.push(wallpaperId); item.wallpaperCount = item.wallpaperIds.length; } saveWebPlaylists(items); return; }
    await tauriInvoke("add_wallpaper_to_playlist", {
      playlistId,
      wallpaperId,
    });
    await tauriInvoke("mark_playlist_local_change", { playlistId, deleted: false });
    await notifyLocalChange("playlist_item_update", playlistId);
  },

  getPlaylistWallpapers(playlistId: string): Promise<string[]> {
    if (!isTauri()) return Promise.resolve(webPlaylists().find(item => item.id === playlistId)?.wallpaperIds ?? []);
    return tauriInvoke<string[]>("get_playlist_wallpapers", {
      playlistId,
    });
  },

  async applyRemote(playlistId: string, name: string, wallpaperIds: string[]): Promise<void> {
    if (isTauri()) return;
    const items = webPlaylists();
    const existing = items.find(item => item.id === playlistId);
    const normalizedIds = [...new Set(wallpaperIds)];
    if (existing) { existing.name = name.trim() || existing.name; existing.wallpaperIds = normalizedIds; existing.wallpaperCount = normalizedIds.length; }
    else items.push({ id: playlistId, name: name.trim() || "Lista sincronizada", createdAt: new Date().toISOString(), wallpaperCount: normalizedIds.length, wallpaperIds: normalizedIds });
    saveWebPlaylists(items);
  },

  async removeWallpaper(playlistId: string, wallpaperId: string): Promise<void> {
    if (!isTauri()) { const items = webPlaylists(); const item = items.find(value => value.id === playlistId); if (item) { item.wallpaperIds = item.wallpaperIds.filter(id => id !== wallpaperId); item.wallpaperCount = item.wallpaperIds.length; } saveWebPlaylists(items); return; }
    await tauriInvoke("remove_wallpaper_from_playlist", { playlistId, wallpaperId });
    await tauriInvoke("mark_playlist_local_change", { playlistId, deleted: false });
    await notifyLocalChange("playlist_item_update", playlistId);
  },

  async reorder(playlistId: string, wallpaperIds: string[]): Promise<void> {
    if (!isTauri()) { const items = webPlaylists(); const item = items.find(value => value.id === playlistId); if (item) item.wallpaperIds = [...wallpaperIds]; saveWebPlaylists(items); return; }
    await tauriInvoke("reorder_playlist", { playlistId, wallpaperIds });
    await tauriInvoke("mark_playlist_local_change", { playlistId, deleted: false });
    await notifyLocalChange("playlist_item_update", playlistId);
  },
};
