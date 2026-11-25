import { tauriInvoke } from "./tauri";
import type { Wallpaper } from "../types/wallpaper";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { notifyLocalChange } from "./syncQueueService";
import { isTauri } from "./tauri";
import { webStorage } from "./webStorage";

const webSelectedFiles = new Map<string, File>();
const webOnly = () => { if (isTauri()) throw new Error("Esta operación web no debe ejecutarse en Tauri."); };

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
    if (!isTauri()) return Promise.resolve(() => undefined);
    return listen<AnimatedActivationEvent>("animated-activation", event => handler(event.payload));
  },
  async getAll(): Promise<Wallpaper[]> {
    if (!isTauri()) return webStorage.list();
    return tauriInvoke<Wallpaper[]>("list_wallpapers");
  },

  async listDerivativeIds(parentWallpaperId: string): Promise<string[]> {
    if (!isTauri()) return [];
    return tauriInvoke<string[]>("list_wallpaper_derivative_ids", { parentWallpaperId });
  },

  async regenerateThumbnail(id: string): Promise<void> {
    if (!isTauri()) return;
    await tauriInvoke("regenerate_wallpaper_thumbnail", { wallpaperId: id });
    await notifyLocalChange("wallpaper_update", id);
  },

  async setActive(id: string, monitorId?: string): Promise<WallpaperApplyResult> {
    if (!isTauri()) {
      webOnly();
      const wallpapers = await webStorage.list();
      await Promise.all(wallpapers.map(item => webStorage.update(item.id, { active: item.id === id })));
      localStorage.setItem("lumina.web.activeWallpaper", id);
      document.body.style.backgroundImage = `url(${wallpapers.find(item => item.id === id)?.path ?? ""})`;
      document.body.style.backgroundSize = "cover";
      document.body.style.backgroundPosition = "center";
      return { state: "confirmed", wallpaperId: id, monitorId: monitorId ?? "browser" };
    }
    return tauriInvoke<WallpaperApplyResult>("set_active_wallpaper", {
      wallpaperId: id,
      monitorId,
    });
  },

  async toggleFavorite(id: string): Promise<void> {
    if (!isTauri()) { const item = (await webStorage.list()).find(value => value.id === id); if (item) await webStorage.update(id, { favorite: !item.favorite }); return; }
    await tauriInvoke("toggle_favorite", {
      wallpaperId: id,
    });
    await notifyLocalChange("wallpaper_update", id);
  },

  async importWallpaper(path: string): Promise<string> {
    if (!isTauri()) { const file = webSelectedFiles.get(path); if (!file) throw new Error("El archivo web seleccionado ya no está disponible."); const id = await webStorage.importFile(file); webSelectedFiles.delete(path); return id; }
    const id = await tauriInvoke<string>("import_wallpaper", {
      sourcePath: path,
    });
    await notifyLocalChange("wallpaper_create", id); return id;
  },

  async deleteWallpaper(id: string): Promise<void> {
    if (!isTauri()) { await webStorage.delete(id); return; }
    await tauriInvoke("delete_wallpaper", {
      wallpaperId: id,
    });
    await notifyLocalChange("wallpaper_delete", id);
  },

  async renameWallpaper(id: string, name: string): Promise<void> {
    if (!isTauri()) { const normalized = name.trim(); if (!normalized) throw new Error("El nombre no puede estar vacío."); await webStorage.update(id, { name: normalized }); return; }
    await tauriInvoke("rename_wallpaper", { wallpaperId: id, name }); await notifyLocalChange("wallpaper_update", id);
  },

  async rotateWallpaperVersion(id: string, degrees: 0 | 90 | 180 | 270): Promise<void> {
    if (!isTauri()) throw new Error("La rotación de imágenes en la web aún no está disponible; puedes descargar el fondo y rotarlo desde un editor.");
    await tauriInvoke("rotate_wallpaper_version", { wallpaperId: id, degrees }); await notifyLocalChange("wallpaper_update", id);
  },

  async selectWallpaperFile(): Promise<string | null> {
    if (!isTauri()) { const files = await this.selectWallpaperFiles(); return files[0] ?? null; }
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
    if (!isTauri()) {
      webOnly();
      return new Promise(resolve => { const input = document.createElement("input"); input.type = "file"; input.accept = "image/png,image/jpeg,image/webp,image/bmp,image/gif"; input.multiple = true; input.onchange = () => { const values = Array.from(input.files ?? []).filter(file => file.type.startsWith("image/")).map(file => { const id = `web-file:${crypto.randomUUID()}`; webSelectedFiles.set(id, file); return id; }); resolve(values); }; input.click(); });
    }
    const files = await open({
      multiple: true,
      filters: [{ name: "Images", extensions: ["jpg", "jpeg", "png", "webp", "bmp", "gif"] }],
    });
    if (!files) return [];
    return Array.isArray(files) ? files : [files];
  },

  async upscaleWallpaper(id: string, scale = 4): Promise<UpscaleResult> {
    if (!isTauri()) throw new Error("La mejora Real-ESRGAN requiere la aplicación de escritorio.");
    const result = await invoke<UpscaleResult>(
      "upscale_wallpaper",
      {
        wallpaperId: id,
        scale,
      }
    ); await notifyLocalChange("wallpaper_update", id); return result;
  },

  async scanFolder(): Promise<void> {
    if (!isTauri()) return;
    return tauriInvoke(
      "scan_wallpaper_folder"
    );
  },

  async rotateWallpaper(): Promise<void> {
    if (!isTauri()) { const items = await webStorage.list(); if (items.length) await this.setActive(items[Math.floor(Math.random() * items.length)].id); return; }
    return tauriInvoke(
      "rotate_random_wallpaper"
    );
  },

  async assignWallpaperToMonitor(
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
};
