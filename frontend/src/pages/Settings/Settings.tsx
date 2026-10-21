import { ImageOff, MonitorPlay, Save, Settings2, ShieldCheck } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useSettingsStore } from "../../store/settingsStore";
import type { Settings as SettingsType } from "../../types/settings";
import type { Wallpaper } from "../../types/wallpaper";
import { playlistService } from "../../services/playlistService";
import { useWallpaperStore } from "../../store/wallpaperStore";
import { wallpaperThumbnailSrc } from "../../utils/wallpaperPreview";
import { animatedPrototypeService } from "../../services/animatedPrototypeService";
import { monitorService } from "../../services/monitorService";
import type { Monitor } from "../../types/monitor";

const defaults: SettingsType = { launch_at_startup: false, minimize_to_tray: true, hardware_acceleration: true, pause_on_battery: false, pause_when_maximized: true, scaling_mode: "fill", accent_color: "violet", ui_scale: 100, dynamic_enabled: false, dynamic_playlist_id: null, dynamic_interval_minutes: 5, dynamic_current_wallpaper_id: null, dynamic_last_error: null, cursor_enabled: false, active_cursor_id: null };
const dynamicIntervals = [1, 5, 10, 15, 30, 60, 120, 360, 720, 1440];

function PlaylistWallpaperPreview({ wallpaper, name }: { wallpaper?: Wallpaper; name: string }) {
  const preview = wallpaper ? wallpaperThumbnailSrc(wallpaper) : null;
  const [failed, setFailed] = useState(!preview);

  return (
    <div className="aspect-video bg-white/5">
      {failed ? (
        <div className="grid h-full place-items-center text-slate-500" title="Miniatura no disponible"><ImageOff size={20}/></div>
      ) : (
        <img src={preview!} alt={name} className="h-full w-full object-cover" onError={() => setFailed(true)}/>
      )}
    </div>
  );
}

function Switch({ checked, onChange, label, description }: { checked: boolean; onChange: (value: boolean) => void; label: string; description: string }) {
  return <label className="flex cursor-pointer items-center justify-between gap-5 border-b border-white/[.07] py-4 last:border-0"><span><span className="block text-sm font-medium">{label}</span><span className="mt-1 block text-xs leading-5 muted">{description}</span></span><input className="sr-only" type="checkbox" checked={checked} onChange={e => onChange(e.target.checked)}/><span className={`relative h-6 w-11 shrink-0 rounded-full transition ${checked ? "bg-violet-500" : "bg-white/15"}`}><span className={`absolute top-1 h-4 w-4 rounded-full bg-white transition ${checked ? "left-6" : "left-1"}`}/></span></label>;
}

