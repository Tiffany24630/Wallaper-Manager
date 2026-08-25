import { ImagePlus, Search, Shuffle } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import WallpaperCard from "../../components/wallpaper/WallpaperCard/WallpaperCard";
import { useWallpaperStore } from "../../store/wallpaperStore";

export default function Library() {
  const store = useWallpaperStore();
  const [query, setQuery] = useState("");
  const [favorites, setFavorites] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  useEffect(() => { void store.loadWallpapers(); }, [store.loadWallpapers]);
  const visible = useMemo(() => store.wallpapers.filter(w => (!favorites || w.favorite) && w.name.toLowerCase().includes(query.toLowerCase())), [store.wallpapers, query, favorites]);

  async function run(action: () => Promise<unknown>, success?: string) {
    setBusy(true); setMessage(null);
    try { await action(); if (success) setMessage(success); }
    catch (error) { setMessage(String(error)); }
    finally { setBusy(false); }
  }
  const remove = (id: string) => { if (window.confirm("¿Eliminar este fondo y su copia administrada por Lumina?")) void run(() => store.deleteWallpaper(id), "Fondo eliminado."); };

  return <div className="page">
    <header className="page-header">
      <div><div className="eyebrow">Colección local</div><h1 className="page-title">Biblioteca</h1><p className="page-subtitle">Importa imágenes, marca favoritas y aplica un fondo a Windows.</p></div>
      <div className="flex flex-wrap gap-2">
        <button className="btn" disabled={busy || !store.wallpapers.length} onClick={() => void run(store.rotateWallpaper, "Se aplicó un fondo aleatorio.")}><Shuffle size={16}/> Aleatorio</button>
        <button className="btn btn-primary" disabled={busy} onClick={() => void run(store.selectAndImportWallpaper, "Imagen añadida a la biblioteca.")}><ImagePlus size={17}/> Importar imagen</button>
      </div>
    </header>
    {(store.error || message) && <div className={(store.error || message)?.includes("eliminado") || message?.includes("añadida") || message?.includes("aplicó") ? "status-success" : "status-error"}>{store.error || message}</div>}
    <div className="panel flex flex-wrap items-center gap-3 p-3">
      <label className="relative min-w-[220px] flex-1"><Search size={16} className="absolute left-3 top-3.5 text-slate-500"/><input className="input pl-10" value={query} onChange={e => setQuery(e.target.value)} placeholder="Buscar por nombre…" /></label>
      <button className={`btn ${favorites ? "bg-rose-500/15 text-rose-200" : ""}`} onClick={() => setFavorites(v => !v)}>Favoritos {favorites ? "activados" : ""}</button>
      <span className="px-2 text-sm muted">{visible.length} de {store.wallpapers.length}</span>
    </div>
    {store.loading ? <div className="empty">Cargando biblioteca…</div> : visible.length === 0 ? <div className="empty"><ImagePlus className="mx-auto mb-3"/> {store.wallpapers.length ? "No hay coincidencias." : "Todavía no hay fondos. Importa tu primera imagen para comenzar."}</div> :
      <section className="grid gap-5 sm:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4">{visible.map(w => <WallpaperCard key={w.id} wallpaper={w} onActivate={id => void run(() => store.setActive(id), "Fondo aplicado.")} onFavorite={id => void run(() => store.toggleFavorite(id))} onDelete={remove}/>)}</section>}
  </div>;
}
