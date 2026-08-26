import { Save, Settings2, ShieldCheck } from "lucide-react";
import { useEffect, useState } from "react";
import { useSettingsStore } from "../../store/settingsStore";
import type { Settings as SettingsType } from "../../types/settings";

const defaults: SettingsType = { launch_at_startup: false, minimize_to_tray: true, hardware_acceleration: true, pause_on_battery: false, pause_when_maximized: true, scaling_mode: "fill" };

function Switch({ checked, onChange, label, description }: { checked: boolean; onChange: (value: boolean) => void; label: string; description: string }) {
  return <label className="flex cursor-pointer items-center justify-between gap-5 border-b border-white/[.07] py-4 last:border-0"><span><span className="block text-sm font-medium">{label}</span><span className="mt-1 block text-xs leading-5 muted">{description}</span></span><input className="sr-only" type="checkbox" checked={checked} onChange={e => onChange(e.target.checked)}/><span className={`relative h-6 w-11 shrink-0 rounded-full transition ${checked ? "bg-violet-500" : "bg-white/15"}`}><span className={`absolute top-1 h-4 w-4 rounded-full bg-white transition ${checked ? "left-6" : "left-1"}`}/></span></label>;
}

export default function Settings() {
  const { settings, loading, error, loadSettings, saveSettings } = useSettingsStore();
  const [draft, setDraft] = useState<SettingsType>(defaults);
  const [message, setMessage] = useState<string | null>(null);
  useEffect(() => { void loadSettings(); }, [loadSettings]);
  useEffect(() => { if (settings) setDraft(settings); }, [settings]);
  const patch = <K extends keyof SettingsType>(key: K, value: SettingsType[K]) => setDraft(old => ({ ...old, [key]: value }));
  async function save() { setMessage(null); try { await saveSettings(draft); setMessage("Preferencias guardadas."); } catch (e) { setMessage(String(e)); } }

  return <div className="page">
    <header className="page-header"><div><div className="eyebrow">Preferencias locales</div><h1 className="page-title">Ajustes</h1><p className="page-subtitle">Configura el comportamiento que Lumina conservará entre sesiones.</p></div><button className="btn btn-primary" onClick={() => void save()} disabled={loading}><Save size={16}/> Guardar cambios</button></header>
    {(error || message) && <div className={error ? "status-error" : "status-success"}>{error || message}</div>}
    <div className="grid gap-5 lg:grid-cols-2">
      <section className="panel panel-pad"><div className="mb-2 flex items-center gap-2"><Settings2 size={18} className="text-violet-300"/><h2 className="font-semibold">Aplicación</h2></div>
        <Switch checked={draft.launch_at_startup} onChange={v => patch("launch_at_startup", v)} label="Preferir inicio automático" description="Guarda la preferencia para una integración de inicio automático."/>
        <Switch checked={draft.minimize_to_tray} onChange={v => patch("minimize_to_tray", v)} label="Minimizar a la bandeja" description="Mantener Lumina disponible al cerrar la ventana principal."/>
        <Switch checked={draft.hardware_acceleration} onChange={v => patch("hardware_acceleration", v)} label="Aceleración por hardware" description="Preferencia para operaciones compatibles, incluida la mejora de imágenes."/>
      </section>
      <section className="panel panel-pad"><div className="mb-2 flex items-center gap-2"><ShieldCheck size={18} className="text-cyan-300"/><h2 className="font-semibold">Consumo y pantalla</h2></div>
        <Switch checked={draft.pause_on_battery} onChange={v => patch("pause_on_battery", v)} label="Reducir actividad con batería" description="Preferencia de ahorro cuando el dispositivo no está conectado."/>
        <Switch checked={draft.pause_when_maximized} onChange={v => patch("pause_when_maximized", v)} label="Reducir actividad con pantalla completa" description="Evita trabajo de fondo innecesario mientras usas otras aplicaciones."/>
        <label className="mt-4 block text-sm font-medium">Modo de ajuste</label><select className="input mt-2" value={draft.scaling_mode} onChange={e => patch("scaling_mode", e.target.value)}><option value="fill">Rellenar pantalla</option><option value="fit">Ajustar completa</option><option value="stretch">Estirar</option><option value="center">Centrar</option></select>
      </section>
    </div>
    <div className="panel panel-pad text-sm leading-6 muted"><strong className="text-slate-200">Transparencia:</strong> estas opciones ya se guardan en SQLite. Las integraciones del sistema que requieren ejecución en segundo plano —inicio automático, bandeja y detección de batería/ventana maximizada— se conservan como preferencias, pero no se presentan como activas hasta que exista ese servicio residente.</div>
  </div>;
}
