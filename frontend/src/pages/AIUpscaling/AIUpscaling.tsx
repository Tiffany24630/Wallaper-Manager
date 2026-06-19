import { Sparkles, Zap, Image as ImageIcon, ArrowRight, Settings } from 'lucide-react';
import { motion } from "framer-motion";
import { useState } from 'react';

export default function AIUpscaling() {
  const [processing, setProcessing] = useState(false);
  const [progress, setProgress] = useState(0);
  const [sharpness, setSharpness] = useState(75);
  const [noiseReduction, setNoiseReduction] = useState(60);
  const [enhancement, setEnhancement] = useState(80);

  const handleProcess = () => {
    setProcessing(true);
    setProgress(0);
    const interval = setInterval(() => {
      setProgress((prev) => {
        if (prev >= 100) {
          clearInterval(interval);
          setProcessing(false);
          return 100;
        }
        return prev + 2;
      });
    }, 100);
  };

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-4xl mb-2">AI Enhancement Studio</h1>
        <p className="text-[var(--text-secondary)]">Upscale and enhance your wallpapers with AI</p>
      </div>

      <div className="grid grid-cols-3 gap-6">
        <div className="col-span-2 rounded-3xl p-6 backdrop-blur-xl" style={{
          background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
          border: '1px solid var(--border-color)',
        }}>
          <div className="flex items-center justify-between mb-6">
            <h3 className="text-xl">Before & After Comparison</h3>
            <div className="flex gap-2">
              <button className="px-4 py-2 rounded-xl bg-[var(--accent-primary)] text-sm">Split View</button>
              <button className="px-4 py-2 rounded-xl bg-white/5 hover:bg-white/10 transition-all text-sm">Side by Side</button>
            </div>
          </div>

          <div className="relative aspect-video rounded-2xl overflow-hidden bg-black/20 border border-[var(--border-color)]">
            <div className="grid grid-cols-2 h-full">
              <div className="relative">
                <img
                  src="https://images.unsplash.com/photo-1705254613735-1abb457f8a60?w=800"
                  alt="Original"
                  className="w-full h-full object-cover opacity-70"
                />
                <div className="absolute top-4 left-4 px-3 py-1 rounded-lg bg-black/50 backdrop-blur-sm text-sm">
                  Original (1920x1080)
                </div>
              </div>
              <div className="relative">
                <img
                  src="https://images.unsplash.com/photo-1705254613735-1abb457f8a60?w=800"
                  alt="Enhanced"
                  className="w-full h-full object-cover"
                />
                <div className="absolute top-4 right-4 px-3 py-1 rounded-lg bg-gradient-to-r from-[var(--accent-primary)] to-[var(--accent-secondary)] backdrop-blur-sm text-sm flex items-center gap-1">
                  <Sparkles className="w-4 h-4" />
                  Enhanced (4K)
                </div>
              </div>
            </div>
            <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2">
              <div className="w-1 h-full bg-white/50" />
            </div>
          </div>

          {processing && (
            <motion.div
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              className="mt-6 p-4 rounded-2xl bg-white/5 border border-[var(--border-color)]"
            >
              <div className="flex items-center justify-between mb-2">
                <div className="flex items-center gap-2">
                  <Zap className="w-4 h-4 text-[var(--accent-primary)] animate-pulse" />
                  <span className="text-sm">Processing with AI...</span>
                </div>
                <span className="text-sm">{progress}%</span>
              </div>
              <div className="h-2 rounded-full bg-white/10 overflow-hidden">
                <motion.div
                  className="h-full bg-gradient-to-r from-[var(--accent-primary)] to-[var(--accent-secondary)] rounded-full"
                  style={{ width: `${progress}%` }}
                  transition={{ duration: 0.3 }}
                />
              </div>
            </motion.div>
          )}

          <div className="mt-6 flex gap-3">
            <button
              onClick={handleProcess}
              disabled={processing}
              className="flex-1 py-3 rounded-full bg-gradient-to-r from-[var(--accent-primary)] to-[var(--accent-secondary)] hover:shadow-lg hover:shadow-purple-500/50 transition-all flex items-center justify-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              <Sparkles className="w-5 h-5" />
              {processing ? 'Processing...' : 'Enhance with AI'}
            </button>
            <button className="px-6 py-3 rounded-full bg-white/5 hover:bg-white/10 border border-[var(--border-color)] transition-all">
              Export
            </button>
          </div>
        </div>

        <div className="space-y-6">
          <div className="rounded-3xl p-6 backdrop-blur-xl" style={{
            background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
            border: '1px solid var(--border-color)',
          }}>
            <h3 className="text-xl mb-4">Enhancement Controls</h3>
            <div className="space-y-4">
              <div>
                <div className="flex justify-between text-sm mb-2">
                  <span className="text-[var(--text-secondary)]">Sharpness</span>
                  <span>{sharpness}%</span>
                </div>
                <input
                  type="range"
                  min="0"
                  max="100"
                  value={sharpness}
                  onChange={(e) => setSharpness(Number(e.target.value))}
                  className="w-full"
                />
              </div>
              <div>
                <div className="flex justify-between text-sm mb-2">
                  <span className="text-[var(--text-secondary)]">Noise Reduction</span>
                  <span>{noiseReduction}%</span>
                </div>
                <input
                  type="range"
                  min="0"
                  max="100"
                  value={noiseReduction}
                  onChange={(e) => setNoiseReduction(Number(e.target.value))}
                  className="w-full"
                />
              </div>
              <div>
                <div className="flex justify-between text-sm mb-2">
                  <span className="text-[var(--text-secondary)]">Enhancement Strength</span>
                  <span>{enhancement}%</span>
                </div>
                <input
                  type="range"
                  min="0"
                  max="100"
                  value={enhancement}
                  onChange={(e) => setEnhancement(Number(e.target.value))}
                  className="w-full"
                />
              </div>
            </div>
          </div>

          <div className="rounded-3xl p-6 backdrop-blur-xl" style={{
            background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
            border: '1px solid var(--border-color)',
          }}>
            <h3 className="text-xl mb-4">Output Settings</h3>
            <div className="space-y-3">
              <div>
                <label className="block text-sm text-[var(--text-secondary)] mb-2">Target Resolution</label>
                <select className="w-full px-4 py-2 rounded-xl bg-white/5 border border-[var(--border-color)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-primary)]">
                  <option>8K (7680x4320)</option>
                  <option>4K (3840x2160)</option>
                  <option>2K (2560x1440)</option>
                  <option>1080p (1920x1080)</option>
                </select>
              </div>
              <div>
                <label className="block text-sm text-[var(--text-secondary)] mb-2">AI Model</label>
                <select className="w-full px-4 py-2 rounded-xl bg-white/5 border border-[var(--border-color)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-primary)]">
                  <option>Ultra Quality (Slow)</option>
                  <option>Balanced (Recommended)</option>
                  <option>Fast (Lower Quality)</option>
                </select>
              </div>
            </div>
          </div>

          <div className="rounded-3xl p-6 backdrop-blur-xl" style={{
            background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
            border: '1px solid var(--border-color)',
          }}>
            <h3 className="text-xl mb-4">Advanced</h3>
            <div className="space-y-3">
              <div className="flex items-center justify-between">
                <span className="text-sm">Color Enhancement</span>
                <div className="w-12 h-6 bg-[var(--accent-primary)] rounded-full p-1 cursor-pointer">
                  <div className="w-4 h-4 bg-white rounded-full ml-auto" />
                </div>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-sm">Detail Preservation</span>
                <div className="w-12 h-6 bg-[var(--accent-primary)] rounded-full p-1 cursor-pointer">
                  <div className="w-4 h-4 bg-white rounded-full ml-auto" />
                </div>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-sm">Auto-Adjust</span>
                <div className="w-12 h-6 bg-white/20 rounded-full p-1 cursor-pointer">
                  <div className="w-4 h-4 bg-white rounded-full" />
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div className="rounded-3xl p-6 backdrop-blur-xl" style={{
        background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
        border: '1px solid var(--border-color)',
      }}>
        <h3 className="text-xl mb-4">Performance Impact</h3>
        <div className="grid grid-cols-4 gap-4">
          <div className="p-4 rounded-2xl bg-white/5">
            <div className="flex items-center gap-2 mb-2">
              <Zap className="w-4 h-4 text-yellow-400" />
              <span className="text-sm text-[var(--text-secondary)]">Processing Time</span>
            </div>
            <div className="text-2xl">~45s</div>
          </div>
          <div className="p-4 rounded-2xl bg-white/5">
            <div className="flex items-center gap-2 mb-2">
              <ImageIcon className="w-4 h-4 text-blue-400" />
              <span className="text-sm text-[var(--text-secondary)]">Output Size</span>
            </div>
            <div className="text-2xl">24 MB</div>
          </div>
          <div className="p-4 rounded-2xl bg-white/5">
            <div className="flex items-center gap-2 mb-2">
              <Settings className="w-4 h-4 text-green-400" />
              <span className="text-sm text-[var(--text-secondary)]">GPU Usage</span>
            </div>
            <div className="text-2xl">85%</div>
          </div>
          <div className="p-4 rounded-2xl bg-white/5">
            <div className="flex items-center gap-2 mb-2">
              <ArrowRight className="w-4 h-4 text-purple-400" />
              <span className="text-sm text-[var(--text-secondary)]">Quality Gain</span>
            </div>
            <div className="text-2xl">4x</div>
          </div>
        </div>
      </div>
    </div>
  );
}
