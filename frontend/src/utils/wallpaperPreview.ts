import { convertFileSrc } from "@tauri-apps/api/core";
import type { Wallpaper } from "../types/wallpaper";
import { isTauri } from "../services/tauri";

export function wallpaperThumbnailSrc(wallpaper: Wallpaper): string | null {
  const thumbnailPath = wallpaper.thumbnailAsset?.path ?? wallpaper.thumbnail;
  if (!thumbnailPath || wallpaper.thumbnailAsset?.available === false) return null;
  const version = wallpaper.hash || `${wallpaper.sizeBytes ?? 0}-${wallpaper.width ?? 0}x${wallpaper.height ?? 0}`;
  const src = isTauri() ? convertFileSrc(thumbnailPath) : thumbnailPath;
  return `${src}?v=${encodeURIComponent(version)}`;
}
