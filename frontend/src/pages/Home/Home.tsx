import MainLayout from "../../layouts/MainLayout/MainLayout";

export default function HomePage() {
  return (
    <MainLayout>
      <div className="space-y-8">
        <div>
          <h1 className="text-5xl font-bold tracking-tight">
            Wallpaper Dashboard
          </h1>

          <p className="text-zinc-400 mt-3 text-lg">
            Manage animated wallpapers, playlists and monitor rendering.
          </p>
        </div>

        <div className="grid grid-cols-3 gap-6">
          <DashboardCard
            title="Wallpapers"
            value="24"
            subtitle="Images and videos"
          />

          <DashboardCard
            title="Playlists"
            value="5"
            subtitle="Carousel collections"
          />

          <DashboardCard
            title="Active Monitors"
            value="2"
            subtitle="GPU accelerated"
          />
        </div>

        <div className="grid grid-cols-2 gap-6">
          <div className="bg-zinc-800 rounded-3xl p-6 border border-zinc-700">
            <div className="flex items-center justify-between">
              <div>
                <h2 className="text-2xl font-semibold">
                  Active Wallpaper
                </h2>

                <p className="text-zinc-400 mt-2">
                  Cyberpunk City Loop
                </p>
              </div>

              <button className="bg-blue-600 hover:bg-blue-500 transition px-5 py-3 rounded-2xl">
                Change
              </button>
            </div>

            <div className="mt-6 aspect-video rounded-2xl bg-gradient-to-br from-blue-600 to-purple-700" />
          </div>

          <div className="bg-zinc-800 rounded-3xl p-6 border border-zinc-700">
            <h2 className="text-2xl font-semibold">
              Performance
            </h2>

            <div className="mt-6 space-y-5">
              <PerformanceBar label="CPU" value="24%" width="24%" />
              <PerformanceBar label="GPU" value="51%" width="51%" />
              <PerformanceBar label="RAM" value="37%" width="37%" />
            </div>
          </div>
        </div>
      </div>
    </MainLayout>
  );
}

function DashboardCard({
  title,
  value,
  subtitle
}: {
  title: string;
  value: string;
  subtitle: string;
}) {
  return (
    <div className="bg-zinc-800 rounded-3xl p-6 border border-zinc-700 hover:border-blue-500 transition-all">
      <p className="text-zinc-400 text-sm">
        {title}
      </p>

      <h2 className="text-5xl font-bold mt-4">
        {value}
      </h2>

      <p className="text-zinc-500 mt-4">
        {subtitle}
      </p>
    </div>
  );
}

function PerformanceBar({
  label,
  value,
  width
}: {
  label: string;
  value: string;
  width: string;
}) {
  return (
    <div>
      <div className="flex items-center justify-between mb-2">
        <span>{label}</span>

        <span className="text-zinc-400">
          {value}
        </span>
      </div>

      <div className="h-3 rounded-full bg-zinc-700 overflow-hidden">
        <div
          className="h-full bg-blue-500 rounded-full"
          style={{ width }}
        />
      </div>
    </div>
  );
}