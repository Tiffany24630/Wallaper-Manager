import {Home, Image, Monitor, Settings, ListMusic, Activity,} from "lucide-react";
import { NavLink } from "react-router-dom";

const items = [
  {
    path: "/",
    label: "Dashboard",
    icon: Home,
  },
  {
    path: "/library",
    label: "Biblioteca",
    icon: Image,
  },
  {
    path: "/monitors",
    label: "Monitores",
    icon: Monitor,
  },
  {
    path: "/playlists",
    label: "Playlists",
    icon: ListMusic,
  },
  {
    path: "/performance",
    label: "Rendimiento",
    icon: Activity,
  },
  {
    path: "/settings",
    label: "Configuración",
    icon: Settings,
  },
];

export default function Sidebar() {
  return (
    <aside
      className="
        w-72
        bg-zinc-900
        border-r
        border-zinc-800
      "
    >
      <div className="p-6">
        <h1 className="text-xl font-bold">
          Wallpaper Manager
        </h1>
      </div>

      <nav className="px-3">
        {items.map((item) => {
          const Icon = item.icon;

          return (
            <NavLink
              key={item.path}
              to={item.path}
              className={({ isActive }) =>
                `
                flex
                items-center
                gap-3
                px-4
                py-3
                rounded-lg
                mb-2

                ${
                  isActive
                    ? "bg-cyan-600"
                    : "hover:bg-zinc-800"
                }
              `
              }
            >
              <Icon size={18} />
              {item.label}
            </NavLink>
          );
        })}
      </nav>
    </aside>
  );
}