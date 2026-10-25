import { ImagePlus, Search, Shuffle } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import WallpaperCard from "../../components/wallpaper/WallpaperCard/WallpaperCard";
import UpscaleComparisonDialog from "../../components/wallpaper/UpscaleComparisonDialog";
import { monitorService } from "../../services/monitorService";
import { useWallpaperStore } from "../../store/wallpaperStore";
import type { Wallpaper } from "../../types/wallpaper";

export default function Library() {
  const store = useWallpaperStore();
  const [query, setQuery] = useState("");
  const [favorites, setFavorites] = useState(false);
  const [filter, setFilter] = useState<"all" | "available" | "missing" | "static" | "derived">("all");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [renameId, setRenameId] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [deleteTarget, setDeleteTarget] = useState<Wallpaper | null>(null);
  const [comparison, setComparison] = useState<{ originalId: string; enhancedId: string } | null>(null);
  const [monitors, setMonitors] = useState<Awaited<ReturnType<typeof monitorService.getAll>>>([]);
  const [monitorId, setMonitorId] = useState("");
  const [monitorWallpapers, setMonitorWallpapers] = useState<Awaited<ReturnType<typeof monitorService.getWallpapers>>>([]);
  const [monitorError, setMonitorError] = useState<string | null>(null);
  useEffect(() => { void store.loadWallpapers(); }, [store.loadWallpapers]);
  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    void Promise.resolve((event?: { state?: string; error?: string }) => {
      if (!active) return;
      void store.refresh();
      setMessage(event.state === "ready"
        ? "Animación lista; la visibilidad física no está verificada."
        : `No se pudo aplicar la animación${event.error ? `: ${event.error}` : "."}`);
    }).then(() => {});
    return () => { active = false; unlisten?.(); };
  }, [store.refresh]);
  useEffect(() => {
    const latest = Object.values(store.animatedActivations).sort((a, b) => b.generation - a.generation)[0];
    if (!latest) return;
    if (latest.state === "pending") setMessage("Animación en carga; la visibilidad física aún no está confirmada.");
    else if (latest.state === "ready") setMessage("Animación lista; la visibilidad física no está verificada.");
    else if (latest.state === "failed") setMessage(`No se pudo aplicar la animación${latest.error ? `: ${latest.error}` : "."}`);
    else setMessage("La activación de la animación fue cancelada.");
  }, [store.animatedActivations]);
  useEffect(() => {
    void Promise.all([monitorService.getAll(), monitorService.getWallpapers()]).then(([items, assignments]) => {
      setMonitors(items);
      setMonitorWallpapers(assignments);
      setMonitorId(current => {
        const selected = current || localStorage.getItem("lumina.library.monitor") || items.find(item => item.primary)?.id || (items.length === 1 ? items[0].id : "");
        if (selected) localStorage.setItem("lumina.library.monitor", selected);
        return selected;
      });
      setMonitorError(null);
    }).catch(error => setMonitorError(String(error)));
  }, []);
  useEffect(() => {
    if (!deleteTarget) return;
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busy) setDeleteTarget(null);
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [deleteTarget, busy]);
  const visible = useMemo(() => {
    const normalizedQuery = query.normalize("NFKC").toLocaleLowerCase();
    return store.wallpapers.filter(w => {
      const matchesFilter = filter === "all"
        || (filter === "available" && w.asset?.available)
        || (filter === "missing" && !w.asset?.available)
        || (filter === "static" && w.resourceType === "static")
        || (filter === "derived" && Boolean(w.parentWallpaperId));
      return matchesFilter && (!favorites || w.favorite)
        && w.name.normalize("NFKC").toLocaleLowerCase().includes(normalizedQuery);
    });
  }, [store.wallpapers, query, favorites, filter]);

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
      const importedWallpaper = store.wallpapers.find(wallpaper => wallpaper.id === originalId);
      if (importedWallpaper?.resourceType === "animated") return;
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
      <div className="flex flex-wrap items-center gap-2">
        <label className="text-xs muted" htmlFor="library-monitor">Monitor destino</label>
        <select id="library-monitor" className="input max-w-[220px]" value={monitorId} onChange={event => { setMonitorId(event.target.value); localStorage.setItem("lumina.library.monitor", event.target.value); }}>
          <option value="">Selecciona un monitor</option>
          {monitors.map((monitor, index) => <option key={monitor.id} value={monitor.id}>{monitor.name || `Monitor ${index + 1}`}{monitor.primary ? " · Principal" : ""}</option>)}
        </select>
        <button className="btn" disabled={busy || !store.wallpapers.length} onClick={() => void run(store.rotateWallpaper, "Se aplicó un fondo aleatorio.")}><Shuffle size={16}/> Aleatorio</button>
        <button className="btn btn-primary" disabled={busy} onClick={() => void importAndOfferEnhancement()}><ImagePlus size={17}/> {busy ? "Procesando…" : "Importar imágenes"}</button>
      </div>
    </header>
    {(store.error || monitorError || message) && <div className={store.error || monitorError || message?.startsWith("Error:") ? "status-error" : "status-success"}>{store.error || monitorError || message}</div>}
    <div className="panel flex flex-wrap items-center gap-3 p-3">
      <label className="relative min-w-[220px] flex-1"><Search size={16} className="absolute left-3 top-3.5 text-slate-500"/><input className="input pl-10" value={query} onChange={e => setQuery(e.target.value)} placeholder="Buscar por nombre…" /></label>
      <button className={`btn ${favorites ? "bg-rose-500/15 text-rose-200" : ""}`} onClick={() => setFavorites(v => !v)}>Favoritos {favorites ? "activados" : ""}</button>
      <select className="input max-w-[190px]" value={filter} onChange={event => setFilter(event.target.value as typeof filter)} aria-label="Filtrar biblioteca">
        <option value="all">Todos</option><option value="available">Disponibles localmente</option><option value="missing">Archivos faltantes</option><option value="static">Estáticos</option><option value="derived">Derivados</option>
      </select>
      <span className="px-2 text-sm muted">{visible.length} de {store.wallpapers.length}</span>
    </div>
    {store.loading ? <div className="empty">Cargando biblioteca…</div> : store.error ? <div className="empty">No se pudo cargar la biblioteca. Revisa el error anterior y vuelve a intentarlo.</div> : visible.length === 0 ? <div className="empty"><ImagePlus className="mx-auto mb-3"/> {store.wallpapers.length ? "No hay coincidencias." : "Todavía no hay fondos. Importa tu primera imagen para comenzar."}</div> :
      <section className="grid gap-5 sm:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4">{visible.map(w => <WallpaperCard key={w.id} wallpaper={w} isActive={monitorWallpapers.some(item => item.monitorId === monitorId && item.wallpaperId === w.id)} busy={busy} onActivate={id => void run(async () => { if (!monitorId) throw new Error("Selecciona un monitor de destino antes de aplicar el fondo."); const state = await store.setActive(id, monitorId); setMonitorWallpapers(await monitorService.getWallpapers()); setMessage(state === "pending" ? "Animated en carga; la aplicación física y su visibilidad aún no están confirmadas." : "Fondo aplicado."); })} onFavorite={id => void run(() => store.toggleFavorite(id))} onDelete={remove} onRename={beginRename} onRotate={rotate} onRegenerateThumbnail={id => void run(() => store.regenerateThumbnail(id), "Miniatura regenerada.")}/>)}</section>}
    {renameId && <div className="modal-backdrop" role="dialog" aria-modal="true" aria-labelledby="rename-title"><div className="modal-card max-w-lg"><h2 id="rename-title" className="text-xl font-semibold">Renombrar fondo</h2><p className="mt-1 text-sm muted">El nombre visible cambiará en toda la biblioteca, sin mover el archivo.</p><input className="input mt-5" autoFocus value={renameValue} onChange={event => setRenameValue(event.target.value)} maxLength={200} onKeyDown={event => { if (event.key === "Enter") void confirmRename(); if (event.key === "Escape") setRenameId(null); }}/><div className="mt-5 flex justify-end gap-2"><button className="btn" onClick={() => setRenameId(null)} disabled={busy}>Cancelar</button><button className="btn btn-primary" onClick={() => void confirmRename()} disabled={busy}>Guardar</button></div></div></div>}
    {deleteTarget && <div className="modal-backdrop" role="dialog" aria-modal="true" aria-labelledby="delete-title" onClick={event => { if (event.target === event.currentTarget && !busy) setDeleteTarget(null); }}><div className="modal-card max-w-lg"><h2 id="delete-title" className="text-xl font-semibold">Eliminar wallpaper</h2><p className="mt-2 text-sm muted">¿Quieres eliminar <strong className="text-slate-200">{deleteTarget.name}</strong> de la biblioteca?</p><p className="mt-2 text-sm muted">Se eliminarán el registro y los archivos administrados por Lumina. Esta acción no se puede deshacer.</p><div className="mt-5 flex justify-end gap-2"><button className="btn" onClick={() => setDeleteTarget(null)} disabled={busy}>Cancelar</button><button className="btn btn-danger" onClick={() => void confirmDelete()} disabled={busy}>{busy ? "Eliminando…" : "Eliminar"}</button></div></div></div>}
    {original && enhanced && <UpscaleComparisonDialog original={original} enhanced={enhanced} busy={busy} onChoose={choice => void chooseVersion(choice)}/>}
  </div>;
}
