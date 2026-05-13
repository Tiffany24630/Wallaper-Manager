import MainLayout from "../../layouts/MainLayout/MainLayout";

export default function SettingsPage() {
  return (
    <MainLayout>
      <h1 className="text-3xl font-bold mb-6">
        Settings
      </h1>

      <div className="bg-zinc-800 rounded-2xl p-6 space-y-4">
        <div>
          <label className="block mb-2">
            FPS Limit
          </label>

          <input
            type="range"
            min="15"
            max="60"
            className="w-full"
          />
        </div>

        <div>
          <label className="flex items-center gap-3">
            <input type="checkbox" />
            Pause wallpapers on fullscreen
          </label>
        </div>
      </div>
    </MainLayout>
  );
}