import {
  Cpu,
  Image,
  Layers,
  Activity
} from "lucide-react";

export default function Dashboard() {
  return (
    <div className="max-w-[1700px] mx-auto">
      <div className="flex items-center justify-between mb-10">
        <div>
          <h1 className="text-7xl font-bold leading-none">
            Dashboard
          </h1>

          <p className="mt-4 text-xl text-[#D4A7F9]">
            Premium animated wallpaper manager
          </p>
        </div>

        <button
          className="
            px-8
            py-5
            rounded-3xl
            bg-[#7559CB]
            hover:scale-105
            transition-all
            shadow-2xl
          "
        >
          Upload Wallpaper
        </button>
      </div>

      <div className="grid grid-cols-4 gap-6 mb-6">
        <StatCard
          title="Wallpapers"
          value="248"
          icon={<Image />}
        />

        <StatCard
          title="GPU Usage"
          value="24%"
          icon={<Cpu />}
        />

        <StatCard
          title="Playlists"
          value="18"
          icon={<Layers />}
        />

        <StatCard
          title="FPS"
          value="60"
          icon={<Activity />}
        />
      </div>

      <div className="grid grid-cols-3 gap-6 mb-6">
        <WallpaperCard />
        <WallpaperCard />
        <WallpaperCard />
      </div>

      <div className="grid grid-cols-2 gap-6">
        <PerformanceCard />
        <PreviewCard />
      </div>
    </div>
  );
}

function StatCard({
  title,
  value,
  icon
}: any) {
  return (
    <div
      className="
        rounded-[36px]
        p-7
        border
        border-white/10
        backdrop-blur-xl
      "
      style={{
        background:
          "linear-gradient(135deg, rgba(94,58,162,.45), rgba(117,89,203,.15))"
      }}
    >
      <div className="flex items-center justify-between">
        <span className="text-[#D4A7F9]">
          {title}
        </span>

        <div className="text-[#9563DE]">
          {icon}
        </div>
      </div>

      <h2 className="text-6xl font-bold mt-8">
        {value}
      </h2>
    </div>
  );
}

function WallpaperCard() {
  return (
    <div
      className="
        rounded-[36px]
        overflow-hidden
        border
        border-white/10
        backdrop-blur-xl
      "
      style={{
        background:
          "linear-gradient(135deg, rgba(94,58,162,.45), rgba(117,89,203,.15))"
      }}
    >
      <div className="aspect-video bg-gradient-to-br from-[#7559CB] via-[#9563DE] to-[#D4A7F9]" />

      <div className="p-6">
        <h3 className="text-2xl font-semibold">
          Cinematic Wallpaper
        </h3>

        <p className="mt-3 text-[#D4A7F9]">
          GPU accelerated animated background
        </p>
      </div>
    </div>
  );
}

function PerformanceCard() {
  return (
    <div
      className="
        rounded-[36px]
        p-8
        border
        border-white/10
      "
      style={{
        background:
          "linear-gradient(135deg, rgba(94,58,162,.45), rgba(117,89,203,.15))"
      }}
    >
      <h2 className="text-3xl font-bold">
        Performance
      </h2>

      <div className="space-y-7 mt-8">
        <Bar label="CPU" width="35%" />
        <Bar label="GPU" width="62%" />
        <Bar label="RAM" width="48%" />
      </div>
    </div>
  );
}

function PreviewCard() {
  return (
    <div
      className="
        rounded-[36px]
        p-8
        border
        border-white/10
      "
      style={{
        background:
          "linear-gradient(135deg, rgba(94,58,162,.45), rgba(117,89,203,.15))"
      }}
    >
      <h2 className="text-3xl font-bold">
        Current Wallpaper
      </h2>

      <div className="mt-6 rounded-[30px] aspect-video bg-gradient-to-br from-pink-500 via-purple-500 to-blue-500 shadow-2xl" />
    </div>
  );
}

function Bar({
  label,
  width
}: {
  label: string;
  width: string;
}) {
  return (
    <div>
      <div className="flex items-center justify-between mb-3">
        <span>{label}</span>

        <span>{width}</span>
      </div>

      <div className="h-4 rounded-full bg-white/10 overflow-hidden">
        <div
          className="h-full rounded-full bg-gradient-to-r from-[#9563DE] to-[#D4A7F9]"
          style={{ width }}
        />
      </div>
    </div>
  );
}