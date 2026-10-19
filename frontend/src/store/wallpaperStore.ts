import { create } from "zustand";
import type { Wallpaper } from "../types/wallpaper";
import { wallpaperService } from "../services/wallpaperService";
import type { UpscaleResult } from "../services/wallpaperService";

interface WallpaperState {
  wallpapers: Wallpaper[];
  loading: boolean;
  error: string | null;

  loadWallpapers: () => Promise<void>;
  refresh: () => Promise<void>;
  setActive: (id: string) => Promise<void>;
  toggleFavorite: (id: string) => Promise<void>;
  importWallpaper: (path: string) => Promise<string>;
  selectAndImportWallpaper: () => Promise<string | null>;
  selectWallpaperFiles: () => Promise<string[]>;
  importWallpapers: (paths: string[], onProgress?: (current: number, total: number) => void) => Promise<{ imported: string[]; failed: string[] }>;
  deleteWallpaper: (id: string) => Promise<void>;
  renameWallpaper: (id: string, name: string) => Promise<void>;
  rotateWallpaperVersion: (id: string, degrees: 0 | 90 | 180 | 270) => Promise<void>;
  scanFolder: () => Promise<void>;
  rotateWallpaper: () => Promise<void>;
  upscaleWallpaper: (id: string, scale?: number) => Promise<UpscaleResult>;
  assignWallpaperToMonitor: (monitorId: string, wallpaperId: string) => Promise<void>;
}

export const useWallpaperStore =
  create<WallpaperState>((set) => {
    let latestFetch = 0;

    const fetchLatest = async (): Promise<Wallpaper[]> => {
      const fetchId = ++latestFetch;
      const wallpapers = await wallpaperService.getAll();
      if (fetchId === latestFetch) set({ wallpapers });
      return wallpapers;
    };

    return {
    wallpapers: [],
    loading: false,
    error: null,

    refresh: async () => {
      try {
        await fetchLatest();
        set({ error: null });
      } catch (error) {
        set({
          error: String(error),
        });
      }
    },

    upscaleWallpaper: async (id, scale) => {
      const result = await wallpaperService.upscaleWallpaper(id, scale);
      await fetchLatest();
      return result;
    },

    loadWallpapers: async () => {
      try {
        set({
          loading: true,
          error: null,
        });

        await fetchLatest();
        set({ loading: false });
      } catch (error) {
        set({
          loading: false,
          error: String(error),
        });
      }
    },

    setActive: async (id) => {
      await wallpaperService.setActive(id);
      await fetchLatest();
    },

    toggleFavorite: async (id) => {
      await wallpaperService.toggleFavorite(id);
      await fetchLatest();
    },

    importWallpaper: async (path) => {
      const id = await wallpaperService.importWallpaper(path);
      await fetchLatest();
      return id;
    },

    selectAndImportWallpaper: async () => {
      const file = await wallpaperService.selectWallpaperFile();

      if (!file) return null;

      const id = await wallpaperService.importWallpaper(file);
      await fetchLatest();
      return id;
    },

    selectWallpaperFiles: () => wallpaperService.selectWallpaperFiles(),

    importWallpapers: async (paths, onProgress) => {
      const imported: string[] = [];
      const failed: string[] = [];
      for (let index = 0; index < paths.length; index += 1) {
        const path = paths[index];
        onProgress?.(index + 1, paths.length);
        try {
          imported.push(await wallpaperService.importWallpaper(path));
        } catch (error) {
          const fileName = path.split(/[\\/]/).pop() || "archivo";
          failed.push(`${fileName}: ${String(error)}`);
        }
      }
      await fetchLatest();
      return { imported, failed };
    },

    deleteWallpaper: async (id) => {
      await wallpaperService.deleteWallpaper(id);
      await fetchLatest();
    },

    renameWallpaper: async (id, name) => {
      await wallpaperService.renameWallpaper(id, name);
      latestFetch += 1;
      const normalizedName = name.trim();
      set(state => ({
        wallpapers: state.wallpapers.map(wallpaper => wallpaper.id === id ? { ...wallpaper, name: normalizedName } : wallpaper),
      }));
    },

    rotateWallpaperVersion: async (id, degrees) => {
      await wallpaperService.rotateWallpaperVersion(id, degrees);
      await fetchLatest();
    },

    scanFolder: async () => {
      await wallpaperService.scanFolder();
      await fetchLatest();
    },

    rotateWallpaper: async () => {
      await wallpaperService.rotateWallpaper();
      await fetchLatest();
    },

    assignWallpaperToMonitor: async (monitorId, wallpaperId) => {
      await wallpaperService.assignWallpaperToMonitor(monitorId, wallpaperId);
      await fetchLatest();
    },
    };
  });
