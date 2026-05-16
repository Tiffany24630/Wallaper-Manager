import {
  LayoutDashboard,
  Image,
  Activity,
  Settings,
  MonitorPlay
} from "lucide-react";

const items = [
  {
    label: "Dashboard",
    icon: LayoutDashboard,
    active: true
  },
  {
    label: "Library",
    icon: Image
  },
  {
    label: "Playlists",
    icon: MonitorPlay
  },
  {
    label: "Performance",
    icon: Activity
  },
  {
    label: "Settings",
    icon: Settings
  }
];

export default function Sidebar() {
  return (
    <aside
      className="
        w-[300px]
        h-full
        p-6
        border-r
        border-white/10
        backdrop-blur-xl
      "
      style={{
        background:
          "linear-gradient(180deg, rgba(42,24,79,.95), rgba(20,15,45,.85))"
      }}
    >
      <div className="mb-10">
        <h1 className="text-5xl font-bold tracking-tight">
          Lumina
        </h1>

        <p className="text-[#D4A7F9] mt-2">
          Wallpaper Manager
        </p>
      </div>

      <nav className="space-y-4">
        {items.map((item) => {
          const Icon = item.icon;

          return (
            <button
              key={item.label}
              className={`
                w-full
                flex
                items-center
                gap-4
                px-5
                py-4
                rounded-3xl
                transition-all
                duration-300

                ${
                  item.active
                    ? "bg-[#7559CB] shadow-2xl"
                    : "bg-white/5 hover:bg-white/10"
                }
              `}
            >
              <Icon size={24} />

              <span className="text-lg">
                {item.label}
              </span>
            </button>
          );
        })}
      </nav>

      <div className="mt-10">
        <div
          className="
            rounded-[32px]
            p-6
            border
            border-white/10
          "
          style={{
            background:
              "linear-gradient(135deg, rgba(94,58,162,.45), rgba(117,89,203,.15))"
          }}
        >
          <div className="flex items-center justify-between">
            <span className="text-[#D4A7F9]">
              GPU Rendering
            </span>

            <span className="text-[#FBDBFB] font-semibold">
              Active
            </span>
          </div>

          <div className="mt-5 h-3 bg-white/10 rounded-full overflow-hidden">
            <div className="w-[78%] h-full bg-gradient-to-r from-[#9563DE] to-[#D4A7F9]" />
          </div>
        </div>
      </div>
    </aside>
  );
}