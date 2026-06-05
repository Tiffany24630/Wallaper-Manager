import {
  Bell,
  Cpu,
  Download,
  HardDrive,
  Image,
  Layers,
  Monitor,
  Play,
  Plus,
  Search,
  Settings2,
  SlidersHorizontal,
  Sparkles,
  Star,
  Upload,
  Zap
} from "lucide-react";

const stats = [
  {
    title: "Wallpapers",
    value: "1,247",
    detail: "+32 this week",
    icon: Image,
    tone: "text-cyan-300",
    surface: "bg-cyan-300/10"
  },
  {
    title: "Playlists",
    value: "24",
    detail: "6 scheduled",
    icon: Layers,
    tone: "text-violet-300",
    surface: "bg-violet-300/10"
  },
  {
    title: "GPU Load",
    value: "12%",
    detail: "Hardware render",
    icon: Cpu,
    tone: "text-emerald-300",
    surface: "bg-emerald-300/10"
  },
  {
    title: "Frame Rate",
    value: "60",
    detail: "Stable FPS",
    icon: Zap,
    tone: "text-amber-300",
    surface: "bg-amber-300/10"
  }
];

const wallpapers = [
  {
    title: "Aurora Drift",
    resolution: "7680 x 4320",
    type: "Video",
    src: "https://images.unsplash.com/photo-1656427833582-b276ee575f16?w=900",
    accent: "from-cyan-400 to-blue-500",
    favorite: true
  },
  {
    title: "Prism Circuit",
    resolution: "6000 x 4000",
    type: "Image",
    src: "https://images.unsplash.com/photo-1651136044251-bac298a4f1e7?w=900",
    accent: "from-fuchsia-400 to-rose-500",
    favorite: false
  },
  {
    title: "Soft Gradient",
    resolution: "4800 x 4800",
    type: "Video",
    src: "https://images.unsplash.com/photo-1705254613735-1abb457f8a60?w=900",
    accent: "from-emerald-300 to-cyan-400",
    favorite: true
  }
];

const activity = [
  "Playlist Morning Focus started",
  "Aurora Drift applied to Display 1",
  "Cache optimized, 1.8 GB cleared"
];

export default function Dashboard() {
  return (
    <section className="mx-auto flex w-full max-w-[1540px] flex-col gap-6 px-4 py-5 sm:px-6 lg:px-8 lg:py-7">
      <header className="flex flex-col gap-4 border-b border-white/10 pb-5 xl:flex-row xl:items-center xl:justify-between">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2 text-sm text-slate-400">
            <span className="rounded-md border border-emerald-300/20 bg-emerald-300/10 px-2 py-1 text-emerald-200">
              Live
            </span>
            <span>3 displays synchronized</span>
          </div>

          <h1 className="mt-3 text-3xl font-semibold tracking-normal text-white sm:text-4xl">
            Dashboard
          </h1>

          <p className="mt-2 max-w-2xl text-sm leading-6 text-slate-400 sm:text-base">
            Manage active wallpapers, monitor rendering load, and keep playlists ready.
          </p>
        </div>

        <div className="flex w-full flex-col gap-3 sm:flex-row xl:w-auto">
          <label className="relative min-w-0 flex-1 xl:w-80 xl:flex-none">
            <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-500" />
            <input
              type="search"
              placeholder="Search wallpapers"
              className="h-11 w-full rounded-lg border border-white/10 bg-white/[0.05] pl-10 pr-3 text-sm text-white placeholder:text-slate-500"
            />
          </label>

          <div className="flex gap-2">
            <button
              className="flex h-11 w-11 items-center justify-center rounded-lg border border-white/10 bg-white/[0.05] text-slate-300 transition hover:bg-white/10 hover:text-white"
              title="Notifications"
              aria-label="Notifications"
            >
              <Bell size={18} />
            </button>

            <button
              className="flex h-11 w-11 items-center justify-center rounded-lg border border-white/10 bg-white/[0.05] text-slate-300 transition hover:bg-white/10 hover:text-white"
              title="Filters"
              aria-label="Filters"
            >
              <SlidersHorizontal size={18} />
            </button>

            <button className="inline-flex h-11 items-center gap-2 rounded-lg bg-cyan-300 px-4 text-sm font-semibold text-slate-950 transition hover:bg-cyan-200">
              <Upload size={18} />
              Upload
            </button>
          </div>
        </div>
      </header>

      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
        {stats.map((stat) => (
          <StatCard key={stat.title} {...stat} />
        ))}
      </div>

      <div className="grid gap-6 xl:grid-cols-[minmax(0,1.55fr)_minmax(320px,.85fr)]">
        <CurrentWallpaper />

        <div className="grid gap-6 md:grid-cols-2 xl:grid-cols-1">
          <PerformanceCard />
          <QueueCard />
        </div>
      </div>

      <div>
        <div className="mb-3 flex items-center justify-between gap-3">
          <h2 className="text-lg font-semibold text-white">
            Recently Used
          </h2>

          <button className="inline-flex items-center gap-2 rounded-lg border border-white/10 bg-white/[0.04] px-3 py-2 text-sm text-slate-300 transition hover:bg-white/10 hover:text-white">
            <Plus size={16} />
            New playlist
          </button>
        </div>

        <div className="grid gap-4 md:grid-cols-2 2xl:grid-cols-3">
          {wallpapers.map((wallpaper) => (
            <WallpaperCard key={wallpaper.title} wallpaper={wallpaper} />
          ))}
        </div>
      </div>
    </section>
  );
}

