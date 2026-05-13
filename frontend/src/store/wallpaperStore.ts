import { create } from "zustand";

interface WallpaperState {
  currentWallpaper: string | null;
  setWallpaper: (path: string) => void;
}

export const useWallpaperStore = create<WallpaperState>((set) => ({
  currentWallpaper: null,

  setWallpaper: (path) =>
    set({ currentWallpaper: path })
}));