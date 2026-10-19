import { open } from "@tauri-apps/plugin-dialog";
import { tauriInvoke } from "./tauri";

export const animatedPrototypeService = {
  async chooseMedia(): Promise<string | null> {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Animated media", extensions: ["mp4", "gif"] }],
    });
    return typeof selected === "string" ? selected : null;
  },

  open(mediaPath: string, monitorIndex = 0, diagnostic = true): Promise<void> {
    return tauriInvoke("open_animated_prototype", { mediaPath, monitorIndex, diagnostic });
  },

  openGeneratedTest(monitorIndex = 0): Promise<void> {
    return tauriInvoke("open_animated_prototype_test", { monitorIndex });
  },

  close(monitorIndex = 0): Promise<void> {
    return tauriInvoke("close_animated_prototype", { monitorIndex });
  },
};
