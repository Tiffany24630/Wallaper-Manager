import { invoke } from "@tauri-apps/api/core";
import {open} from "@tauri-apps/plugin-dialog";

export const isTauri = () => Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);

export async function tauriInvoke<T>(
  command: string,
  payload?: Record<string, unknown>
): Promise<T> {
  return invoke<T>(command, payload);
}

export async function selectWallpaper() {
  if (!isTauri()) throw new Error("Esta operación de archivos requiere la aplicación de escritorio Lumina.");
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
