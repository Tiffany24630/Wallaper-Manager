import { invoke } from "@tauri-apps/api/core";
import {open} from "@tauri-apps/plugin-dialog";

export async function tauriInvoke<T>(
  command: string,
  payload?: Record<string, unknown>
): Promise<T> {
  return invoke<T>(command, payload);
}

export async function selectWallpaper() {
  return await open({
    multiple: false,

    filters: [
      {
        name: "Images",
        extensions: [
          "png",
          "jpg",
          "jpeg",
          "webp",
          "bmp"
        ]
      }
    ]
  });
}