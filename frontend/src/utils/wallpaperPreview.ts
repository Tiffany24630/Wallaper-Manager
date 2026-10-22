import { convertFileSrc } from "@tauri-apps/api/core";
import type { Wallpaper } from "../types/wallpaper";

export function wallpaperThumbnailSrc(wallpaper: Wallpaper): string | null {
  const thumbnailPath = wallpaper.thumbnailAsset?.path ?? wallpaper.thumbnail;
  if (!thumbnailPath || wallpaper.thumbnailAsset?.available === false) return null;
  const version = wallpaper.hash || `${wallpaper.sizeBytes ?? 0}-${wallpaper.width ?? 0}x${wallpaper.height ?? 0}`;
  return `${convertFileSrc(thumbnailPath)}?v=${encodeURIComponent(version)}`;
}
