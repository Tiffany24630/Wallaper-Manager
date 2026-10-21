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

  open(mediaPath: string, monitorId: string, diagnostic = true): Promise<void> {
    return tauriInvoke("open_animated_prototype", { mediaPath, monitorId, diagnostic });
  },

  openGeneratedTest(monitorId: string): Promise<void> {
    return tauriInvoke("open_animated_prototype_test", { monitorId });
  },

  openNativeGeneratedTest(monitorId: string): Promise<void> {
    return tauriInvoke("open_native_animated_prototype_test", { monitorId });
  },

  closeNative(): Promise<void> {
    return tauriInvoke("close_native_animated_prototype");
  },

  close(monitorId: string): Promise<void> {
    return tauriInvoke("close_animated_prototype", { monitorId });
  },

  reportMediaError(
    surfaceId: string,
    activationId: string,
    kind: "media" | "webview",
    reason?: string,
  ): Promise<void> {
    return tauriInvoke("report_animated_media_error", {
      surfaceId,
      activationId,
      kind,
      reason,
    });
  },
};
