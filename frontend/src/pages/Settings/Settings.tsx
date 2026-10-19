import { Save, Settings2, ShieldCheck } from "lucide-react";
import { useEffect, useState } from "react";
import { useSettingsStore } from "../../store/settingsStore";
import type { Settings as SettingsType } from "../../types/settings";
import { playlistService } from "../../services/playlistService";
import { useWallpaperStore } from "../../store/wallpaperStore";

const defaults: SettingsType = { launch_at_startup: false, minimize_to_tray: true, hardware_acceleration: true, pause_on_battery: false, pause_when_maximized: true, scaling_mode: "fill", accent_color: "violet", ui_scale: 100, dynamic_enabled: false, dynamic_playlist_id: null, dynamic_interval_minutes: 5, dynamic_current_wallpaper_id: null, dynamic_last_error: null, cursor_enabled: false, active_cursor_id: null };
const dynamicIntervals = [1, 5, 10, 15, 30, 60, 120, 360, 720, 1440];

function Switch({ checked, onChange, label, description }: { checked: boolean; onChange: (value: boolean) => void; label: string; description: string }) {
  return <label className="flex cursor-pointer items-center justify-between gap-5 border-b border-white/[.07] py-4 last:border-0"><span><span className="block text-sm font-medium">{label}</span><span className="mt-1 block text-xs leading-5 muted">{description}</span></span><input className="sr-only" type="checkbox" checked={checked} onChange={e => onChange(e.target.checked)}/><span className={`relative h-6 w-11 shrink-0 rounded-full transition ${checked ? "bg-violet-500" : "bg-white/15"}`}><span className={`absolute top-1 h-4 w-4 rounded-full bg-white transition ${checked ? "left-6" : "left-1"}`}/></span></label>;
}

export default function Settings() {
  const { settings, loading, error, loadSettings, saveSettings } = useSettingsStore();
  const [draft, setDraft] = useState<SettingsType>(defaults);
  const [message, setMessage] = useState<string | null>(null);
  const [playlists, setPlaylists] = useState<{ id: string; name: string }[]>([]);
  const { wallpapers, loadWallpapers } = useWallpaperStore();
  useEffect(() => { void loadSettings(); }, [loadSettings]);
  useEffect(() => { void playlistService.getAll().then(setPlaylists).catch(() => setPlaylists([])); void loadWallpapers(); }, [loadWallpapers]);
  useEffect(() => { const timer = window.setInterval(() => void loadSettings(), 5000); return () => window.clearInterval(timer); }, [loadSettings]);
  useEffect(() => { if (settings) setDraft(settings); }, [settings]);
  const patch = <K extends keyof SettingsType>(key: K, value: SettingsType[K]) => setDraft(old => ({ ...old, [key]: value }));
  async function save() { setMessage(null); try { await saveSettings(draft); setMessage("Preferencias guardadas."); } catch (e) { setMessage(String(e)); } }

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
    </div>
    <div className="panel panel-pad text-sm leading-6 muted"><strong className="text-slate-200">Transparencia:</strong> estas opciones se guardan en SQLite. Lumina puede permanecer en la bandeja para mantener activos los fondos dinámicos, y el inicio automático se configura para tu usuario de Windows.</div>
  </div>;
}
