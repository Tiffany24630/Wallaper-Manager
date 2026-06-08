import { tauriInvoke } from "./tauri";
import type { Wallpaper } from "../types/wallpaper";
import { open } from "@tauri-apps/plugin-dialog";

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

  async importWallpaper(path: string): Promise<void> {
    return tauriInvoke("import_wallpaper", {
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
};