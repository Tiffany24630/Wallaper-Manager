import { Check, Monitor, RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import { useMonitorStore } from "../../store/monitorStore";
import { useWallpaperStore } from "../../store/wallpaperStore";
import { usePlaylistStore } from "../../store/playlistStore";
import { monitorService } from "../../services/monitorService";
import { wallpaperThumbnailSrc } from "../../utils/wallpaperPreview";

export default function MultiMonitor() {
  const { monitors, loading, error, loadMonitors, assignWallpaper } = useMonitorStore();
  const { wallpapers, loadWallpapers, error: wallpaperError } = useWallpaperStore();
  const { playlists, loadPlaylists, error: playlistError } = usePlaylistStore();
  const [monitorId, setMonitorId] = useState("");
  const [wallpaperId, setWallpaperId] = useState("");
  const [synced, setSynced] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [playlistId, setPlaylistId] = useState("");
  const [monitorPlaylists, setMonitorPlaylists] = useState<Awaited<ReturnType<typeof monitorService.getPlaylists>>>([]);
  const [monitorPlaylistsError, setMonitorPlaylistsError] = useState<string | null>(null);
  useEffect(() => { void loadMonitors(); void loadWallpapers(); void loadPlaylists(); void monitorService.getPlaylists().then(items => { setMonitorPlaylists(items); setMonitorPlaylistsError(null); }).catch(error => setMonitorPlaylistsError(String(error))); }, [loadMonitors, loadWallpapers, loadPlaylists]);
  useEffect(() => { if (!monitorId && monitors[0]) setMonitorId(monitors[0].id); }, [monitors, monitorId]);

  async function apply() {
    if (!wallpaperId || (!monitorId && !synced)) return;
    setBusy(true); setMessage(null);
    try {
      const targets = synced ? monitors.map(m => m.id) : [monitorId];
      for (const id of targets) await assignWallpaper(id, wallpaperId);
      setMessage(`Fondo aplicado a ${targets.length} ${targets.length === 1 ? "pantalla" : "pantallas"}.`);
    } catch (e) { setMessage(String(e)); } finally { setBusy(false); }
  }

  const selectedMonitorPlaylist = monitorPlaylists.find(item => item.monitorId === monitorId);
  useEffect(() => { setPlaylistId(selectedMonitorPlaylist?.playlistId ?? ""); }, [selectedMonitorPlaylist?.playlistId]);
  async function applyPlaylist() {
    if (!monitorId || !playlistId) return;
    setBusy(true); setMessage(null);
    try {
      await monitorService.assignPlaylist(monitorId, playlistId);
      setMonitorPlaylists(await monitorService.getPlaylists());
      setMessage("Playlist asignada a la pantalla. El modo dinámico global quedó pausado.");
    } catch (e) { setMessage(String(e)); } finally { setBusy(false); }
  }

  async function removePlaylist() {
    if (!monitorId) return;
    setBusy(true); setMessage(null);
    try {
      await monitorService.removePlaylist(monitorId);
      setMonitorPlaylists(await monitorService.getPlaylists());
      setMessage("Playlist desactivada para esta pantalla.");
    } catch (e) { setMessage(String(e)); } finally { setBusy(false); }
  }

  return <div className="page">
    <header className="page-header"><div><div className="eyebrow">Configuración de pantalla</div><h1 className="page-title">Monitores</h1><p className="page-subtitle">Las resoluciones y frecuencias se obtienen directamente de Windows.</p></div><button className="btn" onClick={loadMonitors} disabled={loading}><RefreshCw size={16} className={loading ? "animate-spin" : ""}/> Detectar de nuevo</button></header>
    {(error || wallpaperError || playlistError || monitorPlaylistsError || message) && <div className={error || wallpaperError || playlistError || monitorPlaylistsError || message?.toLowerCase().includes("error") || message?.includes("pudo") ? "status-error" : "status-success"}>{error || wallpaperError || playlistError || monitorPlaylistsError || message}</div>}
    <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
      {monitors.map(m => <button key={m.id} onClick={() => { setMonitorId(m.id); setSynced(false); }} className={`panel panel-pad text-left transition ${monitorId === m.id && !synced ? "ring-2 ring-violet-400/60" : "hover:border-white/20"}`}>
        <div className="flex items-start justify-between"><div className="grid h-11 w-11 place-items-center rounded-xl bg-violet-500/15 text-violet-300"><Monitor/></div>{m.primary && <span className="rounded-full bg-emerald-500/10 px-2.5 py-1 text-xs text-emerald-300">Principal</span>}</div>
        <div className="mt-5 font-semibold">{m.name || "Pantalla sin nombre"}</div><div className="mt-1 text-sm muted">{m.width} × {m.height} · {m.refreshRate ? `${m.refreshRate} Hz` : "frecuencia no disponible"}</div>
        {monitorPlaylists.some(item => item.monitorId === m.id) && <div className="mt-2 text-xs text-violet-200">Playlist dinámica asignada</div>}
      </button>)}
      {!loading && !error && monitors.length === 0 && <div className="empty md:col-span-2 xl:col-span-3">Windows no reportó monitores disponibles.</div>}
    </section>
    <section className="panel panel-pad">
      <div><h2 className="text-lg font-semibold">Playlist por pantalla</h2><p className="mt-1 text-sm muted">Cada pantalla puede cambiar entre una playlist distinta. La asignación se conserva aunque la pantalla se desconecte.</p></div>
      <div className="mt-5 grid gap-3 md:grid-cols-[1fr_1fr_auto_auto]">
        <select className="input" value={monitorId} onChange={event => { setMonitorId(event.target.value); setSynced(false); }}><option value="">Selecciona una pantalla</option>{monitors.map(m => <option key={m.id} value={m.id}>{m.name || m.id}</option>)}</select>
        <select className="input" value={playlistId} onChange={event => setPlaylistId(event.target.value)}><option value="">Selecciona una playlist</option>{playlists.map(list => <option key={list.id} value={list.id}>{list.name}</option>)}</select>
        <button className="btn btn-primary" onClick={() => void applyPlaylist()} disabled={busy || !monitorId || !playlistId}>{busy ? "Guardando…" : "Aplicar playlist"}</button>
        <button className="btn" onClick={() => void removePlaylist()} disabled={busy || !selectedMonitorPlaylist}>Desactivar</button>
      </div>
      {selectedMonitorPlaylist?.lastError && <div className="status-error mt-4">{selectedMonitorPlaylist.lastError}</div>}
    </section>
    <section className="panel panel-pad">
      <div className="flex flex-wrap items-center justify-between gap-3"><div><h2 className="text-lg font-semibold">Asignar fondo</h2><p className="mt-1 text-sm muted">Elige una imagen y aplícala a una pantalla o a todas.</p></div><label className="flex items-center gap-2 text-sm"><input type="checkbox" checked={synced} onChange={e => setSynced(e.target.checked)}/> Aplicar a todas</label></div>
      {wallpaperError ? <div className="status-error mt-6">No se pudieron cargar los wallpapers: {wallpaperError}</div> : wallpapers.length === 0 ? <div className="empty mt-6">Importa imágenes en la biblioteca antes de asignarlas.</div> : <>
        <div className="mt-5 grid gap-3 sm:grid-cols-2 md:grid-cols-3 xl:grid-cols-5">{wallpapers.map(w => <button key={w.id} onClick={() => setWallpaperId(w.id)} className={`relative overflow-hidden rounded-xl border text-left ${wallpaperId === w.id ? "border-violet-400 ring-2 ring-violet-400/30" : "border-white/10"}`}><div className="aspect-video bg-white/5">{wallpaperThumbnailSrc(w) && <img src={wallpaperThumbnailSrc(w)!} alt={w.name} className="h-full w-full object-cover"/>}</div><div className="truncate p-2.5 text-xs">{w.name}</div>{wallpaperId === w.id && <Check size={16} className="absolute right-2 top-2 rounded-full bg-violet-500 p-0.5"/>}</button>)}</div>
        <div className="mt-5 flex justify-end"><button className="btn btn-primary" onClick={() => void apply()} disabled={busy || !wallpaperId || monitors.length === 0}>{busy ? "Aplicando…" : synced ? "Aplicar a todas" : "Aplicar a pantalla"}</button></div>
      </>}
    </section>
  </div>;
}
