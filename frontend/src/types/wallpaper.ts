export interface Wallpaper {
  id: string;
  name: string;
  path: string;
  thumbnail?: string;
  favorite: boolean;
  active: boolean;
  sizeBytes?: number;
  createdAt?: string;
}