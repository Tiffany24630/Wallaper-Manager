import { tauriInvoke } from "./tauri";
import type { Wallpaper } from "../types/wallpaper";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { notifyLocalChange } from "./syncQueueService";

export interface UpscaleResult {
  id: string;
  method: string;
  warning?: string;
}

export const wallpaperService = {
  async getAll(): Promise<Wallpaper[]> {
    return tauriInvoke<Wallpaper[]>("list_wallpapers");
  },

  async setActive(id: string): Promise<void> {
    return tauriInvoke("set_active_wallpaper", {
      wallpaperId: id,
    });
  },

  async toggleFavorite(id: string): Promise<void> {
    await tauriInvoke("toggle_favorite", {
      wallpaperId: id,
    });
    await notifyLocalChange("wallpaper_update", id);
  },

  async importWallpaper(path: string): Promise<string> {
    const id = await tauriInvoke<string>("import_wallpaper", {
      sourcePath: path,
    });
    await notifyLocalChange("wallpaper_create", id); return id;
  },

  async deleteWallpaper(id: string): Promise<void> {
    await tauriInvoke("delete_wallpaper", {
      wallpaperId: id,
    });
    await notifyLocalChange("wallpaper_delete", id);
  },

  async renameWallpaper(id: string, name: string): Promise<void> {
    await tauriInvoke("rename_wallpaper", { wallpaperId: id, name }); await notifyLocalChange("wallpaper_update", id);
  },

  async rotateWallpaperVersion(id: string, degrees: 0 | 90 | 180 | 270): Promise<void> {
    await tauriInvoke("rotate_wallpaper_version", { wallpaperId: id, degrees }); await notifyLocalChange("wallpaper_update", id);
  },

  async selectWallpaperFile(): Promise<string | null> {
    const file = await open({
      multiple: false,

      filters: [
        {
          name: "Images",
          extensions: [
            "jpg",
            "jpeg",
            "png",
            "webp",
            "bmp"
          ],
        },
      ],
    });
    return file as string | null;
  },

  async selectWallpaperFiles(): Promise<string[]> {
    const files = await open({
      multiple: true,
      filters: [{ name: "Images", extensions: ["jpg", "jpeg", "png", "webp", "bmp"] }],
    });
    if (!files) return [];
    return Array.isArray(files) ? files : [files];
  },

  async upscaleWallpaper(id: string, scale = 4): Promise<UpscaleResult> {
    const result = await invoke<UpscaleResult>(
      "upscale_wallpaper",
      {
        wallpaperId: id,
        scale,
      }
    ); await notifyLocalChange("wallpaper_update", id); return result;
  },

  async scanFolder(): Promise<void> {
    return tauriInvoke(
      "scan_wallpaper_folder"
    );
  },

  async rotateWallpaper(): Promise<void> {
    return tauriInvoke(
      "rotate_random_wallpaper"
    );
  },

  async assignWallpaperToMonitor(
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
