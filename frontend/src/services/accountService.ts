import type { Device, Session } from "../types/account";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { wallpaperService } from "./wallpaperService";
import { cursorService } from "./cursorService";
import { playlistService } from "./playlistService";

const apiKey = "lumina.account.apiUrl";
const deviceKey = "lumina.account.deviceId";
let tokenCache: string | null | undefined;
export type SyncResponse = { records: Array<{ record_id: string; kind: string; revision: number; payload: Record<string, unknown>; deleted?: boolean }>; files: Array<{ id: string; record_id: string; sha256: string; kind: string; original_name?: string; extension?: string }>; conflicts: unknown[] };
type PlaylistSyncState = { playlistId: string; lastRemoteRevision?: number; basePayload?: string; localDirty: boolean; deletedPending: boolean };
const apiUrl = () => localStorage.getItem(apiKey) || "http://127.0.0.1:8765";
async function ensureToken(): Promise<string | null> {
  if (tokenCache !== undefined) return tokenCache;
  try { tokenCache = await invoke<string | null>("get_secure_session_token"); } catch { tokenCache = null; }
  return tokenCache;
}
async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const token = await ensureToken();
  const headers = new Headers(init.headers); headers.set("Content-Type", "application/json");
  if (token) headers.set("Authorization", `Bearer ${token}`);
  const response = await fetch(`${apiUrl()}${path}`, { ...init, headers });
  const data = await response.json().catch(() => ({ error: "Respuesta no válida del servidor" }));
  if (response.status === 401) { tokenCache = null; localStorage.removeItem("lumina.account.authenticated"); await invoke("delete_secure_session_token").catch(() => undefined); window.dispatchEvent(new Event("lumina:auth-changed")); }
  if (!response.ok) throw new Error(data.error || `Error HTTP ${response.status}`);
  return data as T;
}
async function fileRequest(path: string, init: RequestInit = {}) {
  const token = await ensureToken(); const headers = new Headers(init.headers);
  if (token) headers.set("Authorization", `Bearer ${token}`);
  const deviceId = localStorage.getItem(deviceKey); if (deviceId) headers.set("X-Device-ID", deviceId);
  const response = await fetch(`${apiUrl()}${path}`, { ...init, headers });
  if (!response.ok) { const data = await response.json().catch(() => ({ error: `Error HTTP ${response.status}` })); throw new Error(data.error || `Error HTTP ${response.status}`); }
  return response;
}
export const accountService = {
  initialize: async () => {
    await ensureToken();
    if (tokenCache) localStorage.setItem("lumina.account.authenticated", "1");
    const legacy = localStorage.getItem("lumina.account.token");
    if (tokenCache === null && legacy) {
      try { await fetch(`${apiUrl()}/v1/auth/logout`, { method: "POST", headers: { Authorization: `Bearer ${legacy}` } }); } catch { /* revoke best effort; never retain the legacy token */ }
      localStorage.removeItem("lumina.account.token");
    }
    if (!tokenCache) localStorage.removeItem("lumina.account.authenticated");
  },
  hasSession: () => tokenCache != null,
  getApiUrl: () => apiUrl(), setApiUrl: (url: string) => localStorage.setItem(apiKey, url.replace(/\/$/, "")),
  register: async (email: string, password: string) => { const session = await request<Session>("/v1/auth/register", { method: "POST", body: JSON.stringify({ email, password }) }); await invoke("set_secure_session_token", { token: session.token }); tokenCache = session.token; localStorage.setItem("lumina.account.authenticated", "1"); window.dispatchEvent(new Event("lumina:auth-changed")); return session; },
  login: async (email: string, password: string) => { const session = await request<Session>("/v1/auth/login", { method: "POST", body: JSON.stringify({ email, password }) }); await invoke("set_secure_session_token", { token: session.token }); tokenCache = session.token; localStorage.setItem("lumina.account.authenticated", "1"); window.dispatchEvent(new Event("lumina:auth-changed")); return session; },
  session: () => request<{ authenticated: boolean; user?: Session["user"] }>("/v1/auth/session"),
  logout: async () => { try { await request("/v1/auth/logout", { method: "POST" }); } finally { await invoke("delete_secure_session_token").catch(() => undefined); tokenCache = null; localStorage.removeItem("lumina.account.token"); localStorage.removeItem("lumina.account.authenticated"); window.dispatchEvent(new Event("lumina:auth-changed")); } },
  devices: () => request<{ devices: Device[] }>("/v1/devices"),
  registerDevice: async (name: string) => { const id = localStorage.getItem(deviceKey) || crypto.randomUUID(); localStorage.setItem(deviceKey, id); return request<{ device_id: string }>("/v1/devices", { method: "POST", body: JSON.stringify({ id, name, platform: "windows" }) }); },
  sync: (changes: unknown[] = []) => request<SyncResponse>("/v1/sync", { method: "POST", body: JSON.stringify({ device_id: localStorage.getItem(deviceKey), changes }) }),
  remoteResources: () => request<SyncResponse>("/v1/sync", { method: "POST", body: JSON.stringify({ device_id: localStorage.getItem(deviceKey), changes: [] }) }),
  queue: () => invoke<Array<{ id: string; operationType: string; state: string; attempts: number; lastError?: string }>>("list_sync_queue"),
  enqueue: () => invoke<string>("enqueue_sync_operation", { operationType: "library_sync", entityId: null, revision: null }),
  claim: (id: string) => invoke<boolean>("claim_sync_operation", { id }),
  finish: (id: string, success: boolean, error?: string) => invoke("finish_sync_operation", { id, success, error: error?.slice(0, 500) }),
  retry: (id: string) => invoke("retry_sync_operation", { id }),
  syncLibrary: async () => {
    const wallpapers = await wallpaperService.getAll(); const cursors = await cursorService.list(); const playlists = await playlistService.getAll();
    const remote = await accountService.sync(); const revisions = new Map(remote.records.map(record => [record.record_id, record.revision]));
    const states = await invoke<PlaylistSyncState[]>("list_sync_playlist_states").catch(() => [] as PlaylistSyncState[]);
    const stateMap = new Map(states.map(state => [state.playlistId, state]));
    const localPlaylistMap = new Map(playlists.map(playlist => [playlist.id, playlist]));
    const localPlaylistPayloads = new Map<string, Record<string, unknown>>();
    for (const playlist of playlists) localPlaylistPayloads.set(playlist.id, { name: playlist.name, wallpaper_ids: await playlistService.getPlaylistWallpapers(playlist.id) });
    for (const record of remote.records.filter(item => item.kind === "playlist")) {
      const local = localPlaylistMap.get(record.record_id); const state = stateMap.get(record.record_id);
      if (!local) {
        if (!state?.deletedPending && !record.deleted) {
          await invoke("apply_synced_playlist", { playlistId: record.record_id, name: String(record.payload.name || "Lista sincronizada"), wallpaperIds: Array.isArray(record.payload.wallpaper_ids) ? record.payload.wallpaper_ids.filter((id): id is string => typeof id === "string") : [] });
          await invoke("mark_playlist_synced", { playlistId: record.record_id, revision: record.revision, payload: record.payload });
        }
        continue;
      }
      if (!state) continue;
      if (state.localDirty) {
        let remoteUnchanged = false;
        try { remoteUnchanged = !!state.basePayload && JSON.stringify(JSON.parse(state.basePayload)) === JSON.stringify(record.payload); } catch { remoteUnchanged = false; }
        if (!remoteUnchanged && record.revision > (state.lastRemoteRevision ?? 0)) await invoke("record_playlist_conflict", { playlistId: record.record_id, localPayload: localPlaylistPayloads.get(record.record_id), remotePayload: record.payload, baseRevision: state.lastRemoteRevision ?? null, remoteRevision: record.revision });
      } else if (record.deleted) {
        await invoke("delete_playlist", { playlistId: record.record_id }).catch(() => undefined);
        await invoke("mark_playlist_synced", { playlistId: record.record_id, revision: record.revision, payload: record.payload });
      } else if (record.revision > (state.lastRemoteRevision ?? 0)) {
        await invoke("apply_synced_playlist", { playlistId: record.record_id, name: String(record.payload.name || local.name), wallpaperIds: Array.isArray(record.payload.wallpaper_ids) ? record.payload.wallpaper_ids.filter((id): id is string => typeof id === "string") : [] });
        await invoke("mark_playlist_synced", { playlistId: record.record_id, revision: record.revision, payload: record.payload });
      }
    }
    const changes = wallpapers.map(wallpaper => ({ id: wallpaper.id, kind: "wallpaper", base_revision: revisions.get(wallpaper.id) ?? 0, payload: { name: wallpaper.name, hash: wallpaper.hash ?? null, size_bytes: wallpaper.sizeBytes ?? null, width: wallpaper.width ?? null, height: wallpaper.height ?? null, file_type: wallpaper.fileType ?? null, favorite: wallpaper.favorite } }));
    const cursorChanges = cursors.map(cursor => ({ id: cursor.id, kind: "cursor", base_revision: revisions.get(cursor.id) ?? 0, payload: { name: cursor.name, resource_type: cursor.resourceType, hotspot_x: cursor.hotspotX, hotspot_y: cursor.hotspotY } }));
    const playlistChanges = playlists.map(playlist => ({ id: playlist.id, kind: "playlist", base_revision: stateMap.get(playlist.id)?.lastRemoteRevision ?? 0, payload: localPlaylistPayloads.get(playlist.id) }));
    const tombstones = await invoke<Array<[string, string]>>("list_sync_tombstones").catch(() => []);
    const deletedChanges = tombstones.map(([id, kind]) => ({ id, kind, base_revision: stateMap.get(id)?.lastRemoteRevision ?? revisions.get(id) ?? 0, deleted: true, payload: {} }));
    const result = await accountService.sync([...changes, ...cursorChanges, ...playlistChanges, ...deletedChanges]);
    const conflictIds = new Set(result.conflicts.map(conflict => String((conflict as { id?: string }).id || "")));
    for (const record of result.records.filter(item => item.kind === "playlist" && !conflictIds.has(item.record_id))) await invoke("mark_playlist_synced", { playlistId: record.record_id, revision: record.revision, payload: record.payload }).catch(() => undefined);
    const uploaded: string[] = []; const errors: string[] = [];
    for (const wallpaper of wallpapers) {
      if (result.files.some(file => file.record_id === wallpaper.id && file.sha256 === (wallpaper.hash ?? ""))) continue;
      try { const blob = await (await fetch(convertFileSrc(wallpaper.path, "asset"))).blob(); await accountService.uploadFile(blob, wallpaper.name, wallpaper.id, "wallpaper", mimeFor(wallpaper.fileType, wallpaper.path)); uploaded.push(wallpaper.id); } catch (error) { errors.push(`${wallpaper.name}: ${String(error)}`); }
    }
    for (const cursor of cursors) {
      try { const blob = await (await fetch(convertFileSrc(cursor.path, "asset"))).blob(); if (!result.files.some(file => file.record_id === cursor.id)) { await accountService.uploadFile(blob, cursor.name, cursor.id, "cursor", cursor.resourceType === "animated" ? "application/octet-stream" : "image/x-icon"); uploaded.push(cursor.id); } } catch (error) { errors.push(`${cursor.name}: ${String(error)}`); }
    }
    return { ...result, uploaded, errors };
  },
  uploadFile: async (file: Blob, name: string, recordId: string, kind: "wallpaper" | "cursor", mime: string) => {
    const hash = Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", await file.arrayBuffer()))).map(value => value.toString(16).padStart(2, "0")).join("");
    const init = await request<{ upload_id: string; offset: number; chunk_size: number }>("/v1/files/upload/init", { method: "POST", body: JSON.stringify({ device_id: localStorage.getItem(deviceKey), record_id: recordId, kind, name, mime, size: file.size, sha256: hash }) });
    let offset = init.offset; while (offset < file.size) { const chunk = file.slice(offset, offset + init.chunk_size); const response = await fileRequest(`/v1/files/upload/${encodeURIComponent(init.upload_id)}`, { method: "PUT", headers: { "Content-Type": "application/octet-stream", "X-Upload-Length": String(chunk.size), "X-Upload-Offset": String(offset) }, body: chunk }); const result = await response.json() as { offset: number }; offset = result.offset; }
    return request<{ file_id: string; sha256: string }>(`/v1/files/upload/${encodeURIComponent(init.upload_id)}/complete`, { method: "POST" });
  },
  downloadFile: async (fileId: string) => (await fileRequest(`/v1/files/${encodeURIComponent(fileId)}`)).blob(),
  restoreWallpaper: async (fileId: string, name: string, extension: string, expectedHash: string) => invoke<string>("download_and_import_synced_resource", { apiUrl: apiUrl(), deviceId: localStorage.getItem(deviceKey), fileId, kind: "wallpaper", name, extension, expectedHash }),
  restoreCursor: async (fileId: string, name: string, extension: string, expectedHash: string, hotspotX = 0, hotspotY = 0) => invoke<string>("download_and_import_synced_resource", { apiUrl: apiUrl(), deviceId: localStorage.getItem(deviceKey), fileId, kind: "cursor", name, extension, expectedHash, hotspotX, hotspotY }),
  revokeDevice: (id: string) => request(`/v1/devices/${encodeURIComponent(id)}`, { method: "DELETE" }),
};

function mimeFor(fileType: string | undefined, path: string): string {
  const extension = (fileType || path.split(".").pop() || "").toLowerCase();
  return ({ png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", webp: "image/webp", bmp: "image/bmp" } as Record<string, string>)[extension] || "application/octet-stream";
}
