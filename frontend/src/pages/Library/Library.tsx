import MainLayout from "../../layouts/MainLayout/MainLayout";
import WallpaperCard from "../../components/wallpaper/WallpaperCard/WallpaperCard";

export default function LibraryPage() {
  return (
    <MainLayout>
      <div className="space-y-8">
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-5xl font-bold">
              Wallpaper Library
            </h1>

            <p className="text-zinc-400 mt-2">
              Manage your wallpapers and animated backgrounds.
            </p>
          </div>

          <button className="bg-blue-600 hover:bg-blue-500 transition px-6 py-3 rounded-2xl font-medium">
            Upload Wallpaper
          </button>
        </div>

        <div className="grid grid-cols-4 gap-6">
          <WallpaperCard />
          <WallpaperCard />
          <WallpaperCard />
          <WallpaperCard />
          <WallpaperCard />
          <WallpaperCard />
        </div>
      </div>
    </MainLayout>
  );
}