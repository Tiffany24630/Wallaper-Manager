import { tauriInvoke } from "./tauri";
import type { Wallpaper } from "../types/wallpaper";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { notifyLocalChange } from "./syncQueueService";

export interface UpscaleResult {
  jobId: string;
  state: "pending" | "running" | "completed" | "failed" | "cancelled";
  id: string;
  method: string;
  warning?: string;
}

export interface WallpaperApplyResult {
  state: "confirmed" | "pending";
  wallpaperId: string;
  monitorId?: string;
  generation?: number;
  activationId?: string;
}

export type WallpaperApplyState = WallpaperApplyResult | "pending";

export interface AnimatedActivationEvent {
  state: "pending" | "ready" | "failed" | "cancelled";
  monitorId: string;
  generation: number;
  activationId: string;
  wallpaperId?: string;
  error?: string;
}

export const wallpaperService = {
  listenAnimatedActivation(handler: (event: AnimatedActivationEvent) => void): Promise<() => void> {
    return listen<AnimatedActivationEvent>("animated-activation", event => handler(event.payload));
  },
  async getAll(): Promise<Wallpaper[]> {
    return tauriInvoke<Wallpaper[]>("list_wallpapers");
  },

  async listDerivativeIds(parentWallpaperId: string): Promise<string[]> {
    return tauriInvoke<string[]>("list_wallpaper_derivative_ids", { parentWallpaperId });
  },

  async regenerateThumbnail(id: string): Promise<void> {
    await tauriInvoke("regenerate_wallpaper_thumbnail", { wallpaperId: id });
    await notifyLocalChange("wallpaper_update", id);
  },

  async setActive(id: string, monitorId?: string): Promise<WallpaperApplyResult> {
    return tauriInvoke<WallpaperApplyResult>("set_active_wallpaper", {
      wallpaperId: id,
      monitorId,
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
            "bmp",
            "gif"
          ],
        },
      ],
    });
    return file as string | null;
  },

  async selectWallpaperFiles(): Promise<string[]> {
    const files = await open({
      multiple: true,
      filters: [{ name: "Images", extensions: ["jpg", "jpeg", "png", "webp", "bmp", "gif"] }],
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
