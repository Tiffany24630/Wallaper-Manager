import { create } from "zustand";
import type { Wallpaper } from "../types/wallpaper";
import { wallpaperService } from "../services/wallpaperService";

interface WallpaperState {
  wallpapers: Wallpaper[];
  loading: boolean;
  error: string | null;

  loadWallpapers: () => Promise<void>;
  refresh: () => Promise<void>;
  setActive: (id: string) => Promise<void>;
  toggleFavorite: (id: string) => Promise<void>;
  importWallpaper: (path: string) => Promise<void>;
  selectAndImportWallpaper: () => Promise<void>;
  deleteWallpaper: (id: string) => Promise<void>;
  scanFolder: () => Promise<void>;
  rotateWallpaper: () => Promise<void>;
  upscaleWallpaper: (id: string) => Promise<string>;
  assignWallpaperToMonitor: (monitorId: string, wallpaperId: string) => Promise<void>;
}

export const useWallpaperStore =
  create<WallpaperState>((set) => ({
    wallpapers: [],
    loading: false,
    error: null,

    refresh: async () => {
      try {
        const wallpapers = await wallpaperService.getAll();

        set({
          wallpapers,
          error: null,
        });
      } catch (error) {
        set({
          error: String(error),
        });
      }
    },

    upscaleWallpaper: async (id) => {
      return await wallpaperService.upscaleWallpaper(id);
    },

    loadWallpapers: async () => {
      try {
        set({
          loading: true,
          error: null,
        });

        const wallpapers = await wallpaperService.getAll();

        set({
          wallpapers,
          loading: false,
        });
      } catch (error) {
        set({
          loading: false,
          error: String(error),
        });
      }
    },

    setActive: async (id) => {
      await wallpaperService.setActive(id);
      const wallpapers = await wallpaperService.getAll();
      set({ wallpapers });
    },

    toggleFavorite: async (id) => {
      await wallpaperService.toggleFavorite(id);
      const wallpapers = await wallpaperService.getAll();
      set({ wallpapers });
    },

    importWallpaper: async (path) => {
      await wallpaperService.importWallpaper(path);
      const wallpapers = await wallpaperService.getAll();
      set({ wallpapers });
    },

    selectAndImportWallpaper: async () => {
      const file = await wallpaperService.selectWallpaperFile();

      if (!file) return;

      await wallpaperService.importWallpaper(file);
      const wallpapers = await wallpaperService.getAll();
      set({ wallpapers });
    },

    deleteWallpaper: async (id) => {
      await wallpaperService.deleteWallpaper(id);
      const wallpapers = await wallpaperService.getAll();
      set({ wallpapers });
    },

    scanFolder: async () => {
      await wallpaperService.scanFolder();
      const wallpapers = await wallpaperService.getAll();
      set({ wallpapers });
    },

    rotateWallpaper: async () => {
      await wallpaperService.rotateWallpaper();
      const wallpapers = await wallpaperService.getAll();
      set({ wallpapers });
    },

    assignWallpaperToMonitor: async (monitorId, wallpaperId) => {
      await wallpaperService.assignWallpaperToMonitor(monitorId, wallpaperId);
      const wallpapers = await wallpaperService.getAll();
      set({ wallpapers });
    },
  }));