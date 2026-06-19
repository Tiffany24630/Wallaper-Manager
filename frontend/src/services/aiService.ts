import { invoke } from "@tauri-apps/api/core";

export async function upscaleWallpaper(
  wallpaperId: string
): Promise<string> {
  return await invoke(
    "upscale_wallpaper",
    {
      wallpaperId,
    }
  );
}