export interface Wallpaper {
  id: string;
  title: string;
  path: string;
  relativePath: string;
  kind: string;
  sizeBytes: number;
  active: boolean;
  favorite: boolean;
  canApplyToDesktop: boolean;
  createdAt: string;
}