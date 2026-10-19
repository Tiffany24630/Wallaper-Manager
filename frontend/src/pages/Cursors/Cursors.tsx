import { convertFileSrc } from "@tauri-apps/api/core";
import { Check, MousePointer2, Pencil, RotateCcw, Trash2, Upload } from "lucide-react";
import { useEffect, useState } from "react";
import { cursorService } from "../../services/cursorService";
import { useSettingsStore } from "../../store/settingsStore";
import type { CursorResource } from "../../types/cursor";

export default function Cursors() {
  const { settings, loadSettings } = useSettingsStore();
  const [cursors, setCursors] = useState<CursorResource[]>([]);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [hotspotX, setHotspotX] = useState(0);
  const [hotspotY, setHotspotY] = useState(0);

  async function refresh() { setCursors(await cursorService.list()); }
  useEffect(() => { void refresh().catch(error => setMessage(`Error: ${String(error)}`)); void loadSettings(); }, [loadSettings]);

  async function importCursor() {
    const path = await cursorService.select();
    if (!path) return;
    setBusy(true); setMessage(null);
    try { await cursorService.import(path, undefined, hotspotX, hotspotY); await refresh(); setMessage("Cursor importado. Puedes activarlo cuando quieras."); }
    catch (error) { setMessage(`Error: ${String(error)}`); }
    finally { setBusy(false); }
  }
  async function activate(cursor: CursorResource) { setBusy(true); setMessage(null); try { await cursorService.activate(cursor.id); await loadSettings(); setMessage(`Cursor “${cursor.name}” activado globalmente en Windows.`); } catch (error) { setMessage(`Error: ${String(error)}`); } finally { setBusy(false); } }
  async function restore() { setBusy(true); setMessage(null); try { await cursorService.restore(); await loadSettings(); setMessage("Se restauró el cursor predeterminado de Windows."); } catch (error) { setMessage(`Error: ${String(error)}`); } finally { setBusy(false); } }
  async function rename(cursor: CursorResource) { const name = window.prompt("Nuevo nombre del cursor", cursor.name); if (name === null) return; setBusy(true); try { await cursorService.rename(cursor.id, name); await refresh(); setMessage("Nombre actualizado."); } catch (error) { setMessage(`Error: ${String(error)}`); } finally { setBusy(false); } }
  async function remove(cursor: CursorResource) { if (!window.confirm(`¿Eliminar “${cursor.name}”?`)) return; setBusy(true); try { await cursorService.remove(cursor.id); await refresh(); await loadSettings(); setMessage("Cursor eliminado."); } catch (error) { setMessage(`Error: ${String(error)}`); } finally { setBusy(false); } }

  return <div className="page">
    <header className="page-header"><div><div className="eyebrow">Personalización del sistema</div><h1 className="page-title">Cursores</h1><p className="page-subtitle">Importa cursores estáticos o animados y aplícalos globalmente en Windows. Las imágenes se convierten en archivos CUR administrados por Lumina.</p></div><div className="flex flex-wrap gap-2"><button className="btn" onClick={() => void restore()} disabled={busy}><RotateCcw size={16}/> Restaurar Windows</button><button className="btn btn-primary" onClick={() => void importCursor()} disabled={busy}><Upload size={16}/> Importar cursor</button></div></header>
    {(message || settings?.cursor_enabled) && <div className={message?.startsWith("Error:") ? "status-error" : "status-success"}>{message || "Hay un cursor personalizado activo en Windows."}</div>}
    <section className="panel panel-pad"><div className="flex flex-wrap items-end gap-4"><div><label className="block text-sm font-medium">Punto caliente para imágenes</label><p className="mt-1 text-xs muted">Se usa al convertir PNG, JPG, WEBP o BMP a CUR.</p></div><label className="text-sm muted">X<input className="input mt-1 w-20" type="number" min="0" max="255" value={hotspotX} onChange={event => setHotspotX(Number(event.target.value))}/></label><label className="text-sm muted">Y<input className="input mt-1 w-20" type="number" min="0" max="255" value={hotspotY} onChange={event => setHotspotY(Number(event.target.value))}/></label></div></section>
    {cursors.length === 0 ? <div className="empty"><MousePointer2 className="mx-auto mb-3"/>Todavía no hay cursores importados.</div> : <section className="grid gap-5 sm:grid-cols-2 xl:grid-cols-3">{cursors.map(cursor => <article key={cursor.id} className={`panel overflow-hidden ${settings?.active_cursor_id === cursor.id ? "ring-2 ring-violet-400/60" : ""}`}><div className="grid aspect-video place-items-center bg-white/[.03]">{cursor.preview ? <img src={convertFileSrc(cursor.preview)} alt={cursor.name} className="h-full w-full object-contain"/> : <MousePointer2 size={54} className="text-violet-300"/>}</div><div className="p-4"><div className="flex items-start justify-between gap-2"><div><h2 className="truncate font-semibold">{cursor.name}</h2><div className="mt-1 text-xs muted">{cursor.resourceType === "animated" ? "Animado (.ANI)" : "Estático (.CUR)"}</div></div>{settings?.active_cursor_id === cursor.id && <Check className="shrink-0 text-emerald-300" size={18}/>}</div><div className="mt-4 flex gap-2"><button className="btn btn-primary flex-1 text-sm" onClick={() => void activate(cursor)} disabled={busy || settings?.active_cursor_id === cursor.id}>{settings?.active_cursor_id === cursor.id ? "Activo" : "Activar"}</button><button className="btn w-11 px-0" aria-label="Renombrar cursor" onClick={() => void rename(cursor)} disabled={busy}><Pencil size={16}/></button><button className="btn btn-danger w-11 px-0" aria-label="Eliminar cursor" onClick={() => void remove(cursor)} disabled={busy}><Trash2 size={16}/></button></div></div></article>)}</section>}
  </div>;
}
