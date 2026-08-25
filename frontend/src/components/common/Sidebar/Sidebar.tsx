import { Activity, Image, LayoutDashboard, ListVideo, Monitor, Settings, Sparkles } from "lucide-react";
import { NavLink } from "react-router-dom";

const items = [
  { path: "/", label: "Inicio", icon: LayoutDashboard, end: true },
  { path: "/library", label: "Biblioteca", icon: Image },
  { path: "/playlists", label: "Listas", icon: ListVideo },
  { path: "/monitors", label: "Pantallas", icon: Monitor },
  { path: "/enhance", label: "Mejorar", icon: Sparkles },
  { path: "/performance", label: "Rendimiento", icon: Activity },
  { path: "/settings", label: "Ajustes", icon: Settings },
];

export default function Sidebar() {
  return (
    <aside className="sticky top-0 z-20 flex h-screen w-[244px] shrink-0 flex-col border-r border-white/10 bg-[#0d0f18]/95 px-4 py-5 backdrop-blur-xl max-[980px]:fixed max-[980px]:bottom-0 max-[980px]:top-auto max-[980px]:h-[72px] max-[980px]:w-full max-[980px]:flex-row max-[980px]:items-center max-[980px]:overflow-x-auto max-[980px]:border-r-0 max-[980px]:border-t max-[980px]:px-3 max-[980px]:py-2">
      <div className="mb-8 flex items-center gap-3 px-2 max-[980px]:mb-0 max-[980px]:mr-3">
        <div className="grid h-10 w-10 place-items-center rounded-xl bg-gradient-to-br from-violet-500 to-cyan-400 shadow-lg shadow-violet-500/20">
          <Sparkles size={19} />
        </div>
        <div className="max-[980px]:hidden">
          <div className="font-bold tracking-tight">Lumina</div>
          <div className="text-xs text-slate-500">Wallpaper manager</div>
        </div>
      </div>

      <nav className="flex flex-1 flex-col gap-1.5 max-[980px]:flex-row">
        {items.map(({ path, label, icon: Icon, end }) => (
          <NavLink
            key={path}
            to={path}
            end={end}
            title={label}
            className={({ isActive }) => `flex min-h-11 items-center gap-3 rounded-xl px-3 text-sm transition-colors max-[980px]:min-w-12 max-[980px]:justify-center max-[980px]:px-3 ${
              isActive ? "bg-violet-500/18 text-violet-200 ring-1 ring-violet-400/20" : "text-slate-400 hover:bg-white/5 hover:text-white"
            }`}
          >
            <Icon size={18} />
            <span className="max-[980px]:hidden">{label}</span>
          </NavLink>
        ))}
      </nav>

      <div className="rounded-xl border border-white/10 bg-white/[.03] p-3 text-xs text-slate-500 max-[980px]:hidden">
        <div className="mb-1 font-semibold text-slate-300">Lumina 0.1</div>
        Tus imágenes permanecen en este dispositivo.
      </div>
    </aside>
  );
}
