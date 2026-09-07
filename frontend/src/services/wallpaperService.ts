import { tauriInvoke } from "./tauri";
import type { Wallpaper } from "../types/wallpaper";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";

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
    return tauriInvoke("toggle_favorite", {
      wallpaperId: id,
    });
  },

  async importWallpaper(path: string): Promise<string> {
    return tauriInvoke<string>("import_wallpaper", {
      sourcePath: path,
    });
  },

  async deleteWallpaper(id: string): Promise<void> {
    return tauriInvoke("delete_wallpaper", {
      wallpaperId: id,
    });
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

  async upscaleWallpaper(id: string, scale = 4) {
    return invoke<string>(
      "upscale_wallpaper",
      {
        wallpaperId: id,
        scale,
      }
    );
  },

  async scanFolder(): Promise<void> {
    return tauriInvoke(
      "scan_wallpaper_folder"
    );
  },

  async rotateWallpaper(): Promise<void> {
    return tauriInvoke(
      "rotate_wallpaper"
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
