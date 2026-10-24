import { ChevronDown, ChevronUp, ImageOff, ListPlus, Pencil, Plus, Trash2, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import type { FormEvent } from "react";
import { playlistService } from "../../services/playlistService";
import { usePlaylistStore } from "../../store/playlistStore";
import { useWallpaperStore } from "../../store/wallpaperStore";
import { useSettingsStore } from "../../store/settingsStore";
import { wallpaperThumbnailSrc } from "../../utils/wallpaperPreview";

export default function PlaylistsPage() {
  const { playlists, loadPlaylists, createPlaylist, renamePlaylist, deletePlaylist, reorderPlaylist, error } = usePlaylistStore();
  const { wallpapers, loadWallpapers } = useWallpaperStore();
  const { settings, loadSettings } = useSettingsStore();
  const [name, setName] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const [members, setMembers] = useState<string[]>([]);
  const [wallpaperId, setWallpaperId] = useState("");
  const [failedPreviews, setFailedPreviews] = useState<string[]>([]);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => { void loadPlaylists(); void loadWallpapers(); void loadSettings(); }, [loadPlaylists, loadWallpapers, loadSettings]);
  useEffect(() => { const timer = window.setInterval(() => void loadSettings(), 5000); return () => window.clearInterval(timer); }, [loadSettings]);
  useEffect(() => { if (!selected && playlists[0]) setSelected(playlists[0].id); }, [playlists, selected]);
  useEffect(() => {
    let cancelled = false;
    setMembers([]);
    if (!selected) return () => { cancelled = true; };

    void playlistService.getPlaylistWallpapers(selected)
      .then(nextMembers => {
        if (!cancelled) setMembers(nextMembers);
      })
      .catch(errorValue => {
        if (!cancelled) setMessage(String(errorValue));
      });

    return () => { cancelled = true; };
  }, [selected]);

  const selectedPlaylist = playlists.find(item => item.id === selected);
  const memberWallpapers = useMemo(() => members.map(id => wallpapers.find(wallpaper => wallpaper.id === id)), [members, wallpapers]);
  const available = wallpapers.filter(wallpaper => !members.includes(wallpaper.id));

  async function create(event: FormEvent) { event.preventDefault(); try { await createPlaylist(name); setName(""); setMessage("Playlist creada."); } catch (errorValue) { setMessage(String(errorValue)); } }
  async function add() { if (!selected || !wallpaperId) return; try { await playlistService.addWallpaper(selected, wallpaperId); setMembers(await playlistService.getPlaylistWallpapers(selected)); setWallpaperId(""); setMessage("Wallpaper añadido."); } catch (errorValue) { setMessage(String(errorValue)); } }
  async function remove(id: string) { if (!selected) return; try { await playlistService.removeWallpaper(selected, id); setMembers(await playlistService.getPlaylistWallpapers(selected)); setMessage("Wallpaper retirado."); } catch (errorValue) { setMessage(String(errorValue)); } }
  async function move(index: number, direction: -1 | 1) { if (!selected) return; const next = [...members]; const target = index + direction; if (target < 0 || target >= next.length) return; [next[index], next[target]] = [next[target], next[index]]; try { await reorderPlaylist(selected, next); setMembers(next); setMessage("Orden actualizado."); } catch (errorValue) { setMessage(String(errorValue)); } }
  async function rename(id: string, currentName: string) { const nextName = window.prompt("Nuevo nombre de la playlist", currentName); if (nextName === null) return; try { await renamePlaylist(id, nextName); setMessage("Playlist renombrada."); } catch (errorValue) { setMessage(String(errorValue)); } }
  async function removeList(id: string) { if (!window.confirm("Eliminar esta lista? Las imágenes permanecerán en tu biblioteca.")) return; try { await deletePlaylist(id); if (selected === id) setSelected(null); setMessage("Playlist eliminada."); } catch (errorValue) { setMessage(String(errorValue)); } }

  return <div className="page">
    <header><div className="eyebrow">Secuencias</div><h1 className="page-title">Listas de fondos</h1><p className="page-subtitle">Agrupa imágenes para organizar colecciones y preparar rotaciones.</p></header>
    {(error || message) && <div className={error ? "status-error" : "status-success"}>{error || message}</div>}
    <div className="grid gap-5 lg:grid-cols-[320px_1fr]">
      <aside className="panel panel-pad self-start"><form className="flex gap-2" onSubmit={create}><input className="input" value={name} onChange={event => setName(event.target.value)} maxLength={80} placeholder="Nueva lista"/><button className="btn btn-primary w-11 px-0" aria-label="Crear"><Plus size={18}/></button></form><div className="mt-5 grid gap-2">{playlists.length === 0 && <div className="py-8 text-center text-sm muted">No hay listas todavía.</div>}{playlists.map(list => <div key={list.id} className={`flex items-center rounded-xl border ${selected === list.id ? "border-violet-400/30 bg-violet-500/10" : "border-transparent bg-white/[.03]"}`}><button className="flex-1 truncate px-3 py-3 text-left text-sm" onClick={() => setSelected(list.id)}>{list.name}<span className="ml-2 text-xs muted">{list.wallpaperCount}</span>{settings?.dynamic_enabled && settings.dynamic_playlist_id === list.id && <span className="ml-2 rounded-full bg-violet-500/20 px-2 py-1 text-[10px] text-violet-200">Fondo dinámico</span>}</button><button className="p-2 text-slate-500 hover:text-slate-200" aria-label={`Renombrar ${list.name}`} onClick={() => void rename(list.id, list.name)}><Pencil size={15}/></button><button className="mr-1 p-2 text-slate-500 hover:text-rose-300" aria-label="Eliminar lista" onClick={() => void removeList(list.id)}><Trash2 size={15}/></button></div>)}</div></aside>
      <section className="panel panel-pad min-h-[420px]">{!selectedPlaylist ? <div className="empty"><ListPlus className="mx-auto mb-3"/>Crea o selecciona una lista.</div> : <><div className="flex flex-wrap items-center justify-between gap-3"><div><h2 className="text-xl font-semibold">{selectedPlaylist.name}</h2><div className="mt-1 text-sm muted">{members.length} {members.length === 1 ? "imagen" : "imágenes"}</div></div><div className="min-w-[300px] flex-1"><div className="grid max-h-36 gap-2 overflow-y-auto rounded-xl border border-white/10 p-2 sm:grid-cols-2">{available.length === 0 ? <div className="p-2 text-sm muted">Todos los fondos ya están en la lista.</div> : available.map(wallpaper => { const preview = wallpaperThumbnailSrc(wallpaper); const failed = failedPreviews.includes(wallpaper.id); return <button key={wallpaper.id} type="button" onClick={() => setWallpaperId(wallpaper.id)} className={`flex items-center gap-2 rounded-lg border p-1.5 text-left ${wallpaperId === wallpaper.id ? "border-violet-400 bg-violet-500/10" : "border-transparent bg-white/[.03]"}`}><div className="h-10 w-14 shrink-0 overflow-hidden rounded bg-white/5">{preview && !failed ? <img src={preview} alt="" className="h-full w-full object-cover" onError={() => setFailedPreviews(old => old.includes(wallpaper.id) ? old : [...old, wallpaper.id])}/> : <div className="grid h-full place-items-center text-slate-500" title="Miniatura no disponible"><ImageOff size={15}/></div>}</div><span className="truncate text-xs">{wallpaper.name}</span></button>; })}</div><button className="btn btn-primary mt-2 w-full" onClick={() => void add()} disabled={!wallpaperId}>Añadir fondo seleccionado</button></div></div>{memberWallpapers.length === 0 ? <div className="empty mt-6">Esta lista está vacía. Selecciona un wallpaper arriba para agregarlo.</div> : <div className="mt-6 grid gap-3 sm:grid-cols-2 xl:grid-cols-3">{memberWallpapers.map((wallpaper, index) => <article key={members[index]} className="relative overflow-hidden rounded-xl border border-white/10 bg-white/[.03]"><div className="aspect-video bg-white/5">{wallpaper && wallpaperThumbnailSrc(wallpaper) ? <img className="h-full w-full object-cover" src={wallpaperThumbnailSrc(wallpaper)!} alt={wallpaper.name}/> : <div className="grid h-full place-items-center gap-1 text-xs muted"><ImageOff size={18}/><span>{wallpaper ? "Miniatura no disponible" : "Wallpaper no disponible"}</span></div>}</div><div className="flex items-center gap-2 p-3"><div className="flex-1 truncate text-sm">{wallpaper?.name ?? "Wallpaper no disponible"}</div><button className="p-1 text-slate-500 disabled:opacity-30" onClick={() => void move(index, -1)} disabled={index === 0} aria-label="Mover arriba"><ChevronUp size={16}/></button><button className="p-1 text-slate-500 disabled:opacity-30" onClick={() => void move(index, 1)} disabled={index === members.length - 1} aria-label="Mover abajo"><ChevronDown size={16}/></button><button className="text-slate-500 hover:text-rose-300" onClick={() => void remove(members[index])} aria-label="Quitar de la lista"><X size={16}/></button></div></article>)}</div>}</>}</section>
    </div>
  </div>;
}
