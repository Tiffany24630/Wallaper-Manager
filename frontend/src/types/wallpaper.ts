export type WallpaperResourceType = "static" | "animated";

export interface WallpaperAsset {
  path: string;
  originalPath?: string;
  hash?: string;
  sizeBytes?: number;
  fileType?: string;
  available: boolean;
}

export interface WallpaperThumbnail {
  path?: string;
  available: boolean;
}

export interface Wallpaper {
  id: string;
  name: string;
  path: string;
  thumbnail?: string;
  favorite: boolean;
  active: boolean;
  sizeBytes?: number;
  width?: number;
  height?: number;
  fileType?: string;
  hash?: string;
  createdAt?: string;
  resourceType: WallpaperResourceType;
  parentWallpaperId?: string;
  asset: WallpaperAsset;
  thumbnailAsset: WallpaperThumbnail;
}
