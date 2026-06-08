import { tauriInvoke } from "./tauri";
import type { Monitor } from "../types/monitor";

export const monitorService = {
  async getAll(): Promise<Monitor[]> {
    return tauriInvoke<Monitor[]>("get_monitors");
  },
};