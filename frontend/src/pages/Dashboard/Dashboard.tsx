import { Activity, Image, List, Monitor, Zap, TrendingUp } from 'lucide-react';
import { motion } from 'motion/react';

const stats = [
  { label: 'Wallpapers', value: '1,247', icon: Image, color: 'from-purple-500 to-pink-500' },
  { label: 'Playlists', value: '24', icon: List, color: 'from-blue-500 to-cyan-500' },
  { label: 'Monitors', value: '3', icon: Monitor, color: 'from-green-500 to-emerald-500' },
  { label: 'GPU Usage', value: '12%', icon: Zap, color: 'from-orange-500 to-red-500' },
];

const recentWallpapers = [
  { id: 1, url: 'https://images.unsplash.com/photo-1705254613735-1abb457f8a60?w=400', title: 'Abstract Art' },
  { id: 2, url: 'https://images.unsplash.com/photo-1651136044251-bac298a4f1e7?w=400', title: 'Colorful Wires' },
  { id: 3, url: 'https://images.unsplash.com/photo-1656427833582-b276ee575f16?w=400', title: 'Pattern' },
  { id: 4, url: 'https://images.unsplash.com/photo-1641160858304-6aded85fa2c4?w=400', title: 'Pink Purple' },
  { id: 5, url: 'https://images.unsplash.com/photo-1656188505561-19f1a1b6cda8?w=400', title: 'Gradient' },
];

export default function Dashboard() {
  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-4xl mb-2">Welcome back</h1>
        <p className="text-[var(--text-secondary)]">Manage your desktop wallpapers with precision</p>
      </div>

      <div className="grid grid-cols-4 gap-6">
        {stats.map((stat, index) => (
          <motion.div
            key={stat.label}
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ delay: index * 0.1 }}
            className="relative overflow-hidden rounded-3xl p-6 backdrop-blur-xl"
            style={{
              background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
              border: '1px solid var(--border-color)',
            }}
          >
            <div className={`absolute inset-0 opacity-10 bg-gradient-to-br ${stat.color}`} />
            <div className="relative z-10">
              <stat.icon className="w-8 h-8 mb-4 text-[var(--highlight-soft)]" />
              <div className="text-3xl mb-1">{stat.value}</div>
              <div className="text-sm text-[var(--text-secondary)]">{stat.label}</div>
            </div>
          </motion.div>
        ))}
      </div>

      <div className="grid grid-cols-3 gap-6">
        <div className="col-span-2 rounded-3xl p-6 backdrop-blur-xl" style={{
          background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
          border: '1px solid var(--border-color)',
        }}>
          <h3 className="text-xl mb-4">Current Wallpaper</h3>
          <div className="relative aspect-video rounded-2xl overflow-hidden group">
            <img
              src="https://images.unsplash.com/photo-1656427833582-b276ee575f16?w=1200"
              alt="Current wallpaper"
              className="w-full h-full object-cover transition-transform duration-500 group-hover:scale-105"
            />
            <div className="absolute inset-0 bg-gradient-to-t from-black/50 to-transparent opacity-0 group-hover:opacity-100 transition-opacity duration-300" />
          </div>
          <div className="mt-4 flex items-center justify-between">
            <div>
              <div className="text-lg">Abstract Pattern</div>
              <div className="text-sm text-[var(--text-secondary)]">7680x4320 • 60 FPS • Video</div>
            </div>
            <button className="px-6 py-2 rounded-full bg-gradient-to-r from-[var(--accent-primary)] to-[var(--accent-secondary)] hover:shadow-lg hover:shadow-purple-500/50 transition-all">
              Change
            </button>
          </div>
        </div>

        <div className="space-y-6">
          <div className="rounded-3xl p-6 backdrop-blur-xl" style={{
            background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
            border: '1px solid var(--border-color)',
          }}>
            <div className="flex items-center gap-2 mb-4">
              <Activity className="w-5 h-5 text-[var(--accent-primary)]" />
              <h3>System Performance</h3>
            </div>
            <div className="space-y-4">
              <div>
                <div className="flex justify-between text-sm mb-2">
                  <span className="text-[var(--text-secondary)]">CPU</span>
                  <span>24%</span>
                </div>
                <div className="h-2 rounded-full bg-white/10 overflow-hidden">
                  <div className="h-full w-[24%] bg-gradient-to-r from-[var(--accent-primary)] to-[var(--accent-secondary)] rounded-full" />
                </div>
              </div>
              <div>
                <div className="flex justify-between text-sm mb-2">
                  <span className="text-[var(--text-secondary)]">GPU</span>
                  <span>12%</span>
                </div>
                <div className="h-2 rounded-full bg-white/10 overflow-hidden">
                  <div className="h-full w-[12%] bg-gradient-to-r from-blue-500 to-cyan-500 rounded-full" />
                </div>
              </div>
              <div>
                <div className="flex justify-between text-sm mb-2">
                  <span className="text-[var(--text-secondary)]">RAM</span>
                  <span>8.2GB / 32GB</span>
                </div>
                <div className="h-2 rounded-full bg-white/10 overflow-hidden">
                  <div className="h-full w-[25%] bg-gradient-to-r from-green-500 to-emerald-500 rounded-full" />
                </div>
              </div>
            </div>
          </div>

          <div className="rounded-3xl p-6 backdrop-blur-xl" style={{
            background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
            border: '1px solid var(--border-color)',
          }}>
            <div className="flex items-center gap-2 mb-4">
              <TrendingUp className="w-5 h-5 text-[var(--accent-primary)]" />
              <h3>Quick Stats</h3>
            </div>
            <div className="space-y-3">
              <div className="flex justify-between text-sm">
                <span className="text-[var(--text-secondary)]">FPS</span>
                <span>60</span>
              </div>
              <div className="flex justify-between text-sm">
                <span className="text-[var(--text-secondary)]">Resolution</span>
                <span>4K</span>
              </div>
              <div className="flex justify-between text-sm">
                <span className="text-[var(--text-secondary)]">Uptime</span>
                <span>3h 24m</span>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div className="rounded-3xl p-6 backdrop-blur-xl" style={{
        background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
        border: '1px solid var(--border-color)',
      }}>
        <h3 className="text-xl mb-4">Recently Used</h3>
        <div className="grid grid-cols-5 gap-4">
          {recentWallpapers.map((wallpaper, index) => (
            <motion.div
              key={wallpaper.id}
              initial={{ opacity: 0, scale: 0.9 }}
              animate={{ opacity: 1, scale: 1 }}
              transition={{ delay: index * 0.05 }}
              className="group cursor-pointer"
            >
              <div className="relative aspect-video rounded-2xl overflow-hidden">
                <img
                  src={wallpaper.url}
                  alt={wallpaper.title}
                  className="w-full h-full object-cover transition-transform duration-500 group-hover:scale-110"
                />
                <div className="absolute inset-0 bg-black/0 group-hover:bg-black/40 transition-colors duration-300" />
                <div className="absolute inset-0 flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity duration-300">
                  <div className="w-10 h-10 rounded-full bg-white/20 backdrop-blur-sm flex items-center justify-center">
                    <Activity className="w-5 h-5" />
                  </div>
                </div>
              </div>
              <div className="mt-2 text-sm text-center text-[var(--text-secondary)]">{wallpaper.title}</div>
            </motion.div>
          ))}
        </div>
      </div>
    </div>
  );
}
