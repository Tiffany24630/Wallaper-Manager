import { X, Star, Plus, Play, TrendingUp, Download } from 'lucide-react';
import { motion } from 'framer-motion';

interface WallpaperModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export default function WallpaperModal({ isOpen, onClose }: WallpaperModalProps) {
  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-8">
      <div className="absolute inset-0 bg-black/60 backdrop-blur-md" onClick={onClose} />
      <motion.div
        initial={{ opacity: 0, scale: 0.9 }}
        animate={{ opacity: 1, scale: 1 }}
        exit={{ opacity: 0, scale: 0.9 }}
        className="relative z-10 w-full max-w-5xl rounded-3xl p-8 backdrop-blur-xl"
        style={{
          background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.9), rgba(117, 89, 203, 0.7))',
          border: '1px solid var(--border-color)',
          boxShadow: '0 25px 50px -12px rgba(0, 0, 0, 0.5)',
        }}
      >
        <button
          onClick={onClose}
          className="absolute top-6 right-6 w-10 h-10 rounded-full bg-white/10 hover:bg-white/20 transition-all flex items-center justify-center"
        >
          <X className="w-5 h-5" />
        </button>

        <div className="grid grid-cols-3 gap-8">
          <div className="col-span-2">
            <div className="relative aspect-video rounded-2xl overflow-hidden mb-6">
              <img
                src="https://images.unsplash.com/photo-1656427833582-b276ee575f16?w=1200"
                alt="Wallpaper preview"
                className="w-full h-full object-cover"
              />
              <div className="absolute top-4 right-4 px-3 py-1 rounded-lg bg-black/50 backdrop-blur-sm flex items-center gap-1 text-sm">
                <Play className="w-4 h-4" />
                Video
              </div>
            </div>

            <div className="flex gap-3">
              <button className="flex-1 py-3 rounded-full bg-gradient-to-r from-[var(--accent-primary)] to-[var(--accent-secondary)] hover:shadow-lg hover:shadow-purple-500/50 transition-all flex items-center justify-center gap-2">
                <Play className="w-5 h-5" />
                Apply Wallpaper
              </button>
              <button className="px-6 py-3 rounded-full bg-white/10 hover:bg-white/20 transition-all flex items-center gap-2">
                <Star className="w-5 h-5" />
              </button>
              <button className="px-6 py-3 rounded-full bg-white/10 hover:bg-white/20 transition-all flex items-center gap-2">
                <Download className="w-5 h-5" />
              </button>
            </div>
          </div>

          <div className="space-y-6">
            <div>
              <h2 className="text-3xl mb-2">Abstract Pattern</h2>
              <p className="text-[var(--text-secondary)]">High-quality animated wallpaper</p>
            </div>

            <div className="rounded-2xl p-4 bg-white/5 border border-[var(--border-color)]">
              <h3 className="mb-3">Metadata</h3>
              <div className="space-y-2 text-sm">
                <div className="flex justify-between">
                  <span className="text-[var(--text-secondary)]">Resolution</span>
                  <span>7680x4320</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-[var(--text-secondary)]">File Size</span>
                  <span>24.8 MB</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-[var(--text-secondary)]">Format</span>
                  <span>MP4</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-[var(--text-secondary)]">Duration</span>
                  <span>0:45</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-[var(--text-secondary)]">FPS</span>
                  <span>60</span>
                </div>
              </div>
            </div>

            <div className="rounded-2xl p-4 bg-white/5 border border-[var(--border-color)]">
              <div className="flex items-center gap-2 mb-3">
                <TrendingUp className="w-4 h-4 text-[var(--accent-primary)]" />
                <h3>Performance Impact</h3>
              </div>
              <div className="space-y-3">
                <div>
                  <div className="flex justify-between text-sm mb-2">
                    <span className="text-[var(--text-secondary)]">CPU Usage</span>
                    <span className="text-green-400">Low</span>
                  </div>
                  <div className="h-2 rounded-full bg-white/10 overflow-hidden">
                    <div className="h-full w-[25%] bg-green-500 rounded-full" />
                  </div>
                </div>
                <div>
                  <div className="flex justify-between text-sm mb-2">
                    <span className="text-[var(--text-secondary)]">GPU Usage</span>
                    <span className="text-blue-400">Medium</span>
                  </div>
                  <div className="h-2 rounded-full bg-white/10 overflow-hidden">
                    <div className="h-full w-[50%] bg-blue-500 rounded-full" />
                  </div>
                </div>
                <div>
                  <div className="flex justify-between text-sm mb-2">
                    <span className="text-[var(--text-secondary)]">RAM Usage</span>
                    <span className="text-green-400">Low</span>
                  </div>
                  <div className="h-2 rounded-full bg-white/10 overflow-hidden">
                    <div className="h-full w-[20%] bg-green-500 rounded-full" />
                  </div>
                </div>
              </div>
            </div>

            <button className="w-full py-3 rounded-full bg-white/10 hover:bg-white/20 border border-[var(--border-color)] transition-all flex items-center justify-center gap-2">
              <Plus className="w-5 h-5" />
              Add to Playlist
            </button>

            <div>
              <h3 className="mb-2">Tags</h3>
              <div className="flex flex-wrap gap-2">
                <span className="px-3 py-1 rounded-lg bg-white/10 text-sm">4K</span>
                <span className="px-3 py-1 rounded-lg bg-white/10 text-sm">Abstract</span>
                <span className="px-3 py-1 rounded-lg bg-white/10 text-sm">Pattern</span>
                <span className="px-3 py-1 rounded-lg bg-white/10 text-sm">Animated</span>
              </div>
            </div>
          </div>
        </div>
      </motion.div>
    </div>
  );
}
