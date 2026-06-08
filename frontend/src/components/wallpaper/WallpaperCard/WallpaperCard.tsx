import { Heart, MonitorPlay, Trash2 } from "lucide-react";
import type { Wallpaper } from "../../../types/wallpaper";

interface Props {
  wallpaper: Wallpaper;

  onActivate: (id: string) => void;

  onFavorite: (id: string) => void;

  onDelete: (id: string) => void;
}

export default function WallpaperCard({
  wallpaper,
  onActivate,
  onFavorite,
  onDelete,
}: Props) {
  return (
    <div
      className={`
        rounded-xl
        border
        bg-zinc-900
        overflow-hidden
        shadow-md
        transition-all

        ${wallpaper.active ? "border-cyan-500" : "border-zinc-800"}
      `}
    >
      <div className="aspect-video bg-zinc-800 flex items-center justify-center">
        {wallpaper.thumbnail ? (
          <img
            src={wallpaper.thumbnail}
            alt={wallpaper.name}
            className="w-full h-full object-cover"
          />
        ) : (
          <span className="text-zinc-500">
            Sin vista previa
          </span>
        )}
      </div>

      <div className="p-4">
        <h3 className="font-semibold truncate">
          {wallpaper.name}
        </h3>

        <p className="text-sm text-zinc-500 truncate">
          {wallpaper.path}
        </p>

        <div className="flex gap-2 mt-4">
          <button
            onClick={() => onActivate(wallpaper.id)}
            className="
              flex-1
              flex
              items-center
              justify-center
              gap-2
              px-3
              py-2
              rounded-lg
              bg-cyan-600
              hover:bg-cyan-700
            "
          >
            <MonitorPlay size={18} />
            Aplicar
          </button>

          <button
            onClick={() => onFavorite(wallpaper.id)}
            className="
              p-2
              rounded-lg
              bg-zinc-800
              hover:bg-zinc-700
            "
          >
            <Heart
              size={18}
              fill={wallpaper.favorite ? "currentColor" : "none"}
            />
          </button>

          <button
            onClick={() => onDelete(wallpaper.id)}
            className="
              p-2
              rounded-lg
              bg-red-600
              hover:bg-red-700
            "
          >
            <Trash2 size={18} />
          </button>
        </div>
      </div>
    </div>
  );
}