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
  createdAt?: string;
}
