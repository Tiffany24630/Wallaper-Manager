import { tauriInvoke } from "./tauri";
import type { SystemInfo } from "../types/system";
import { isTauri } from "./tauri";

export const systemService = {
  async getInfo(): Promise<SystemInfo> {
    if (!isTauri()) return { cpuUsage: 0, totalMemory: 0, usedMemory: 0, totalStorage: 0, usedStorage: 0, osName: navigator.userAgent, hostname: "Navegador", uptime: 0, wallpaperCount: 0, cpuName: "No disponible en web", logicalCores: navigator.hardwareConcurrency || 0, appMemory: 0 };
    return tauriInvoke<SystemInfo>("get_system_info");
  },
};
