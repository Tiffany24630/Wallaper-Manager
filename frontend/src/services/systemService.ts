import { tauriInvoke } from "./tauri";
import type { SystemInfo } from "../types/system";

export const systemService = {
  async getInfo(): Promise<SystemInfo> {
    return tauriInvoke<SystemInfo>("get_system_info");
  },
};