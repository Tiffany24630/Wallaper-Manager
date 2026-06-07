import {
  Activity,
  Image,
  LayoutDashboard,
  Monitor,
  MonitorPlay,
  Settings,
  Sparkles,
} from "lucide-react";
import heroImage from "../../assets/hero.png";
import type { AppView } from "../App";
import type { BackendStatus } from "../../services/wallpaperService";

const items: Array<{
  id: AppView;
  label: string;
  icon: typeof LayoutDashboard;
}> = [
  { id: "dashboard", label: "Dashboard", icon: LayoutDashboard },
  { id: "library", label: "Library", icon: Image },
  { id: "playlists", label: "Playlists", icon: MonitorPlay },
  { id: "performance", label: "Performance", icon: Activity },
  { id: "settings", label: "Settings", icon: Settings },
];

interface SidebarProps {
  activeView: AppView;
  status: BackendStatus | null;
  onViewChange: (view: AppView) => void;
}

export default function Sidebar({ activeView, status, onViewChange }: SidebarProps) {
  return (
    <aside className="border-b border-white/10 bg-slate-950/80 p-4 backdrop-blur-2xl lg:flex lg:h-full lg:w-[264px] lg:flex-col lg:border-b-0 lg:border-r lg:p-5">
      <div className="flex items-center gap-3 lg:mb-7">
        <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-lg border border-cyan-300/20 bg-cyan-300/10">
          <img src={heroImage} alt="" className="h-8 w-8 object-contain" />
        </div>

        <div className="min-w-0">
          <h1 className="truncate text-xl font-semibold tracking-normal text-white">
            Lumina
          </h1>
          <p className="truncate text-xs text-slate-400">Wallpaper Manager</p>
        </div>
      </div>

      <nav className="mt-4 flex gap-2 overflow-x-auto pb-1 lg:mt-0 lg:flex-col lg:overflow-visible lg:pb-0">
        {items.map((item) => {
          const Icon = item.icon;
          const active = activeView === item.id;

          return (
            <button
              key={item.id}
              type="button"
              onClick={() => onViewChange(item.id)}
              className={`flex min-w-max items-center gap-3 rounded-lg border px-3 py-2.5 text-sm transition duration-200 lg:w-full ${
                active
                  ? "border-cyan-300/30 bg-cyan-300/15 text-cyan-50 shadow-glow"
                  : "border-transparent text-slate-400 hover:border-white/10 hover:bg-white/[0.06] hover:text-slate-100"
              }`}
            >
              <Icon size={19} />
              <span>{item.label}</span>
            </button>
          );
        })}
      </nav>

      <div className="mt-auto hidden space-y-3 lg:block">
        <div className="rounded-lg border border-white/10 bg-white/[0.04] p-4">
          <div className="flex items-center gap-3">
            <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-emerald-400/10 text-emerald-300">
              <Monitor size={18} />
            </div>
            <div>
              <p className="text-sm font-medium text-white">
                {status?.ready ? "Backend ready" : "Backend offline"}
              </p>
              <p className="text-xs text-slate-400">
                {status ? `${status.totalWallpapers} wallpapers indexed` : "Waiting for Tauri"}
              </p>
            </div>
          </div>
        </div>

        <div className="rounded-lg border border-white/10 bg-white/[0.04] p-4">
          <div className="flex items-center justify-between">
            <span className="flex items-center gap-2 text-sm text-slate-400">
              <Sparkles size={16} className="text-amber-300" />
              GPU Rendering
            </span>
            <span className="text-sm font-semibold text-emerald-300">Active</span>
          </div>

          <div className="mt-4 h-2 overflow-hidden rounded-full bg-slate-800">
            <div className="h-full w-[78%] rounded-full bg-gradient-to-r from-cyan-300 via-emerald-300 to-amber-300" />
          </div>
        </div>
      </div>
    </aside>
  );
}
