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
export const accountService = {
  getApiUrl: () => apiUrl(), setApiUrl: (url: string) => localStorage.setItem(apiKey, url.replace(/\/$/, "")),
  register: async (email: string, password: string) => { const session = await request<Session>("/v1/auth/register", { method: "POST", body: JSON.stringify({ email, password }) }); localStorage.setItem(tokenKey, session.token); return session; },
  login: async (email: string, password: string) => { const session = await request<Session>("/v1/auth/login", { method: "POST", body: JSON.stringify({ email, password }) }); localStorage.setItem(tokenKey, session.token); return session; },
  session: () => request<{ authenticated: boolean; user?: Session["user"] }>("/v1/auth/session"),
  logout: async () => { try { await request("/v1/auth/logout", { method: "POST" }); } finally { localStorage.removeItem(tokenKey); } },
  devices: () => request<{ devices: Device[] }>("/v1/devices"),
  registerDevice: async (name: string) => { const id = localStorage.getItem(deviceKey) || crypto.randomUUID(); localStorage.setItem(deviceKey, id); return request<{ device_id: string }>("/v1/devices", { method: "POST", body: JSON.stringify({ id, name, platform: "windows" }) }); },
  sync: (changes: unknown[] = []) => request<{ records: unknown[]; conflicts: unknown[] }>("/v1/sync", { method: "POST", body: JSON.stringify({ device_id: localStorage.getItem(deviceKey), changes }) }),
  revokeDevice: (id: string) => request(`/v1/devices/${encodeURIComponent(id)}`, { method: "DELETE" }),
};