export default function Settings() {
  const { settings, loading, error, loadSettings, saveSettings } = useSettingsStore();
  const [draft, setDraft] = useState<SettingsType>(defaults);
  const [draftDirty, setDraftDirty] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [prototypeBusy, setPrototypeBusy] = useState(false);
  const [prototypeMediaPath, setPrototypeMediaPath] = useState<string | null>(null);
  const [prototypeStatus, setPrototypeStatus] = useState("Selecciona un archivo MP4 o GIF para comenzar.");
  const [prototypeMonitors, setPrototypeMonitors] = useState<Monitor[]>([]);
  const [prototypeMonitorId, setPrototypeMonitorId] = useState("");
  const [playlists, setPlaylists] = useState<{ id: string; name: string }[]>([]);
  const [playlistMembers, setPlaylistMembers] = useState<string[]>([]);
  const [playlistMembersLoading, setPlaylistMembersLoading] = useState(false);
  const playlistMembersCache = useRef(new Map<string, string[]>());
  const { wallpapers, loadWallpapers } = useWallpaperStore();
  useEffect(() => { void loadSettings(); }, [loadSettings]);
  useEffect(() => { void playlistService.getAll().then(setPlaylists).catch(() => setPlaylists([])); void loadWallpapers(); }, [loadWallpapers]);
  useEffect(() => {
    void monitorService.getAll().then(monitors => {
      setPrototypeMonitors(monitors);
      const primary = monitors.find(monitor => monitor.primary) ?? monitors[0];
      setPrototypeMonitorId(primary?.id ?? "");
    }).catch(() => setPrototypeMonitors([]));
  }, []);
  useEffect(() => { const timer = window.setInterval(() => void loadSettings(), 5000); return () => window.clearInterval(timer); }, [loadSettings]);
  useEffect(() => { if (settings && !draftDirty) setDraft(settings); }, [settings, draftDirty]);
  useEffect(() => {
    const playlistId = draft.dynamic_playlist_id;
    if (!playlistId) {
      setPlaylistMembers([]);
      setPlaylistMembersLoading(false);
      return;
    }
    const cached = playlistMembersCache.current.get(playlistId);
    if (cached) {
      setPlaylistMembers(cached);
      setPlaylistMembersLoading(false);
      return;
    }
    let cancelled = false;
    setPlaylistMembersLoading(true);
    void playlistService.getPlaylistWallpapers(playlistId).then(memberIds => {
      playlistMembersCache.current.set(playlistId, memberIds);
      if (!cancelled) setPlaylistMembers(memberIds);
    }).catch(() => {
      if (!cancelled) setPlaylistMembers([]);
    }).finally(() => {
      if (!cancelled) setPlaylistMembersLoading(false);
    });
    return () => { cancelled = true; };
  }, [draft.dynamic_playlist_id]);
  const patch = <K extends keyof SettingsType>(key: K, value: SettingsType[K]) => { setDraftDirty(true); setDraft(old => ({ ...old, [key]: value })); };
  async function save() {
    setMessage(null);
    if (draft.dynamic_enabled && !draft.dynamic_playlist_id) {
      setMessage("Selecciona una playlist antes de activar el fondo dinámico.");
      return;
    }
    const current = useSettingsStore.getState().settings;
    const next = current ? { ...current, ...draft } : draft;
    try { await saveSettings(next); setDraft(next); setDraftDirty(false); setMessage("Preferencias guardadas."); } catch (e) { setMessage(String(e)); }
  }

  async function chooseAnimatedPrototypeMedia() {
    setMessage(null);
    const mediaPath = await animatedPrototypeService.chooseMedia();
    if (!mediaPath) return;
    setPrototypeMediaPath(mediaPath);
    setPrototypeStatus(`Archivo seleccionado: ${mediaPath.split(/[\\/]/).pop() ?? mediaPath}`);
  }

  async function openAnimatedPrototype() {
    setMessage(null);
    if (!prototypeMediaPath) {
      setPrototypeStatus("Selecciona primero un archivo MP4 o GIF.");
      return;
    }
    setPrototypeBusy(true);
    setPrototypeStatus("Creando ventana diagnóstica y cargando WebView2…");
    try {
      await animatedPrototypeService.open(prototypeMediaPath, prototypeMonitorId, true);
      setPrototypeStatus("Ventana creada; revisa la superficie para confirmar carga y reproducción.");
    } catch (error) {
      setPrototypeStatus("No se pudo crear la superficie diagnóstica.");
      setMessage(String(error));
    } finally {
      setPrototypeBusy(false);
    }
  }

  async function openGeneratedAnimatedPrototypeTest() {
    setMessage(null);
    setPrototypeBusy(true);
    setPrototypeStatus("Generando GIF de diagnóstico y creando la ventana WebView2…");
    try {
      await animatedPrototypeService.openGeneratedTest(prototypeMonitorId);
      setPrototypeStatus("GIF generado y ventana creada; confirma que cambian sus dos fotogramas.");
    } catch (error) {
      setPrototypeStatus("No se pudo iniciar la prueba autónoma.");
      setMessage(String(error));
    } finally {
      setPrototypeBusy(false);
    }
  }

  async function openNativeAnimatedPrototypeTest() {
    setMessage(null);
    setPrototypeBusy(true);
    setPrototypeStatus("Generando GIF y creando un HWND nativo con WebView2…");
    try {
      await animatedPrototypeService.openNativeGeneratedTest(prototypeMonitorId);
      setPrototypeStatus("HWND nativo creado; confirma WebView2, carga del GIF y animación visual.");
    } catch (error) {
      setPrototypeStatus("No se pudo iniciar el renderer HWND nativo.");
      setMessage(String(error));
    } finally {
      setPrototypeBusy(false);
    }
  }

  async function closeNativeAnimatedPrototype() {
    setPrototypeBusy(true);
    try {
      await animatedPrototypeService.closeNative();
      setPrototypeStatus("Renderer HWND nativo cerrado.");
      setMessage("Renderer nativo cerrado.");
    } catch (error) {
      setMessage(String(error));
    } finally {
      setPrototypeBusy(false);
    }
  }

  async function closeAnimatedPrototype() {
    setPrototypeBusy(true);
    try {
      await animatedPrototypeService.close(prototypeMonitorId);
      setPrototypeStatus("Superficie cerrada.");
      setMessage("Superficie del prototipo cerrada.");
    } catch (error) {
      setMessage(String(error));
    } finally {
      setPrototypeBusy(false);
    }
  }

  return <div className="page">
    <header className="page-header"><div><div className="eyebrow">Preferencias locales</div><h1 className="page-title">Ajustes</h1><p className="page-subtitle">Configura el comportamiento que Lumina conservará entre sesiones.</p></div><button className="btn btn-primary" onClick={() => void save()} disabled={loading}><Save size={16}/> Guardar cambios</button></header>
    {(error || message) && <div className={error ? "status-error" : "status-success"}>{error || message}</div>}
    <div className="grid gap-5 lg:grid-cols-2">
      <section className="panel panel-pad"><div className="mb-2 flex items-center gap-2"><Settings2 size={18} className="text-violet-300"/><h2 className="font-semibold">Aplicación</h2></div>
        <Switch checked={draft.launch_at_startup} onChange={v => patch("launch_at_startup", v)} label="Ejecutar Lumina al iniciar Windows" description="Añade o elimina Lumina del inicio automático de tu usuario de Windows."/>
        <Switch checked={draft.minimize_to_tray} onChange={v => patch("minimize_to_tray", v)} label="Minimizar a la bandeja" description="Mantener Lumina disponible al cerrar la ventana principal."/>
        <Switch checked={draft.hardware_acceleration} onChange={v => patch("hardware_acceleration", v)} label="Aceleración por hardware" description="Preferencia para operaciones compatibles, incluida la mejora de imágenes."/>
      </section>
      <section className="panel panel-pad"><div className="mb-2 flex items-center gap-2"><ShieldCheck size={18} className="text-cyan-300"/><h2 className="font-semibold">Consumo y pantalla</h2></div>
        <Switch checked={draft.pause_on_battery} onChange={v => patch("pause_on_battery", v)} label="Reducir actividad con batería" description="Preferencia de ahorro cuando el dispositivo no está conectado."/>
        <Switch checked={draft.pause_when_maximized} onChange={v => patch("pause_when_maximized", v)} label="Reducir actividad con pantalla completa" description="Evita trabajo de fondo innecesario mientras usas otras aplicaciones."/>
        <label className="mt-4 block text-sm font-medium">Modo de ajuste</label><select className="input mt-2" value={draft.scaling_mode} onChange={e => patch("scaling_mode", e.target.value)}><option value="fill">Rellenar pantalla</option><option value="fit">Ajustar completa</option><option value="stretch">Estirar</option><option value="center">Centrar</option></select>
      </section>
      <section className="panel panel-pad lg:col-span-2"><div className="mb-2 flex items-center justify-between gap-3"><div><h2 className="font-semibold">Fondo dinámico</h2><p className="mt-1 text-sm muted">Cambia automáticamente entre los fondos de una lista mientras Lumina está ejecutándose.</p></div><Switch checked={draft.dynamic_enabled} onChange={value => patch("dynamic_enabled", value)} label="Activar" description="Usar la lista seleccionada como fondo global."/></div>
        <div className="grid gap-4 md:grid-cols-2"><div><label className="block text-sm font-medium">Lista de fondos</label><select className="input mt-2" value={draft.dynamic_playlist_id ?? ""} onChange={event => patch("dynamic_playlist_id", event.target.value || null)}><option value="">Selecciona una lista</option>{playlists.map(list => <option key={list.id} value={list.id}>{list.name}</option>)}</select></div><div><label className="block text-sm font-medium">Cambiar cada</label><select className="input mt-2" value={draft.dynamic_interval_minutes} onChange={event => patch("dynamic_interval_minutes", Number(event.target.value))}>{dynamicIntervals.map(minutes => <option key={minutes} value={minutes}>{minutes < 60 ? `${minutes} minuto${minutes === 1 ? "" : "s"}` : minutes < 1440 ? `${minutes / 60} hora${minutes === 60 ? "" : "s"}` : "24 horas"}</option>)}</select></div></div>
        {draft.dynamic_playlist_id && <div className="mt-5">
          <div className="mb-2 flex items-center justify-between gap-3"><span className="text-sm font-medium">Imágenes de la lista</span><span className="text-xs muted">{playlistMembers.length} {playlistMembers.length === 1 ? "imagen" : "imágenes"}</span></div>
          {playlistMembersLoading && playlistMembers.length === 0 ? <div className="rounded-xl border border-white/10 p-4 text-sm muted">Cargando miniaturas…</div> : playlistMembers.length === 0 ? <div className="rounded-xl border border-white/10 p-4 text-sm muted">Esta lista no contiene imágenes.</div> : <div className="grid max-h-72 gap-3 overflow-y-auto pr-1 sm:grid-cols-2 lg:grid-cols-3">{playlistMembers.map(id => {
            const wallpaper = wallpapers.find(item => item.id === id);
            return <article key={id} className="overflow-hidden rounded-xl border border-white/10 bg-white/[.03]">
              <PlaylistWallpaperPreview wallpaper={wallpaper} name={wallpaper?.name ?? "Wallpaper no disponible"}/>
              <div className="truncate p-2.5 text-xs" title={wallpaper?.name}>{wallpaper?.name ?? "Wallpaper no disponible"}</div>
            </article>;
          })}</div>}
        </div>}
        {draft.dynamic_enabled && !draft.dynamic_playlist_id && <div className="status-error mt-4">Selecciona una lista antes de guardar el fondo dinámico.</div>}
        {draft.dynamic_last_error && <div className="status-error mt-4">{draft.dynamic_last_error}</div>}
        {draft.dynamic_current_wallpaper_id && <div className="mt-3 text-sm muted">Fondo actual: {wallpapers.find(w => w.id === draft.dynamic_current_wallpaper_id)?.name ?? "no disponible"}</div>}
      </section>
      <section className="panel panel-pad lg:col-span-2">
        <div className="mb-4 flex items-center gap-2"><Settings2 size={18} className="text-violet-300"/><h2 className="font-semibold">Apariencia</h2></div>
        <div className="grid gap-5 md:grid-cols-2">
          <div><label className="block text-sm font-medium">Color de acento</label><div className="mt-3 flex flex-wrap gap-2">{(["violet", "cyan", "rose", "emerald"] as const).map(color => <button key={color} type="button" onClick={() => patch("accent_color", color)} className={`accent-swatch accent-${color} ${draft.accent_color === color ? "is-selected" : ""}`} aria-label={`Usar acento ${color}`} />)}</div></div>
          <div><label className="block text-sm font-medium">Tamaño de interfaz: {draft.ui_scale}%</label><input className="mt-4 w-full" type="range" min="85" max="115" step="5" value={draft.ui_scale} onChange={e => patch("ui_scale", Number(e.target.value))}/></div>
        </div>
      </section>
      <section className="panel panel-pad lg:col-span-2">
        <div className="mb-2 flex items-center gap-2"><MonitorPlay size={18} className="text-amber-300"/><h2 className="font-semibold">Prototipo técnico WebView2</h2></div>
        <p className="text-sm leading-6 muted">Prueba aislada de una superficie Win32/WebView2 para medios locales. No modifica playlists ni activa fondos animados de producción.</p>
        <label className="mt-4 block text-sm font-medium">Monitor de prueba</label>
        <select className="input mt-2 max-w-xl" value={prototypeMonitorId} onChange={event => setPrototypeMonitorId(event.target.value)} disabled={prototypeMonitors.length === 0}>
          {prototypeMonitors.length === 0 ? <option value="">No se detectaron monitores</option> : prototypeMonitors.map((monitor, index) => <option key={monitor.id} value={monitor.id}>{monitor.name || `Monitor ${index + 1}`} · {monitor.width} × {monitor.height}{monitor.primary ? " · Principal" : ""}</option>)}
        </select>
        <div className="mt-4 rounded-xl border border-white/10 bg-white/[.03] p-3 text-sm muted">
          <div>{prototypeStatus}</div>
          {prototypeMediaPath && <div className="mt-1 truncate text-xs" title={prototypeMediaPath}>Archivo: {prototypeMediaPath.split(/[\\/]/).pop()}</div>}
        </div>
        <div className="mt-4 flex flex-wrap gap-3">
          <button className="btn btn-secondary" type="button" onClick={() => void chooseAnimatedPrototypeMedia()} disabled={prototypeBusy || prototypeMonitors.length === 0}>Seleccionar MP4/GIF</button>
          <button className="btn btn-secondary" type="button" onClick={() => void openAnimatedPrototype()} disabled={prototypeBusy || prototypeMonitors.length === 0 || !prototypeMediaPath}>Abrir prototipo</button>
          <button className="btn btn-secondary" type="button" onClick={() => void openGeneratedAnimatedPrototypeTest()} disabled={prototypeBusy || prototypeMonitors.length === 0}>Probar GIF autónomo</button>
          <button className="btn btn-secondary" type="button" onClick={() => void closeAnimatedPrototype()} disabled={prototypeBusy || prototypeMonitors.length === 0}>Cerrar superficie</button>
          <button className="btn btn-secondary" type="button" onClick={() => void openNativeAnimatedPrototypeTest()} disabled={prototypeBusy || prototypeMonitors.length === 0}>Probar HWND nativo + WebView2</button>
          <button className="btn btn-secondary" type="button" onClick={() => void closeNativeAnimatedPrototype()} disabled={prototypeBusy}>Cerrar renderer nativo</button>
        </div>
      </section>
    </div>
    <div className="panel panel-pad text-sm leading-6 muted"><strong className="text-slate-200">Transparencia:</strong> estas opciones se guardan en SQLite. Lumina puede permanecer en la bandeja para mantener activos los fondos dinámicos, y el inicio automático se configura para tu usuario de Windows.</div>
  </div>;
}
