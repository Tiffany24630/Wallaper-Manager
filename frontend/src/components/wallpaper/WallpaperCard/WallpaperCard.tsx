import { convertFileSrc } from "@tauri-apps/api/core";
import { Heart, MonitorUp, Trash2 } from "lucide-react";
import type { Wallpaper } from "../../../types/wallpaper";
import { formatBytes } from "../../../utils/format";

interface Props {
  wallpaper: Wallpaper;
  onActivate: (id: string) => void;
  onFavorite: (id: string) => void;
  onDelete: (id: string) => void;
}

export default function WallpaperCard({ wallpaper, onActivate, onFavorite, onDelete }: Props) {
  const preview = wallpaper.thumbnail ? convertFileSrc(wallpaper.thumbnail) : null;
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
          <button className="btn btn-primary flex-1 text-sm" onClick={() => onActivate(wallpaper.id)} disabled={wallpaper.active}>
            <MonitorUp size={16} /> {wallpaper.active ? "Aplicado" : "Aplicar"}
          </button>
          <button className="btn w-11 px-0" aria-label={wallpaper.favorite ? "Quitar de favoritos" : "Añadir a favoritos"} onClick={() => onFavorite(wallpaper.id)}>
            <Heart size={17} className={wallpaper.favorite ? "fill-rose-400 text-rose-400" : ""} />
          </button>
          <button className="btn btn-danger w-11 px-0" aria-label="Eliminar" onClick={() => onDelete(wallpaper.id)}><Trash2 size={17} /></button>
        </div>
      </div>
    </article>
  );
}
