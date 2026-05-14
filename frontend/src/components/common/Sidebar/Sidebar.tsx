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
    <aside className="w-72 h-full bg-[#2A184F]/90 backdrop-blur-xl border-r border-white/10 p-6 flex flex-col">
      <div className="mb-10">
        <h1 className="text-4xl font-bold tracking-tight text-soft">
          Lumina
        </h1>

        <p className="text-muted text-sm mt-2">
          Wallpaper Manager
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
              className={`flex items-center gap-4 px-5 py-4 rounded-3xl transition-all duration-300 ${
                active
                  ? "bg-primary shadow-glow text-white"
                  : "hover:bg-white/10 text-text"
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
        <div className="bg-white/5 border border-white/10 rounded-[28px] p-5 backdrop-blur-xl">
          <div className="flex items-center justify-between">
            <span className="text-sm text-muted">
              GPU Rendering
            </span>

            <span className="text-accent font-semibold">
              ON
            </span>
          </div>

          <div className="mt-4 h-3 rounded-full bg-white/10 overflow-hidden">
            <div className="h-full w-[78%] bg-gradient-to-r from-primary to-accent rounded-full" />
          </div>
        </div>
      </div>
    </aside>
  );
}