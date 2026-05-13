export default function WallpaperCard() {
  return (
    <div className="bg-zinc-800 border border-zinc-700 rounded-3xl overflow-hidden hover:scale-[1.02] hover:border-blue-500 transition-all duration-300 cursor-pointer">
      <div className="aspect-video bg-gradient-to-br from-blue-600 via-purple-600 to-pink-600" />

      <div className="p-5">
        <div className="flex items-start justify-between">
          <div>
            <h3 className="text-xl font-semibold">
              Cyberpunk City
            </h3>

            <p className="text-zinc-400 mt-1 text-sm">
              1920x1080 • MP4
            </p>
          </div>

          <div className="bg-green-500/20 text-green-400 text-xs px-3 py-1 rounded-full">
            Active
          </div>
        </div>

        <button className="w-full mt-5 bg-blue-600 hover:bg-blue-500 transition py-3 rounded-2xl font-medium">
          Apply Wallpaper
        </button>
      </div>
    </div>
  );
}