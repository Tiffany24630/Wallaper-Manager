import { convertFileSrc } from "@tauri-apps/api/core";
import { ListPlus, Plus, Trash2, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import type { FormEvent } from "react";
import { playlistService } from "../../services/playlistService";
import { usePlaylistStore } from "../../store/playlistStore";
import { useWallpaperStore } from "../../store/wallpaperStore";

export default function PlaylistsPage() {
  const { playlists, loadPlaylists, createPlaylist, deletePlaylist, error } = usePlaylistStore();
  const { wallpapers, loadWallpapers } = useWallpaperStore();
  const [name, setName] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const [members, setMembers] = useState<string[]>([]);
  const [wallpaperId, setWallpaperId] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  useEffect(() => { void loadPlaylists(); void loadWallpapers(); }, [loadPlaylists, loadWallpapers]);
  useEffect(() => { if (!selected && playlists[0]) setSelected(playlists[0].id); }, [playlists, selected]);
  useEffect(() => { if (selected) void playlistService.getPlaylistWallpapers(selected).then(setMembers); else setMembers([]); }, [selected]);
  const selectedPlaylist = playlists.find(p => p.id === selected);
  const memberWallpapers = useMemo(() => members.map(id => wallpapers.find(w => w.id === id)).filter(Boolean), [members, wallpapers]);
  const available = wallpapers.filter(w => !members.includes(w.id));

  async function create(event: FormEvent) { event.preventDefault(); if (!name.trim()) return; await createPlaylist(name.trim()); setName(""); setMessage("Lista creada."); }
  async function add() { if (!selected || !wallpaperId) return; try { await playlistService.addWallpaper(selected, wallpaperId); setMembers(await playlistService.getPlaylistWallpapers(selected)); setWallpaperId(""); } catch (e) { setMessage(String(e)); } }
  async function remove(id: string) { if (!selected) return; await playlistService.removeWallpaper(selected, id); setMembers(await playlistService.getPlaylistWallpapers(selected)); }
  async function removeList(id: string) { if (!window.confirm("¿Eliminar esta lista? Las imágenes permanecerán en tu biblioteca.")) return; await deletePlaylist(id); if (selected === id) setSelected(null); }

  return <div className="page">
    <header><div className="eyebrow">Secuencias</div><h1 className="page-title">Listas de fondos</h1><p className="page-subtitle">Agrupa imágenes para organizar colecciones y preparar rotaciones.</p></header>
    {(error || message) && <div className={error ? "status-error" : "status-success"}>{error || message}</div>}
    <div className="grid gap-5 lg:grid-cols-[320px_1fr]">
      <aside className="panel panel-pad self-start">
        <form className="flex gap-2" onSubmit={create}><input className="input" value={name} onChange={e => setName(e.target.value)} maxLength={80} placeholder="Nueva lista"/><button className="btn btn-primary w-11 px-0" aria-label="Crear"><Plus size={18}/></button></form>
        <div className="mt-5 grid gap-2">
          {playlists.length === 0 && <div className="py-8 text-center text-sm muted">No hay listas todavía.</div>}
          {playlists.map(list => <div key={list.id} className={`flex items-center rounded-xl border ${selected === list.id ? "border-violet-400/30 bg-violet-500/10" : "border-transparent bg-white/[.03]"}`}><button className="flex-1 truncate px-3 py-3 text-left text-sm" onClick={() => setSelected(list.id)}>{list.name}</button><button className="mr-1 p-2 text-slate-500 hover:text-rose-300" aria-label="Eliminar lista" onClick={() => void removeList(list.id)}><Trash2 size={15}/></button></div>)}
        </div>
      </aside>
      <section className="panel panel-pad min-h-[420px]">
        {!selectedPlaylist ? <div className="empty"><ListPlus className="mx-auto mb-3"/>Crea o selecciona una lista.</div> : <>
          <div className="flex flex-wrap items-center justify-between gap-3"><div><h2 className="text-xl font-semibold">{selectedPlaylist.name}</h2><div className="mt-1 text-sm muted">{members.length} {members.length === 1 ? "imagen" : "imágenes"}</div></div>
            <div className="flex min-w-[300px] gap-2"><select className="input" value={wallpaperId} onChange={e => setWallpaperId(e.target.value)}><option value="">Selecciona un fondo…</option>{available.map(w => <option key={w.id} value={w.id}>{w.name}</option>)}</select><button className="btn btn-primary" onClick={() => void add()} disabled={!wallpaperId}>Añadir</button></div>
          </div>
          {memberWallpapers.length === 0 ? <div className="empty mt-6">Esta lista está vacía.</div> : <div className="mt-6 grid gap-3 sm:grid-cols-2 xl:grid-cols-3">{memberWallpapers.map(w => w && <article key={w.id} className="relative overflow-hidden rounded-xl border border-white/10 bg-white/[.03]"><div className="aspect-video bg-white/5">{w.thumbnail && <img className="h-full w-full object-cover" src={convertFileSrc(w.thumbnail)} alt={w.name}/>}</div><div className="flex items-center gap-2 p-3"><div className="flex-1 truncate text-sm">{w.name}</div><button className="text-slate-500 hover:text-rose-300" onClick={() => void remove(w.id)} aria-label="Quitar de la lista"><X size={16}/></button></div></article>)}</div>}
        </>}
      </section>
    </div>
  </div>;
}