function StatCard({
  title,
  value,
  detail,
  icon: Icon,
  tone,
  surface
}: {
  title: string;
  value: string;
  detail: string;
  icon: typeof Image;
  tone: string;
  surface: string;
}) {
  return (
    <div className="rounded-lg border border-white/10 bg-white/[0.055] p-5 shadow-panel backdrop-blur-xl">
      <div className="flex items-center justify-between">
        <span className="text-sm text-slate-400">
          {title}
        </span>

        <div className={`flex h-9 w-9 items-center justify-center rounded-lg ${surface} ${tone}`}>
          <Icon size={18} />
        </div>
      </div>

      <div className="mt-5 text-3xl font-semibold tracking-normal text-white">
        {value}
      </div>

      <p className="mt-1 text-sm text-slate-500">
        {detail}
      </p>
    </div>
  );
}

function CurrentWallpaper() {
  return (
    <article className="overflow-hidden rounded-lg border border-white/10 bg-white/[0.055] shadow-panel backdrop-blur-xl">
      <div className="relative aspect-[16/9] min-h-[300px] overflow-hidden">
        <img
          src="https://images.unsplash.com/photo-1641326038434-01b0217c18f1?w=1400"
          alt="Current wallpaper"
          className="h-full w-full object-cover"
        />

        <div className="absolute inset-0 bg-gradient-to-t from-slate-950/80 via-slate-950/20 to-transparent" />

        <div className="absolute left-4 right-4 top-4 flex items-center justify-between gap-3">
          <span className="rounded-md border border-emerald-300/30 bg-emerald-400/15 px-2.5 py-1 text-xs font-medium text-emerald-100 backdrop-blur">
            Active wallpaper
          </span>

          <div className="flex gap-2">
            <button
              className="flex h-9 w-9 items-center justify-center rounded-lg bg-slate-950/50 text-white backdrop-blur transition hover:bg-slate-900"
              title="Play preview"
              aria-label="Play preview"
            >
              <Play size={17} />
            </button>

            <button
              className="flex h-9 w-9 items-center justify-center rounded-lg bg-slate-950/50 text-white backdrop-blur transition hover:bg-slate-900"
              title="Wallpaper settings"
              aria-label="Wallpaper settings"
            >
              <Settings2 size={17} />
            </button>
          </div>
        </div>

        <div className="absolute bottom-0 left-0 right-0 p-5">
          <div className="max-w-2xl">
            <p className="text-sm text-cyan-100">
              Display 1, Display 2, Display 3
            </p>

            <h2 className="mt-2 text-2xl font-semibold text-white sm:text-3xl">
              Neon Horizon
            </h2>

            <div className="mt-4 grid gap-3 text-sm text-slate-200 sm:grid-cols-3">
              <Metric icon={Monitor} label="Resolution" value="7680 x 4320" />
              <Metric icon={Zap} label="Frame rate" value="60 FPS" />
              <Metric icon={HardDrive} label="Memory" value="420 MB" />
            </div>
          </div>
        </div>
      </div>
    </article>
  );
}

