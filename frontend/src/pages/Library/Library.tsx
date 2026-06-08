import { useEffect } from "react";
import WallpaperCard from "../../components/wallpaper/WallpaperCard/WallpaperCard";
import { useWallpaperStore } from "../../store/wallpaperStore";

export default function Library() {
  const {
    wallpapers,
    loading,
    error,

    loadWallpapers,
    setActive,
    toggleFavorite,
    deleteWallpaper,
    selectAndImportWallpaper,
  } = useWallpaperStore();

  useEffect(() => {
    loadWallpapers();
  }, []);

  if (loading) {
    return (
      <div className="p-6">
        Cargando wallpapers...
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-6 text-red-500">
        {error}
      </div>
    );
  }

  return (
    <div className="flex items-center justify-between mb-8">
      <h1 className="text-3xl font-bold">
        Biblioteca
      </h1>

      <button
        onClick={selectAndImportWallpaper}
        className="
          px-4
          py-2
          rounded-lg
          bg-cyan-600
          hover:bg-cyan-700
        "
      >
        Importar wallpaper
      </button>

      {wallpapers.length === 0 ? (
        <div
          className="
            border
            border-dashed
            border-zinc-700
            rounded-xl
            p-10
            text-center
          "
        >
          No hay wallpapers registrados.
        </div>
      ) : (
        <div
          className="
            grid
            grid-cols-1
            md:grid-cols-2
            xl:grid-cols-3
            gap-6
          "
        >
          {wallpapers.map((wallpaper) => (
            <WallpaperCard
              key={wallpaper.id}
              wallpaper={wallpaper}
              onActivate={setActive}
              onFavorite={toggleFavorite}
              onDelete={deleteWallpaper}
            />
          ))}
        </div>
      )}
    </div>
  );
}