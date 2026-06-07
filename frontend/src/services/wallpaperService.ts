import { convertFileSrc, invoke } from "@tauri-apps/api/core";

export type WallpaperKind = "image" | "video" | "animated" | "unknown";

export interface Wallpaper {
  id: string;
  title: string;
  kind: WallpaperKind;
  path: string;
  relativePath: string;
  sizeBytes: number;
  favorite: boolean;
  active: boolean;
  canApplyToDesktop: boolean;
}

export interface BackendStatus {
  ready: boolean;
  backendRoot: string;
  wallpaperRoot: string;
  configPath: string;
  cacheRoot: string;
  logRoot: string;
  totalWallpapers: number;
  totalPlaylists: number;
  activeWallpaper: Wallpaper | null;
  uptimeSeconds: number;
  supportedImages: string[];
  supportedVideos: string[];
}

export interface Playlist {
  id: string;
  name: string;
  wallpaperIds: string[];
  createdAt: number;
}

export interface AppSettings {
  launchAtStartup: boolean;
  minimizeToTray: boolean;
  hardwareAcceleration: boolean;
  pauseOnBattery: boolean;
  pauseWhenMaximized: boolean;
  fpsLimit: number;
  quality: string;
  scalingMode: string;
}

export interface ActiveResult {
  wallpaper: Wallpaper;
  desktopApplied: boolean;
  message: string;
}

export interface BackendSnapshot {
  status: BackendStatus;
  wallpapers: Wallpaper[];
  playlists: Playlist[];
  settings: AppSettings;
}

export const defaultSettings: AppSettings = {
  launchAtStartup: false,
  minimizeToTray: true,
  hardwareAcceleration: true,
  pauseOnBattery: true,
  pauseWhenMaximized: false,
  fpsLimit: 60,
  quality: "Ultra",
  scalingMode: "Fill",
};

export function isTauriRuntime() {
  return Boolean(
    typeof window !== "undefined" &&
      ((window as Window & { __TAURI__?: unknown }).__TAURI__ ||
        (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__)
  );
}

export function previewUrl(wallpaper?: Wallpaper | null) {
  if (!wallpaper || wallpaper.kind !== "image" || !isTauriRuntime()) {
    return wallpaper?.path ?? "";
  }

  return convertFileSrc(wallpaper.path);
}

export async function getBackendSnapshot(): Promise<BackendSnapshot> {
  if (isTauriRuntime()) {
    const [status, wallpapers, playlists, settings] = await Promise.all([
      invoke<BackendStatus>("get_backend_status"),
      invoke<Wallpaper[]>("list_wallpapers"),
      invoke<Playlist[]>("list_playlists"),
      invoke<AppSettings>("get_settings"),
    ]);
    return { status, wallpapers, playlists, settings };
  }

  const response = await fetch(`${import.meta.env.VITE_API_URL ?? "http://localhost:4000"}/status`);
  const status = await response.json();
  const [wallpapers, playlists, settings] = await Promise.all([
    fetch(`${import.meta.env.VITE_API_URL ?? "http://localhost:4000"}/wallpapers`).then((res) => res.json()),
    fetch(`${import.meta.env.VITE_API_URL ?? "http://localhost:4000"}/playlists`).then((res) => res.json()),
    fetch(`${import.meta.env.VITE_API_URL ?? "http://localhost:4000"}/settings`).then((res) => res.json()),
  ]);
  return { status: { ...status, ready: true }, wallpapers, playlists, settings };
}

export async function importWallpaper(source: File | string) {
  if (isTauriRuntime()) {
    if (typeof source !== "string") {
      throw new Error("Tauri import requires a local filesystem path.");
    }
    return invoke<Wallpaper>("import_wallpaper", { sourcePath: source });
  }

  if (source instanceof File) {
    const formData = new FormData();
    formData.append("file", source, source.name);
    const response = await fetch(`${import.meta.env.VITE_API_URL ?? "http://localhost:4000"}/wallpapers/import`, {
      method: "POST",
      body: formData,
    });
    if (!response.ok) {
      const error = await response.json().catch(() => ({}));
      throw new Error(error.error || "Import failed");
    }
    return response.json();
  }

  throw new Error("Import from local path is only supported when running in Tauri.");
}

export async function setActiveWallpaper(wallpaperId: string) {
  if (isTauriRuntime()) {
    return invoke<ActiveResult>("set_active_wallpaper", { wallpaperId });
  }
  const response = await fetch(`${import.meta.env.VITE_API_URL ?? "http://localhost:4000"}/wallpapers/${wallpaperId}/activate`, {
    method: "POST",
  });
  if (!response.ok) {
    throw new Error("Failed to activate wallpaper");
  }
  return response.json();
}

export async function toggleFavorite(wallpaperId: string) {
  if (isTauriRuntime()) {
    return invoke<Wallpaper>("toggle_favorite", { wallpaperId });
  }
  const response = await fetch(`${import.meta.env.VITE_API_URL ?? "http://localhost:4000"}/wallpapers/${wallpaperId}/favorite`, {
    method: "POST",
  });
  if (!response.ok) {
    throw new Error("Failed to toggle favorite");
  }
  return response.json();
}

export async function createPlaylist(name: string) {
  if (isTauriRuntime()) {
    return invoke<Playlist>("create_playlist", { name });
  }
  const response = await fetch(`${import.meta.env.VITE_API_URL ?? "http://localhost:4000"}/playlists`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ name }),
  });
  if (!response.ok) {
    throw new Error("Failed to create playlist");
  }
  return response.json();
}

export async function deletePlaylist(playlistId: string) {
  if (isTauriRuntime()) {
    return invoke<Playlist[]>("delete_playlist", { playlistId });
  }
  const response = await fetch(`${import.meta.env.VITE_API_URL ?? "http://localhost:4000"}/playlists/${playlistId}/delete`, {
    method: "POST",
  });
  if (!response.ok) {
    throw new Error("Failed to delete playlist");
  }
  return response.json();
}

export async function addWallpaperToPlaylist(playlistId: string, wallpaperId: string) {
  if (isTauriRuntime()) {
    return invoke<Playlist>("add_wallpaper_to_playlist", { playlistId, wallpaperId });
  }
  const response = await fetch(`${import.meta.env.VITE_API_URL ?? "http://localhost:4000"}/playlists/${playlistId}/add`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ wallpaperId }),
  });
  if (!response.ok) {
    throw new Error("Failed to add wallpaper to playlist");
  }
  return response.json();
}

export async function saveSettings(settings: AppSettings) {
  if (isTauriRuntime()) {
    return invoke<AppSettings>("save_settings", { settings });
  }
  const response = await fetch(`${import.meta.env.VITE_API_URL ?? "http://localhost:4000"}/settings`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(settings),
  });
  if (!response.ok) {
    throw new Error("Failed to save settings");
  }
  return response.json();
}

function assertBackend() {
  if (!isTauriRuntime()) {
    throw new Error("Backend offline: inicia el servicio del backend o ejecuta en Tauri para usar los comandos Rust.");
  }
}
