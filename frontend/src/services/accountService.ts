import type { Device, Session } from "../types/account";

const tokenKey = "lumina.account.token";
const apiKey = "lumina.account.apiUrl";
const deviceKey = "lumina.account.deviceId";
const apiUrl = () => localStorage.getItem(apiKey) || "http://127.0.0.1:8765";
async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const token = localStorage.getItem(tokenKey);
  const headers = new Headers(init.headers); headers.set("Content-Type", "application/json");
  if (token) headers.set("Authorization", `Bearer ${token}`);
  const response = await fetch(`${apiUrl()}${path}`, { ...init, headers });
  const data = await response.json().catch(() => ({ error: "Respuesta no válida del servidor" }));
  if (!response.ok) throw new Error(data.error || `Error HTTP ${response.status}`);
  return data as T;
}
async function fileRequest(path: string, init: RequestInit = {}) {
  const token = localStorage.getItem(tokenKey); const headers = new Headers(init.headers);
  if (token) headers.set("Authorization", `Bearer ${token}`);
  const deviceId = localStorage.getItem(deviceKey); if (deviceId) headers.set("X-Device-ID", deviceId);
  const response = await fetch(`${apiUrl()}${path}`, { ...init, headers });
  if (!response.ok) { const data = await response.json().catch(() => ({ error: `Error HTTP ${response.status}` })); throw new Error(data.error || `Error HTTP ${response.status}`); }
  return response;
}
export const accountService = {
  getApiUrl: () => apiUrl(), setApiUrl: (url: string) => localStorage.setItem(apiKey, url.replace(/\/$/, "")),
  register: async (email: string, password: string) => { const session = await request<Session>("/v1/auth/register", { method: "POST", body: JSON.stringify({ email, password }) }); localStorage.setItem(tokenKey, session.token); return session; },
  login: async (email: string, password: string) => { const session = await request<Session>("/v1/auth/login", { method: "POST", body: JSON.stringify({ email, password }) }); localStorage.setItem(tokenKey, session.token); return session; },
  session: () => request<{ authenticated: boolean; user?: Session["user"] }>("/v1/auth/session"),
  logout: async () => { try { await request("/v1/auth/logout", { method: "POST" }); } finally { localStorage.removeItem(tokenKey); } },
  devices: () => request<{ devices: Device[] }>("/v1/devices"),
  registerDevice: async (name: string) => { const id = localStorage.getItem(deviceKey) || crypto.randomUUID(); localStorage.setItem(deviceKey, id); return request<{ device_id: string }>("/v1/devices", { method: "POST", body: JSON.stringify({ id, name, platform: "windows" }) }); },
  sync: (changes: unknown[] = []) => request<{ records: unknown[]; conflicts: unknown[] }>("/v1/sync", { method: "POST", body: JSON.stringify({ device_id: localStorage.getItem(deviceKey), changes }) }),
  uploadFile: async (file: Blob, name: string, recordId: string, kind: "wallpaper" | "cursor", mime: string) => {
    const hash = Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", await file.arrayBuffer()))).map(value => value.toString(16).padStart(2, "0")).join("");
    const init = await request<{ upload_id: string; offset: number; chunk_size: number }>("/v1/files/upload/init", { method: "POST", body: JSON.stringify({ device_id: localStorage.getItem(deviceKey), record_id: recordId, kind, name, mime, size: file.size, sha256: hash }) });
    let offset = init.offset; while (offset < file.size) { const chunk = file.slice(offset, offset + init.chunk_size); const response = await fileRequest(`/v1/files/upload/${encodeURIComponent(init.upload_id)}`, { method: "PUT", headers: { "Content-Type": "application/octet-stream", "X-Upload-Length": String(chunk.size), "X-Upload-Offset": String(offset) }, body: chunk }); const result = await response.json() as { offset: number }; offset = result.offset; }
    return request<{ file_id: string; sha256: string }>(`/v1/files/upload/${encodeURIComponent(init.upload_id)}/complete`, { method: "POST" });
  },
  downloadFile: async (fileId: string) => (await fileRequest(`/v1/files/${encodeURIComponent(fileId)}`)).blob(),
  revokeDevice: (id: string) => request(`/v1/devices/${encodeURIComponent(id)}`, { method: "DELETE" }),
};
