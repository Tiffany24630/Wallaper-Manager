import { Heart, MonitorUp, Pencil, Trash2 } from "lucide-react";
import type { Wallpaper } from "../../../types/wallpaper";
import { formatBytes } from "../../../utils/format";
import { wallpaperThumbnailSrc } from "../../../utils/wallpaperPreview";

interface Props {
  wallpaper: Wallpaper;
  onActivate: (id: string) => void;
  onFavorite: (id: string) => void;
  onDelete: (id: string) => void;
  onRename: (id: string) => void;
  onRotate: (id: string, degrees: 0 | 90 | 180 | 270) => void;
  busy?: boolean;
}

export default function WallpaperCard({ wallpaper, onActivate, onFavorite, onDelete, onRename, onRotate, busy }: Props) {
  const preview = wallpaperThumbnailSrc(wallpaper);
  return (
    <article className={`group overflow-hidden rounded-[18px] border bg-[#121520] transition ${wallpaper.active ? "border-violet-400/70 shadow-lg shadow-violet-500/10" : "border-white/10 hover:border-white/20"}`}>
      <div className="relative aspect-video overflow-hidden bg-white/5">
        {preview ? <img src={preview} alt={wallpaper.name} className="h-full w-full object-cover transition duration-500 group-hover:scale-[1.03]" /> : <div className="grid h-full place-items-center text-sm text-slate-500">Sin vista previa</div>}
        {wallpaper.active && <span className="absolute left-3 top-3 rounded-full bg-violet-500 px-2.5 py-1 text-[11px] font-bold uppercase tracking-wide">Activo</span>}
      </div>
      <div className="p-4">
        <h3 className="truncate font-semibold" title={wallpaper.name}>{wallpaper.name}</h3>
        <div className="mt-1 flex gap-2 text-xs text-slate-500">
          <span>{wallpaper.width && wallpaper.height ? `${wallpaper.width} × ${wallpaper.height}` : "Resolución desconocida"}</span>
          {wallpaper.sizeBytes ? <><span>·</span><span>{formatBytes(wallpaper.sizeBytes)}</span></> : null}
        </div>
        <div className="mt-4 flex gap-2">
          <button className="btn btn-primary flex-1 text-sm" onClick={() => onActivate(wallpaper.id)} disabled={wallpaper.active || busy}>
            <MonitorUp size={16} /> {wallpaper.active ? "Aplicado" : "Aplicar"}
          </button>
          <button className="btn w-11 px-0" aria-label={wallpaper.favorite ? "Quitar de favoritos" : "Añadir a favoritos"} onClick={() => onFavorite(wallpaper.id)}>
            <Heart size={17} className={wallpaper.favorite ? "fill-rose-400 text-rose-400" : ""} />
          </button>
          <button className="btn w-11 px-0" aria-label="Renombrar" onClick={() => onRename(wallpaper.id)} disabled={busy}><Pencil size={16}/></button>
          <label className="sr-only" htmlFor={`rotate-${wallpaper.id}`}>Rotar {wallpaper.name}</label>
          <select id={`rotate-${wallpaper.id}`} className="input max-w-[100px] px-2 text-xs" defaultValue="" disabled={busy} onChange={event => { const value = event.target.value; event.target.value = ""; if (value) onRotate(wallpaper.id, Number(value) as 0 | 90 | 180 | 270); }}>
            <option value="" disabled>Rotar</option>
            <option value="90">90°</option><option value="180">180°</option><option value="270">270°</option><option value="0">Original</option>
          </select>
          <button className="btn btn-danger w-11 px-0" aria-label="Eliminar" onClick={() => onDelete(wallpaper.id)} disabled={busy}><Trash2 size={17} /></button>
        </div>
      </div>
    </article>
  );
}
