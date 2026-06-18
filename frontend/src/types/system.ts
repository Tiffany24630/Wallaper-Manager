export interface CpuInfo {
  brand: string;
  usage: number;
  cores: number;
}

export interface MemoryInfo {
  total: number;
  used: number;
  free: number;
}

export interface GpuInfo {
  name: string;
  vendor: string;
  memory?: number;
}

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