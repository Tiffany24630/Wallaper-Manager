import { Monitor, Link2, Unlink2, Settings, Maximize2 } from 'lucide-react';
import { motion } from "framer-motion";
import { useState } from 'react';

const monitors = [
  { id: 1, name: 'Monitor 1', resolution: '3840x2160', refresh: '144Hz', wallpaper: 'https://images.unsplash.com/photo-1705254613735-1abb457f8a60?w=600', position: { x: 0, y: 100 }, width: 300, height: 169 },
  { id: 2, name: 'Monitor 2', resolution: '2560x1440', refresh: '165Hz', wallpaper: 'https://images.unsplash.com/photo-1651136044251-bac298a4f1e7?w=600', position: { x: 320, y: 50 }, width: 250, height: 141 },
  { id: 3, name: 'Monitor 3', resolution: '1920x1080', refresh: '60Hz', wallpaper: 'https://images.unsplash.com/photo-1656427833582-b276ee575f16?w=600', position: { x: 320, y: 210 }, width: 200, height: 113 },
];

const availableWallpapers = [
  { id: 1, url: 'https://images.unsplash.com/photo-1641160858304-6aded85fa2c4?w=400', title: 'Pink Purple' },
  { id: 2, url: 'https://images.unsplash.com/photo-1656188505561-19f1a1b6cda8?w=400', title: 'Gradient' },
  { id: 3, url: 'https://images.unsplash.com/photo-1641326038434-01b0217c18f1?w=400', title: 'Blue Pink' },
  { id: 4, url: 'https://images.unsplash.com/photo-1656427868828-79a829b92b2b?w=400', title: 'Pattern 2' },
];

export default function MultiMonitor() {
  const [synced, setSynced] = useState(false);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-4xl mb-2">Multi-Monitor Management</h1>
          <p className="text-[var(--text-secondary)]">Configure wallpapers for each display</p>
        </div>
        <button
          onClick={() => setSynced(!synced)}
          className={`px-6 py-3 rounded-full transition-all flex items-center gap-2 ${synced ? 'bg-gradient-to-r from-[var(--accent-primary)] to-[var(--accent-secondary)] shadow-lg shadow-purple-500/50' : 'bg-white/5 border border-[var(--border-color)]'}`}
        >
          {synced ? <Link2 className="w-5 h-5" /> : <Unlink2 className="w-5 h-5" />}
          {synced ? 'Synced' : 'Independent'}
        </button>
      </div>

      <div className="grid grid-cols-3 gap-6">
        <div className="col-span-2 rounded-3xl p-8 backdrop-blur-xl" style={{
          background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
          border: '1px solid var(--border-color)',
        }}>
          <h3 className="text-xl mb-6">Monitor Layout</h3>
          <div className="relative h-96 rounded-2xl bg-black/20 border border-[var(--border-color)] p-8">
            {monitors.map((monitor) => (
              <motion.div
                key={monitor.id}
                initial={{ opacity: 0, scale: 0.9 }}
                animate={{ opacity: 1, scale: 1 }}
                transition={{ delay: monitor.id * 0.1 }}
                className="absolute group cursor-pointer"
                style={{
                  left: monitor.position.x,
                  top: monitor.position.y,
                  width: monitor.width,
                  height: monitor.height,
                }}
              >
                <div className="relative w-full h-full rounded-xl overflow-hidden border-2 border-[var(--accent-primary)] shadow-lg shadow-purple-500/30">
                  <img
                    src={monitor.wallpaper}
                    alt={monitor.name}
                    className="w-full h-full object-cover"
                  />
                  <div className="absolute inset-0 bg-gradient-to-t from-black/60 to-transparent" />
                  <div className="absolute bottom-2 left-2 right-2">
                    <div className="text-sm mb-1">{monitor.name}</div>
                    <div className="text-xs text-[var(--text-secondary)]">{monitor.resolution}</div>
                  </div>
                  <div className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity">
                    <button className="w-8 h-8 rounded-lg bg-black/50 backdrop-blur-sm flex items-center justify-center hover:bg-black/70 transition-colors">
                      <Settings className="w-4 h-4" />
                    </button>
                  </div>
                </div>
                <div className="absolute -bottom-6 left-1/2 -translate-x-1/2 px-2 py-1 rounded-lg bg-[var(--surface-card)] text-xs whitespace-nowrap">
                  {monitor.refresh}
                </div>
              </motion.div>
            ))}
          </div>
          <div className="mt-6 flex gap-3">
            <button className="flex-1 py-3 rounded-xl bg-white/5 hover:bg-white/10 border border-[var(--border-color)] transition-all flex items-center justify-center gap-2">
              <Maximize2 className="w-4 h-4" />
              Auto-Arrange
            </button>
            <button className="flex-1 py-3 rounded-xl bg-white/5 hover:bg-white/10 border border-[var(--border-color)] transition-all">
              Reset Layout
            </button>
          </div>
        </div>

        <div className="space-y-6">
          <div className="rounded-3xl p-6 backdrop-blur-xl" style={{
            background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
            border: '1px solid var(--border-color)',
          }}>
            <h3 className="text-xl mb-4">Monitor Info</h3>
            <div className="space-y-3">
              {monitors.map((monitor) => (
                <div
                  key={monitor.id}
                  className="p-3 rounded-xl bg-white/5 border border-[var(--border-color)] cursor-pointer hover:bg-white/10 transition-all"
                >
                  <div className="flex items-center gap-2 mb-2">
                    <Monitor className="w-4 h-4 text-[var(--accent-primary)]" />
                    <span className="text-sm">{monitor.name}</span>
                  </div>
                  <div className="text-xs text-[var(--text-secondary)]">{monitor.resolution}</div>
                  <div className="text-xs text-[var(--text-secondary)]">{monitor.refresh}</div>
                </div>
              ))}
            </div>
          </div>

          <div className="rounded-3xl p-6 backdrop-blur-xl" style={{
            background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
            border: '1px solid var(--border-color)',
          }}>
            <h3 className="text-xl mb-4">GPU Rendering</h3>
            <div className="space-y-3">
              <div className="flex justify-between text-sm">
                <span className="text-[var(--text-secondary)]">Monitor 1</span>
                <span className="text-green-400">GPU 0</span>
              </div>
              <div className="flex justify-between text-sm">
                <span className="text-[var(--text-secondary)]">Monitor 2</span>
                <span className="text-green-400">GPU 0</span>
              </div>
              <div className="flex justify-between text-sm">
                <span className="text-[var(--text-secondary)]">Monitor 3</span>
                <span className="text-blue-400">GPU 1</span>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div className="rounded-3xl p-6 backdrop-blur-xl" style={{
        background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
        border: '1px solid var(--border-color)',
      }}>
        <h3 className="text-xl mb-4">Quick Assign Wallpapers</h3>
        <div className="grid grid-cols-4 gap-4">
          {availableWallpapers.map((wallpaper, index) => (
            <motion.div
              key={wallpaper.id}
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
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
                  <div className="text-xs">Drag to Monitor</div>
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