function WallpaperCard({
  wallpaper
}: {
  wallpaper: (typeof wallpapers)[number];
}) {
  return (
    <article className="group overflow-hidden rounded-lg border border-white/10 bg-white/[0.05] transition hover:-translate-y-0.5 hover:border-white/20 hover:bg-white/[0.075]">
      <div className="relative aspect-video overflow-hidden">
        <img
          src={wallpaper.src}
          alt={wallpaper.title}
          className="h-full w-full object-cover transition duration-500 group-hover:scale-105"
        />

        <div className={`absolute inset-x-0 bottom-0 h-20 bg-gradient-to-t ${wallpaper.accent} opacity-55 mix-blend-screen`} />

        <button
          className="absolute right-3 top-3 flex h-8 w-8 items-center justify-center rounded-lg bg-slate-950/50 text-white backdrop-blur transition hover:bg-slate-900"
          title={wallpaper.favorite ? "Favorite" : "Add favorite"}
          aria-label={wallpaper.favorite ? "Favorite" : "Add favorite"}
        >
          <Star
            size={16}
            className={wallpaper.favorite ? "fill-amber-300 text-amber-300" : ""}
          />
        </button>
      </div>

      <div className="flex items-start justify-between gap-3 p-4">
        <div className="min-w-0">
          <h3 className="truncate text-base font-semibold text-white">
            {wallpaper.title}
          </h3>

          <p className="mt-1 text-sm text-slate-400">
            {wallpaper.resolution}
          </p>
        </div>

        <span className="rounded-md border border-white/10 bg-white/[0.06] px-2 py-1 text-xs text-slate-300">
          {wallpaper.type}
        </span>
      </div>
    </article>
  );
}

function PerformanceCard() {
  return (
    <section className="rounded-lg border border-white/10 bg-white/[0.055] p-5 shadow-panel backdrop-blur-xl">
      <div className="flex items-center justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold text-white">
            Performance
          </h2>

          <p className="mt-1 text-sm text-slate-400">
            Renderer load
          </p>
        </div>

        <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-emerald-300/10 text-emerald-300">
          <Sparkles size={18} />
        </div>
      </div>

      <div className="mt-6 space-y-5">
        <Bar label="CPU" width="24%" tone="bg-cyan-300" />
        <Bar label="GPU" width="12%" tone="bg-emerald-300" />
        <Bar label="RAM" width="25%" tone="bg-amber-300" />
      </div>
    </section>
  );
}

function QueueCard() {
  return (
    <section className="rounded-lg border border-white/10 bg-white/[0.055] p-5 shadow-panel backdrop-blur-xl">
      <div className="mb-4 flex items-center justify-between">
        <div>
          <h2 className="text-lg font-semibold text-white">
            Activity
          </h2>

          <p className="mt-1 text-sm text-slate-400">
            Latest events
          </p>
        </div>

        <button
          className="flex h-10 w-10 items-center justify-center rounded-lg border border-white/10 bg-white/[0.04] text-slate-300 transition hover:bg-white/10 hover:text-white"
          title="Download log"
          aria-label="Download log"
        >
          <Download size={17} />
        </button>
      </div>

      <div className="space-y-3">
        {activity.map((item, index) => (
          <div key={item} className="flex items-start gap-3">
            <span className="mt-1 flex h-6 w-6 shrink-0 items-center justify-center rounded-md bg-white/[0.06] text-xs text-cyan-200">
              {index + 1}
            </span>
            <p className="text-sm leading-6 text-slate-300">
              {item}
            </p>
          </div>
        ))}
      </div>
    </section>
  );
}

function Bar({
  label,
  width,
  tone
}: {
  label: string;
  width: string;
  tone: string;
}) {
  return (
    <div>
      <div className="mb-2 flex items-center justify-between text-sm">
        <span className="text-slate-400">{label}</span>

        <span className="text-slate-100">{width}</span>
      </div>

      <div className="h-2 overflow-hidden rounded-full bg-slate-800">
        <div
          className={`h-full rounded-full ${tone}`}
          style={{ width }}
        />
      </div>
    </div>
  );
}

function Metric({
  icon: Icon,
  label,
  value
}: {
  icon: typeof Monitor;
  label: string;
  value: string;
}) {
  return (
    <div className="flex items-center gap-2 rounded-lg bg-slate-950/40 px-3 py-2 backdrop-blur">
      <Icon size={16} className="shrink-0 text-cyan-200" />
      <span className="min-w-0">
        <span className="block text-xs text-slate-400">
          {label}
        </span>
        <span className="block truncate font-medium text-white">
          {value}
        </span>
      </span>
    </div>
  );
}
