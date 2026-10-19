import { ImagePlus, Search, Shuffle } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import WallpaperCard from "../../components/wallpaper/WallpaperCard/WallpaperCard";
import UpscaleComparisonDialog from "../../components/wallpaper/UpscaleComparisonDialog";
import { useWallpaperStore } from "../../store/wallpaperStore";
import type { Wallpaper } from "../../types/wallpaper";

export default function Library() {
  const store = useWallpaperStore();
  const [query, setQuery] = useState("");
  const [favorites, setFavorites] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [renameId, setRenameId] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [deleteTarget, setDeleteTarget] = useState<Wallpaper | null>(null);
  const [comparison, setComparison] = useState<{ originalId: string; enhancedId: string } | null>(null);
  useEffect(() => { void store.loadWallpapers(); }, [store.loadWallpapers]);
  useEffect(() => {
    if (!deleteTarget) return;
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busy) setDeleteTarget(null);
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [deleteTarget, busy]);
  const visible = useMemo(() => store.wallpapers.filter(w => (!favorites || w.favorite) && w.name.toLowerCase().includes(query.toLowerCase())), [store.wallpapers, query, favorites]);

  async function run(action: () => Promise<unknown>, success?: string) {
    setBusy(true); setMessage(null);
    try { await action(); if (success) setMessage(success); }
    catch (error) { setMessage(`Error: ${String(error)}`); }
    finally { setBusy(false); }
  }
  const remove = (id: string) => {
    if (busy) return;
    const wallpaper = store.wallpapers.find(item => item.id === id);
    if (wallpaper) setDeleteTarget(wallpaper);
  };

  async function confirmDelete() {
    if (!deleteTarget || busy) return;
    setBusy(true);
    setMessage(null);
    try {
      await store.deleteWallpaper(deleteTarget.id);
      setDeleteTarget(null);
      setMessage("Fondo eliminado.");
    } catch (error) {
      setMessage(`Error: ${String(error)}`);
    } finally {
      setBusy(false);
    }
  }

  function beginRename(id: string) {
    const wallpaper = store.wallpapers.find(item => item.id === id);
    if (wallpaper) { setRenameId(id); setRenameValue(wallpaper.name); setMessage(null); }
  }
  async function confirmRename() {
    if (!renameId) return;
    setBusy(true);
    setMessage(null);
    try {
      await store.renameWallpaper(renameId, renameValue);
      setRenameId(null);
      setRenameValue("");
      setMessage("Nombre actualizado.");
    } catch (error) {
      setMessage(`Error: ${String(error)}`);
    } finally {
      setBusy(false);
    }
  }
  const rotate = (id: string, degrees: 0 | 90 | 180 | 270) => void run(() => store.rotateWallpaperVersion(id, degrees), degrees === 0 ? "Se restauró la orientación original." : `Fondo rotado ${degrees}°.`);

  async function importAndOfferEnhancement() {
    setBusy(true); setMessage(null);
    try {
      const files = await store.selectWallpaperFiles();
      if (!files.length) return;
      const batchResult = await store.importWallpapers(files, (current, total) => setMessage(`Importando ${current} de ${total} imágenes…`));
      if (!batchResult.imported.length) {
        setMessage(`No se importó ninguna imagen. ${batchResult.failed.join(" ")}`);
        return;
      }
      if (batchResult.failed.length) setMessage(`Importadas ${batchResult.imported.length} de ${files.length}. Fallos: ${batchResult.failed.join(" ")}`);
      else setMessage(`${batchResult.imported.length} ${batchResult.imported.length === 1 ? "imagen importada" : "imágenes importadas"}.`);
      if (batchResult.imported.length !== 1) return;
      const originalId = batchResult.imported[0];
      if (!window.confirm("Imagen importada. ¿Quieres mejorar su calidad con IA? Se conservará el original hasta que compares ambas versiones.")) return;
      setMessage("Mejorando la imagen; este proceso puede tardar varios minutos…");
      const upscaleResult = await store.upscaleWallpaper(originalId, 4);
      setComparison({ originalId, enhancedId: upscaleResult.id });
      setMessage(upscaleResult.warning ? `Aviso: ${upscaleResult.warning}` : null);
    } catch (error) { setMessage(`Error: ${String(error)}`); }
    finally { setBusy(false); }
  }

  async function chooseVersion(choice: "original" | "enhanced") {
    if (!comparison) return;
    setBusy(true);
    try {
      const original = store.wallpapers.find(w => w.id === comparison.originalId);
      if (choice === "enhanced" && original?.active) await store.setActive(comparison.enhancedId);
      await store.deleteWallpaper(choice === "original" ? comparison.enhancedId : comparison.originalId);
      setComparison(null);
      setMessage(choice === "original" ? "Se guardó la imagen original." : "Se guardó la versión mejorada.");
    } catch (error) { setMessage(`Error: ${String(error)}`); }
    finally { setBusy(false); }
  }

  const original = comparison && store.wallpapers.find(w => w.id === comparison.originalId);
  const enhanced = comparison && store.wallpapers.find(w => w.id === comparison.enhancedId);

  return <div className="page">
    <header className="page-header">
      <div><div className="eyebrow">Colección local</div><h1 className="page-title">Biblioteca</h1><p className="page-subtitle">Importa una o varias imágenes, marca favoritas y aplica un fondo a Windows.</p></div>
      <div className="flex flex-wrap gap-2">
        <button className="btn" disabled={busy || !store.wallpapers.length} onClick={() => void run(store.rotateWallpaper, "Se aplicó un fondo aleatorio.")}><Shuffle size={16}/> Aleatorio</button>
        <button className="btn btn-primary" disabled={busy} onClick={() => void importAndOfferEnhancement()}><ImagePlus size={17}/> {busy ? "Procesando…" : "Importar imágenes"}</button>
      </div>
    </header>
    {(store.error || message) && <div className={store.error || message?.startsWith("Error:") ? "status-error" : "status-success"}>{store.error || message}</div>}
    <div className="panel flex flex-wrap items-center gap-3 p-3">
      <label className="relative min-w-[220px] flex-1"><Search size={16} className="absolute left-3 top-3.5 text-slate-500"/><input className="input pl-10" value={query} onChange={e => setQuery(e.target.value)} placeholder="Buscar por nombre…" /></label>
      <button className={`btn ${favorites ? "bg-rose-500/15 text-rose-200" : ""}`} onClick={() => setFavorites(v => !v)}>Favoritos {favorites ? "activados" : ""}</button>
      <span className="px-2 text-sm muted">{visible.length} de {store.wallpapers.length}</span>
    </div>
    {store.loading ? <div className="empty">Cargando biblioteca…</div> : visible.length === 0 ? <div className="empty"><ImagePlus className="mx-auto mb-3"/> {store.wallpapers.length ? "No hay coincidencias." : "Todavía no hay fondos. Importa tu primera imagen para comenzar."}</div> :
      <section className="grid gap-5 sm:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4">{visible.map(w => <WallpaperCard key={w.id} wallpaper={w} busy={busy} onActivate={id => void run(() => store.setActive(id), "Fondo aplicado.")} onFavorite={id => void run(() => store.toggleFavorite(id))} onDelete={remove} onRename={beginRename} onRotate={rotate}/>)}</section>}
    {renameId && <div className="modal-backdrop" role="dialog" aria-modal="true" aria-labelledby="rename-title"><div className="modal-card max-w-lg"><h2 id="rename-title" className="text-xl font-semibold">Renombrar fondo</h2><p className="mt-1 text-sm muted">El nombre visible cambiará en toda la biblioteca, sin mover el archivo.</p><input className="input mt-5" autoFocus value={renameValue} onChange={event => setRenameValue(event.target.value)} maxLength={200} onKeyDown={event => { if (event.key === "Enter") void confirmRename(); if (event.key === "Escape") setRenameId(null); }}/><div className="mt-5 flex justify-end gap-2"><button className="btn" onClick={() => setRenameId(null)} disabled={busy}>Cancelar</button><button className="btn btn-primary" onClick={() => void confirmRename()} disabled={busy}>Guardar</button></div></div></div>}
    {deleteTarget && <div className="modal-backdrop" role="dialog" aria-modal="true" aria-labelledby="delete-title" onClick={event => { if (event.target === event.currentTarget && !busy) setDeleteTarget(null); }}><div className="modal-card max-w-lg"><h2 id="delete-title" className="text-xl font-semibold">Eliminar wallpaper</h2><p className="mt-2 text-sm muted">¿Quieres eliminar <strong className="text-slate-200">{deleteTarget.name}</strong> de la biblioteca?</p><p className="mt-2 text-sm muted">Se eliminarán el registro y los archivos administrados por Lumina. Esta acción no se puede deshacer.</p><div className="mt-5 flex justify-end gap-2"><button className="btn" onClick={() => setDeleteTarget(null)} disabled={busy}>Cancelar</button><button className="btn btn-danger" onClick={() => void confirmDelete()} disabled={busy}>{busy ? "Eliminando…" : "Eliminar"}</button></div></div></div>}
    {original && enhanced && <UpscaleComparisonDialog original={original} enhanced={enhanced} busy={busy} onChoose={choice => void chooseVersion(choice)}/>}
  </div>;
}
