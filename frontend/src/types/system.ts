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
  os: string;
  hostname: string;
  uptime: number;
  cpu: CpuInfo;
  memory: MemoryInfo;
  gpu: GpuInfo[];
}