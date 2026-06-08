export interface SystemInfo {
  cpuUsage: number;
  totalMemory: number;
  usedMemory: number;
  totalStorage: number;
  usedStorage: number;
  osName: string;
  hostname: string;
  uptime: number;
  wallpaperCount: number;
  activeWallpaper?: string;
}