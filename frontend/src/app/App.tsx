import { useState } from "react";
import {
  Info,
  Home as HomeIcon,
  Image,
  List,
  Activity,
  Settings as SettingsIcon,
  Search,
  Bell,
  User,
} from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import About from "./components/About";
import Home from "./components/Home";
import Library from "./components/Library";
import Playlist from "./components/Playlist";
import Performance from "./components/Performance";
import Settings from "./components/Settings";
import WallpaperModal from "./components/WallpaperModal";

type Screen =
  | "about"
  | "home"
  | "library"
  | "playlists"
  | "performance"
  | "settings";

const navItems = [
  { id: "about" as Screen, label: "About", icon: Info },
  { id: "home" as Screen, label: "Home", icon: HomeIcon },
  { id: "library" as Screen, label: "Library", icon: Image },
  { id: "playlists" as Screen, label: "Playlists", icon: List },
  {
    id: "performance" as Screen,
    label: "Performance",
    icon: Activity,
  },
  {
    id: "settings" as Screen,
    label: "Settings",
    icon: SettingsIcon,
  },
];

export default function App() {
  const [activeScreen, setActiveScreen] =
    useState<Screen>("home");
  const [modalOpen, setModalOpen] = useState(false);

  return (
    <div
      className="h-screen w-full flex overflow-hidden"
      style={{
        background: "var(--bg-main)",
        fontFamily: "Inter, system-ui, sans-serif",
      }}
    >
      <aside
        className="w-64 flex flex-col border-r backdrop-blur-xl"
        style={{
          background:
            "linear-gradient(180deg, rgba(76, 76, 156, 0.5), rgba(51, 32, 99, 0.8))",
          borderColor: "var(--border-color)",
        }}
      >
        <div className="p-6">
          <div className="flex items-center gap-3 mb-8">
            <div className="w-10 h-10 rounded-2xl bg-gradient-to-br from-[var(--accent-primary)] to-[var(--accent-secondary)] flex items-center justify-center">
              <Image className="w-6 h-6" />
            </div>
            <div>
              <div className="text-xl">Wallpaper</div>
              <div className="text-sm text-[var(--text-secondary)]">
                Manager
              </div>
            </div>
          </div>

          <nav className="space-y-2">
            {navItems.map((item) => (
              <button
                key={item.id}
                onClick={() => setActiveScreen(item.id)}
                className={`w-full flex items-center gap-3 px-4 py-3 rounded-2xl transition-all ${
                  activeScreen === item.id
                    ? "bg-gradient-to-r from-[var(--accent-primary)] to-[var(--accent-secondary)] shadow-lg shadow-purple-500/30"
                    : "hover:bg-white/5"
                }`}
              >
                <item.icon className="w-5 h-5" />
                <span>{item.label}</span>
              </button>
            ))}
          </nav>
        </div>

        <div className="mt-auto p-6 space-y-3">
          <div className="p-4 rounded-2xl bg-white/5 border border-[var(--border-color)]">
            <div className="text-sm text-[var(--text-secondary)] mb-1">
              Quick Stats
            </div>
            <div className="flex items-center justify-between text-sm">
              <span>GPU</span>
              <span className="text-green-400">12%</span>
            </div>
            <div className="flex items-center justify-between text-sm">
              <span>FPS</span>
              <span>60</span>
            </div>
          </div>

          <button className="w-full flex items-center gap-3 px-4 py-3 rounded-2xl hover:bg-white/5 transition-all">
            <div className="w-8 h-8 rounded-full bg-gradient-to-br from-purple-500 to-pink-500 flex items-center justify-center">
              <User className="w-4 h-4" />
            </div>
            <div className="flex-1 text-left">
              <div className="text-sm">John Doe</div>
              <div className="text-xs text-[var(--text-secondary)]">
                Premium
              </div>
            </div>
          </button>
        </div>
      </aside>

      <main className="flex-1 flex flex-col overflow-hidden">
        <header
          className="border-b backdrop-blur-xl px-8 py-4 flex items-center justify-between"
          style={{
            background:
              "linear-gradient(90deg, rgba(76, 76, 156, 0.3), rgba(51, 32, 99, 0.5))",
            borderColor: "var(--border-color)",
          }}
        >
          <div className="flex-1 max-w-xl">
            <div className="relative">
              <Search className="absolute left-4 top-1/2 -translate-y-1/2 w-5 h-5 text-[var(--text-secondary)]" />
              <input
                type="text"
                placeholder="Search wallpapers, playlists..."
                className="w-full pl-12 pr-4 py-3 rounded-2xl bg-white/5 border border-[var(--border-color)] backdrop-blur-sm focus:outline-none focus:ring-2 focus:ring-[var(--accent-primary)] transition-all"
              />
            </div>
          </div>

          <div className="flex items-center gap-3">
            <button className="w-10 h-10 rounded-full bg-white/5 hover:bg-white/10 border border-[var(--border-color)] transition-all flex items-center justify-center relative">
              <Bell className="w-5 h-5" />
              <div className="absolute top-1 right-1 w-2 h-2 bg-[var(--accent-primary)] rounded-full" />
            </button>
            <div className="px-4 py-2 rounded-full bg-white/5 border border-[var(--border-color)] text-sm">
              v2.4.0
            </div>
          </div>
        </header>

        <div className="flex-1 overflow-y-auto p-8">
          <AnimatePresence mode="wait">
            <motion.div
              key={activeScreen}
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -20 }}
              transition={{ duration: 0.3 }}
            >
              {activeScreen === "about" && <About />}
              {activeScreen === "home" && <Home />}
              {activeScreen === "library" && <Library />}
              {activeScreen === "playlists" && <Playlist />}
              {activeScreen === "performance" && (
                <Performance />
              )}
              {activeScreen === "settings" && <Settings />}
            </motion.div>
          </AnimatePresence>
        </div>
      </main>

      <WallpaperModal
        isOpen={modalOpen}
        onClose={() => setModalOpen(false)}
      />
    </div>
  );
}