import MainLayout from "../../layouts/MainLayout/MainLayout";

export default function HomePage() {
  return (
    <MainLayout>
      <div className="space-y-8">
        <div>
          <h1 className="text-6xl font-bold tracking-tight text-soft">
            Lumina Dashboard
          </h1>

          <p className="text-muted mt-4 text-lg">
            Animated wallpapers, playlists and real-time rendering.
          </p>
        </div>

        <div className="grid grid-cols-3 gap-6">
          <DashboardCard
            title="Wallpapers"
            value="24"
            subtitle="Animated & Static"
          />

          <DashboardCard
            title="Playlists"
            value="5"
            subtitle="Carousel Collections"
          />

          <DashboardCard
            title="Monitors"
            value="2"
            subtitle="GPU Accelerated"
          />
        </div>

        <div className="grid grid-cols-2 gap-6">
          <div className="rounded-[32px] p-6 bg-white/5 border border-white/10 backdrop-blur-xl">
            <div className="flex items-center justify-between">
              <div>
                <h2 className="text-3xl font-semibold">
                  Current Wallpaper
                </h2>

                <p className="text-muted mt-2">
                  Purple Sakura Lights
                </p>
              </div>

              <button className="px-6 py-3 rounded-2xl bg-primary hover:bg-secondary transition-all shadow-glow">
                Change
              </button>
            </div>

            <div className="mt-6 rounded-[28px] overflow-hidden aspect-video bg-gradient-to-br from-primary via-secondary to-accent shadow-glow" />
          </div>

          <div className="rounded-[32px] p-6 bg-white/5 border border-white/10 backdrop-blur-xl">
            <h2 className="text-3xl font-semibold">
              Performance
            </h2>

            <div className="mt-8 space-y-6">
              <PerformanceBar
                label="CPU"
                value="24%"
                width="24%"
              />

              <PerformanceBar
                label="GPU"
                value="58%"
                width="58%"
              />

              <PerformanceBar
                label="RAM"
                value="37%"
                width="37%"
              />
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
    <div className="rounded-[32px] p-6 bg-white/5 border border-white/10 backdrop-blur-xl hover:scale-[1.02] hover:border-soft/40 transition-all duration-300">
      <p className="text-muted">
        {title}
      </p>

      <h2 className="text-6xl font-bold mt-5 text-soft">
        {value}
      </h2>

      <p className="mt-5 text-muted">
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
      <div className="flex items-center justify-between mb-3">
        <span className="text-lg">
          {label}
        </span>

        <span className="text-muted">
          {value}
        </span>
      </div>

      <div className="h-4 rounded-full bg-white/10 overflow-hidden">
        <div
          className="h-full rounded-full bg-gradient-to-r from-primary via-secondary to-accent shadow-glow"
          style={{ width }}
        />
      </div>
    </div>
  );
}