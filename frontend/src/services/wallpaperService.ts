import { invoke } from "@tauri-apps/api/core";

export async function setWallpaper(path: string) {
  return await invoke("set_wallpaper", {
    path
  });
}