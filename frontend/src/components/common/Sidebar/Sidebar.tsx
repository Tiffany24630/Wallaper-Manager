import { Link, useLocation } from "react-router-dom";

import {
  Home,
  Image,
  ListVideo,
  Settings,
  Activity
} from "lucide-react";

const links = [
  {
    to: "/",
    label: "Home",
    icon: Home
  },
  {
    to: "/library",
    label: "Library",
    icon: Image
  },
  {
    to: "/playlists",
    label: "Playlists",
    icon: ListVideo
  },
  {
    to: "/performance",
    label: "Performance",
    icon: Activity
  },
  {
    to: "/settings",
    label: "Settings",
    icon: Settings
  }
];

export default function Sidebar() {
  const location = useLocation();

  return (
    <aside className="w-72 h-full bg-zinc-950 border-r border-zinc-800 p-6 flex flex-col">
      <div className="mb-10">
        <h1 className="text-3xl font-bold tracking-tight">
          Wallpaper
        </h1>

        <p className="text-zinc-500 text-sm mt-1">
          Desktop Manager
        </p>
      </div>

      <nav className="flex flex-col gap-3">
        {links.map((link) => {
          const Icon = link.icon;

          const active = location.pathname === link.to;

          return (
            <Link
              key={link.to}
              to={link.to}
              className={`flex items-center gap-4 px-4 py-3 rounded-2xl transition-all duration-200 ${
                active
                  ? "bg-blue-600 text-white shadow-lg"
                  : "hover:bg-zinc-800 text-zinc-300"
              }`}
            >
              <Icon size={22} />

              <span className="font-medium">
                {link.label}
              </span>
            </Link>
          );
        })}
      </nav>

      <div className="mt-auto">
        <div className="bg-zinc-900 rounded-2xl p-4 border border-zinc-800">
          <p className="text-sm text-zinc-400">
            GPU Rendering Enabled
          </p>

          <div className="mt-3 h-2 rounded-full bg-zinc-700 overflow-hidden">
            <div className="w-4/5 h-full bg-blue-500 rounded-full" />
          </div>
        </div>
      </div>
    </aside>
  );
}