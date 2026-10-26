import { LogIn, LogOut, RefreshCw, UserRound } from "lucide-react";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { accountService, type SyncConflict, type SyncResponse } from "../../services/accountService";
import { syncCoordinator } from "../../services/syncCoordinator";
import { syncQueueService, type QueueItem } from "../../services/syncQueueService";
import type { AccountUser, Device } from "../../types/account";

export default function Account() {
  async function resolveConflict(conflict: SyncConflict) {
    setRemoteBusy(true); setMessage(null);
    try {
      const payload = JSON.parse(conflict.remotePayload) as { name?: unknown; wallpaper_ids?: unknown; hash?: unknown };
      if (conflict.kind !== "playlist") {
        if (typeof payload.hash !== "string" || !/^[a-f0-9]{64}$/i.test(payload.hash)) throw new Error("El conflicto no contiene un hash remoto válido.");
        const remote = await accountService.remoteResources();
        const record = remote.records.find(item => item.kind === conflict.kind && item.record_id === conflict.recordId);
        if (!record || record.revision !== conflict.remoteRevision) throw new Error("La revisión remota cambió; actualiza los conflictos e inténtalo de nuevo.");
        const remoteHash = payload.hash as string;
        const file = remote.files.find(item => item.kind === conflict.kind && item.record_id === conflict.recordId && item.sha256.toLowerCase() === remoteHash.toLowerCase());
        if (!file) throw new Error("No se encontró el archivo remoto correspondiente a este conflicto.");
        await accountService.acceptRemoteConflict(conflict, file);
        await refreshConflicts(); setMessage("Conflicto resuelto aceptando la versión remota.");
        return;
      }
      await invoke("apply_synced_playlist", { playlistId: conflict.recordId, name: String(payload.name || "Lista sincronizada"), wallpaperIds: Array.isArray(payload.wallpaper_ids) ? payload.wallpaper_ids.filter((id): id is string => typeof id === "string") : [] });
      await invoke("mark_playlist_synced", { playlistId: conflict.recordId, revision: conflict.remoteRevision, payload });
      await accountService.resolveConflict(conflict.id, conflict.kind);
      await refreshConflicts(); setMessage("Conflicto de playlist resuelto aceptando la versión remota.");
    } catch (error) { await accountService.recordConflictError(conflict.id, conflict.kind, String(error)).catch(() => undefined); await refreshConflicts(); setMessage(`No se resolvió el conflicto: ${String(error)}`); }
    finally { setRemoteBusy(false); }
  }
  async function keepLocalConflict(conflict: SyncConflict) {
    setRemoteBusy(true); setMessage(null);
    try { await accountService.keepLocalConflict(conflict.id, conflict.kind); await refreshConflicts(); setMessage("Conflicto resuelto conservando la versión local. La próxima sincronización publicará esa versión con la revisión remota actual."); }
    catch (error) { await accountService.recordConflictError(conflict.id, conflict.kind, String(error)).catch(() => undefined); await refreshConflicts(); setMessage(`No se pudo resolver el conflicto: ${String(error)}`); }
    finally { setRemoteBusy(false); }
  }
  const [user, setUser] = useState<AccountUser | null>(null);
  const [devices, setDevices] = useState<Device[]>([]);
  const [queue, setQueue] = useState<QueueItem[]>([]);
  const [remote, setRemote] = useState<SyncResponse | null>(null);
  const [conflicts, setConflicts] = useState<SyncConflict[]>([]);
  const [email, setEmail] = useState(""); const [password, setPassword] = useState("");
  const [deviceName, setDeviceName] = useState("Este equipo"); const [api, setApi] = useState(accountService.getApiUrl());
  const [register, setRegister] = useState(false); const [message, setMessage] = useState<string | null>(null); const [loadError, setLoadError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false); const [remoteBusy, setRemoteBusy] = useState(false);
  const refreshQueue = async () => { try { setQueue(await syncQueueService.list()); setLoadError(null); } catch (error) { setLoadError(`No se pudo cargar la cola: ${String(error)}`); } };
  async function refreshConflicts() { try { setConflicts(await accountService.listConflicts()); setLoadError(null); } catch (error) { setLoadError(`No se pudieron cargar los conflictos: ${String(error)}`); } }
  async function load() { try { await accountService.initialize(); const result = await accountService.session(); await refreshQueue(); if (result.authenticated && result.user) { setUser(result.user); setDevices((await accountService.devices()).devices); await refreshConflicts(); } } catch (error) { setUser(null); setLoadError(`No se pudo cargar la cuenta: ${String(error)}`); } }
  useEffect(() => { void load(); }, []);
  async function submit() { setBusy(true); setMessage(null); try { const session = register ? await accountService.register(email, password) : await accountService.login(email, password); setUser(session.user); await accountService.registerDevice(deviceName); setDevices((await accountService.devices()).devices); setPassword(""); } catch (error) { setMessage(String(error)); } finally { setBusy(false); } }
  async function sync() { setBusy(true); setMessage(null); try { await syncCoordinator.requestNow(); setMessage("Sincronización añadida a la cola."); await refreshQueue(); } catch (error) { setMessage(String(error)); } finally { setBusy(false); } }
  async function discoverRemote() { setRemoteBusy(true); setMessage(null); try { setRemote(await accountService.remoteResources()); } catch (error) { setMessage(String(error)); } finally { setRemoteBusy(false); } }
  async function restoreRemote(file: SyncResponse["files"][number]) { const record = remote?.records.find(item => item.record_id === file.record_id && item.kind === file.kind); if (!record) return; setRemoteBusy(true); setMessage(null); try { const payload = record.payload; const name = String(payload.name || file.original_name || "Recurso remoto"); const extension = String(payload.file_type || file.extension || file.original_name?.split(".").pop() || "png"); if (file.kind === "wallpaper") await accountService.restoreWallpaper(file.id, name, extension, file.sha256); else await accountService.restoreCursor(file.id, name, extension, file.sha256, Number(payload.hotspot_x || 0), Number(payload.hotspot_y || 0)); setMessage(`${name} restaurado correctamente.`); } catch (error) { setMessage(String(error)); } finally { setRemoteBusy(false); } }
  async function logout() { await accountService.logout(); setUser(null); setDevices([]); setRemote(null); }
  const conflictPanel = user ? <div className="panel panel-pad mt-5"><div className="flex items-center justify-between"><h3 className="font-medium">Conflictos pendientes ({conflicts.length})</h3><button className="text-sm text-cyan-300" onClick={() => void refreshConflicts()}>Actualizar</button></div>{conflicts.map(conflict => <div className="mt-2 flex items-center justify-between text-sm" key={conflict.id}><span>{conflict.kind} · {conflict.recordId}{conflict.lastError ? ` · ${conflict.lastError}` : ""}</span><div className="flex gap-2">{conflict.kind === "playlist" && <button className="text-cyan-300" disabled={remoteBusy} onClick={() => void resolveConflict(conflict)}>Aceptar versión remota</button>}<button className="text-cyan-300" disabled={remoteBusy} onClick={() => void keepLocalConflict(conflict)}>Conservar local</button></div></div>)}</div> : null;
  const resourceConflictPanel = user && conflicts.some(conflict => conflict.kind === "wallpaper" || conflict.kind === "cursor") ? <div className="panel panel-pad mt-5"><h3 className="font-medium">Conflictos de recursos</h3>{conflicts.filter(conflict => conflict.kind === "wallpaper" || conflict.kind === "cursor").map(conflict => <div className="mt-2 flex items-center justify-between text-sm" key={`resource-${conflict.id}`}><span>{conflict.kind} · {conflict.recordId}{conflict.lastError ? ` · ${conflict.lastError}` : ""}</span><button className="text-cyan-300" disabled={remoteBusy} onClick={() => void resolveConflict(conflict)}>Aceptar versión remota</button></div>)}</div> : null;
  return <div className="page">
    <header className="page-header"><div><div className="eyebrow">Cuenta opcional</div><h1 className="page-title">Cuenta y dispositivos</h1><p className="page-subtitle">Lumina continúa funcionando localmente sin iniciar sesión.</p></div>{user && <button className="btn btn-secondary" onClick={() => void logout()}><LogOut size={16}/> Cerrar sesión</button>}</header>
    {(message || loadError) && <div className="status-error mb-4">{message || loadError}</div>}{conflictPanel}{resourceConflictPanel}
    <section className="panel panel-pad mb-5"><label className="block text-sm font-medium">URL de la API</label><div className="mt-2 flex gap-2"><input className="input" value={api} onChange={e => setApi(e.target.value)} /><button className="btn btn-secondary" onClick={() => { try { accountService.setApiUrl(api); setMessage("URL guardada."); } catch (error) { setMessage(String(error)); } }}>Guardar</button></div></section>
    {!user ? <section className="panel panel-pad max-w-xl"><div className="mb-4 flex items-center gap-2"><UserRound size={18}/><h2 className="font-semibold">{register ? "Crear cuenta" : "Iniciar sesión"}</h2></div><input className="input mb-3" type="email" placeholder="Correo electrónico" value={email} onChange={e => setEmail(e.target.value)}/><input className="input mb-4" type="password" placeholder="Contraseña (mínimo 10 caracteres)" value={password} onChange={e => setPassword(e.target.value)}/><button className="btn btn-primary" disabled={busy} onClick={() => void submit()}>{register ? <UserRound size={16}/> : <LogIn size={16}/>} {register ? "Registrarse" : "Entrar"}</button><button className="ml-3 text-sm muted" onClick={() => setRegister(!register)}>{register ? "Ya tengo cuenta" : "Crear una cuenta"}</button></section> : <section className="panel panel-pad"><h2 className="font-semibold">{user.email}</h2><div className="mt-4 flex flex-wrap gap-3"><input className="input max-w-xs" value={deviceName} onChange={e => setDeviceName(e.target.value)}/><button className="btn btn-secondary" onClick={() => void accountService.registerDevice(deviceName).then(() => accountService.devices().then(r => setDevices(r.devices)))}><RefreshCw size={16}/> Vincular dispositivo</button><button className="btn btn-primary" disabled={busy} onClick={() => void sync()}>Sincronizar biblioteca</button><button className="btn btn-secondary" disabled={remoteBusy} onClick={() => void discoverRemote()}>Ver recursos remotos</button></div><div className="mt-5 border-t border-white/10 pt-4"><h3 className="font-medium">Cola de sincronización ({queue.filter(item => item.state !== "completed").length} pendiente(s))</h3>{queue.filter(item => item.state !== "completed").map(item => <div className="mt-2 flex items-center justify-between text-sm" key={item.id}><span>{item.state} · intento {item.attempts}{item.lastError ? ` · ${item.lastError}` : ""}</span>{item.state === "failed" && <button className="text-cyan-300" onClick={() => void syncQueueService.retry(item.id).then(refreshQueue)}>Reintentar</button>}</div>)}</div>{remote && <div className="mt-6 border-t border-white/10 pt-4"><div className="flex items-center justify-between"><h3 className="font-medium">Recursos remotos</h3><button className="text-sm text-cyan-300" disabled={remoteBusy} onClick={() => void discoverRemote()}>Actualizar</button></div>{remote.files.filter(file => file.kind === "wallpaper" || file.kind === "cursor").map(file => { const record = remote.records.find(item => item.record_id === file.record_id); const name = String(record?.payload.name || file.original_name || file.id); return <div className="mt-2 flex items-center justify-between text-sm" key={file.id}><span>{name} <small className="muted">({file.kind})</small></span><button className="text-cyan-300" disabled={remoteBusy} onClick={() => void restoreRemote(file)}>Restaurar</button></div>; })}</div>}<h3 className="mt-6 font-medium">Dispositivos vinculados</h3>{devices.map(device => <div className="mt-3 flex items-center justify-between border-b border-white/10 py-3" key={device.id}><span>{device.name} <small className="muted">({device.platform})</small></span><button className="text-sm text-rose-300" onClick={() => void accountService.revokeDevice(device.id).then(() => setDevices(devices.filter(item => item.id !== device.id)))}>Desvincular</button></div>)}</section>}
  </div>;
}
