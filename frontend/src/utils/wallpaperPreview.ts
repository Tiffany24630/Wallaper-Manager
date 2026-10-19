import { convertFileSrc } from "@tauri-apps/api/core";
import type { Wallpaper } from "../types/wallpaper";

export function wallpaperThumbnailSrc(wallpaper: Wallpaper): string | null {
  if (!wallpaper.thumbnail) return null;
  const version = wallpaper.hash || `${wallpaper.sizeBytes ?? 0}-${wallpaper.width ?? 0}x${wallpaper.height ?? 0}`;
  return `${convertFileSrc(wallpaper.thumbnail)}?v=${encodeURIComponent(version)}`;
}
