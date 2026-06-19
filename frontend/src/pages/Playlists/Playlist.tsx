import { Plus, Play, Pause, Shuffle, Monitor, Clock, GripVertical } from 'lucide-react';
import { motion } from "framer-motion";
import { useState } from 'react';

const playlistItems = [
  { id: 1, url: 'https://images.unsplash.com/photo-1705254613735-1abb457f8a60?w=400', title: 'Abstract Art', duration: 30, monitor: 'All' },
  { id: 2, url: 'https://images.unsplash.com/photo-1651136044251-bac298a4f1e7?w=400', title: 'Colorful Wires', duration: 45, monitor: 'Monitor 1' },
  { id: 3, url: 'https://images.unsplash.com/photo-1656427833582-b276ee575f16?w=400', title: 'Pattern', duration: 60, monitor: 'Monitor 2' },
  { id: 4, url: 'https://images.unsplash.com/photo-1641160858304-6aded85fa2c4?w=400', title: 'Pink Purple', duration: 90, monitor: 'All' },
];

export default function Playlist() {
  const [isPlaying, setIsPlaying] = useState(false);
  const [shuffle, setShuffle] = useState(false);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-4xl mb-2">Playlist Manager</h1>
          <p className="text-[var(--text-secondary)]">Create dynamic wallpaper sequences</p>
        </div>
        <div className="flex gap-3">
          <button
            onClick={() => setShuffle(!shuffle)}
            className={`px-6 py-3 rounded-full transition-all flex items-center gap-2 ${shuffle ? 'bg-gradient-to-r from-[var(--accent-primary)] to-[var(--accent-secondary)] shadow-lg shadow-purple-500/50' : 'bg-white/5 border border-[var(--border-color)]'}`}
          >
            <Shuffle className="w-5 h-5" />
            Shuffle
          </button>
          <button
            onClick={() => setIsPlaying(!isPlaying)}
            className="px-6 py-3 rounded-full bg-gradient-to-r from-[var(--accent-primary)] to-[var(--accent-secondary)] hover:shadow-lg hover:shadow-purple-500/50 transition-all flex items-center gap-2"
          >
            {isPlaying ? <Pause className="w-5 h-5" /> : <Play className="w-5 h-5" />}
            {isPlaying ? 'Pause' : 'Play'}
          </button>
        </div>
      </div>

      <div className="grid grid-cols-3 gap-6">
        <div className="col-span-2 rounded-3xl p-6 backdrop-blur-xl" style={{
          background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
          border: '1px solid var(--border-color)',
        }}>
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-xl">Timeline</h3>
            <button className="px-4 py-2 rounded-full bg-white/10 hover:bg-white/20 transition-all flex items-center gap-2 text-sm">
              <Plus className="w-4 h-4" />
              Add Wallpaper
            </button>
          </div>

          <div className="space-y-3">
            {playlistItems.map((item, index) => (
              <motion.div
                key={item.id}
                initial={{ opacity: 0, x: -20 }}
                animate={{ opacity: 1, x: 0 }}
                transition={{ delay: index * 0.1 }}
                className="group flex items-center gap-4 p-4 rounded-2xl bg-white/5 hover:bg-white/10 border border-[var(--border-color)] transition-all cursor-move"
              >
                <GripVertical className="w-5 h-5 text-[var(--text-secondary)] opacity-0 group-hover:opacity-100 transition-opacity" />
                <div className="w-20 h-14 rounded-xl overflow-hidden">
                  <img src={item.url} alt={item.title} className="w-full h-full object-cover" />
                </div>
                <div className="flex-1">
                  <div className="mb-1">{item.title}</div>
                  <div className="text-sm text-[var(--text-secondary)]">Duration: {item.duration}s</div>
                </div>
                <div className="flex items-center gap-2">
                  <Monitor className="w-4 h-4 text-[var(--text-secondary)]" />
                  <span className="text-sm text-[var(--text-secondary)]">{item.monitor}</span>
                </div>
                <div className="flex items-center gap-2">
                  <input
                    type="number"
                    value={item.duration}
                    className="w-20 px-3 py-2 rounded-xl bg-white/5 border border-[var(--border-color)] text-center focus:outline-none focus:ring-2 focus:ring-[var(--accent-primary)]"
                  />
                  <Clock className="w-4 h-4 text-[var(--text-secondary)]" />
                </div>
              </motion.div>
            ))}
          </div>

          <div className="mt-6 p-4 rounded-2xl bg-white/5 border border-[var(--border-color)]">
            <div className="flex items-center justify-between text-sm">
              <span className="text-[var(--text-secondary)]">Total Duration</span>
              <span>3m 45s</span>
            </div>
          </div>
        </div>

        <div className="space-y-6">
          <div className="rounded-3xl p-6 backdrop-blur-xl" style={{
            background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
            border: '1px solid var(--border-color)',
          }}>
            <h3 className="text-xl mb-4">Transition Settings</h3>
            <div className="space-y-4">
              <div>
                <label className="block text-sm text-[var(--text-secondary)] mb-2">Transition Type</label>
                <select className="w-full px-4 py-2 rounded-xl bg-white/5 border border-[var(--border-color)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-primary)]">
                  <option>Fade</option>
                  <option>Slide</option>
                  <option>Zoom</option>
                  <option>None</option>
                </select>
              </div>
              <div>
                <label className="block text-sm text-[var(--text-secondary)] mb-2">Duration (ms)</label>
                <input
                  type="range"
                  min="0"
                  max="2000"
                  defaultValue="500"
                  className="w-full"
                />
                <div className="text-sm text-center mt-1">500ms</div>
              </div>
            </div>
          </div>

          <div className="rounded-3xl p-6 backdrop-blur-xl" style={{
            background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
            border: '1px solid var(--border-color)',
          }}>
            <h3 className="text-xl mb-4">Monitor Assignment</h3>
            <div className="space-y-3">
              <button className="w-full p-3 rounded-xl bg-white/5 border border-[var(--border-color)] hover:bg-white/10 transition-all text-left">
                <div className="flex items-center gap-2">
                  <Monitor className="w-4 h-4" />
                  <span>All Monitors</span>
                </div>
              </button>
              <button className="w-full p-3 rounded-xl bg-white/5 border border-[var(--border-color)] hover:bg-white/10 transition-all text-left">
                <div className="flex items-center gap-2">
                  <Monitor className="w-4 h-4" />
                  <span>Monitor 1</span>
                </div>
              </button>
              <button className="w-full p-3 rounded-xl bg-white/5 border border-[var(--border-color)] hover:bg-white/10 transition-all text-left">
                <div className="flex items-center gap-2">
                  <Monitor className="w-4 h-4" />
                  <span>Monitor 2</span>
                </div>
              </button>
            </div>
          </div>

          <div className="rounded-3xl p-6 backdrop-blur-xl" style={{
            background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
            border: '1px solid var(--border-color)',
          }}>
            <h3 className="text-xl mb-4">Playback Options</h3>
            <div className="space-y-3">
              <div className="flex items-center justify-between">
                <span className="text-sm">Loop Playlist</span>
                <div className="w-12 h-6 bg-[var(--accent-primary)] rounded-full p-1 cursor-pointer">
                  <div className="w-4 h-4 bg-white rounded-full ml-auto" />
                </div>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-sm">Random Order</span>
                <div className="w-12 h-6 bg-white/20 rounded-full p-1 cursor-pointer">
                  <div className="w-4 h-4 bg-white rounded-full" />
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
