import { Settings as SettingsIcon, Palette, Cpu, Volume2, Sparkles, Zap } from 'lucide-react';
import { motion } from 'framer-motion';
import { useState } from 'react';

export default function Settings() {
  const [activeTab, setActiveTab] = useState('general');

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-4xl mb-2">Settings</h1>
        <p className="text-[var(--text-secondary)]">Configure your wallpaper experience</p>
      </div>

      <div className="flex gap-2 p-1 rounded-2xl bg-white/5 border border-[var(--border-color)] w-fit">
        <button
          onClick={() => setActiveTab('general')}
          className={`px-6 py-2 rounded-xl transition-all flex items-center gap-2 ${activeTab === 'general' ? 'bg-[var(--accent-primary)] shadow-lg' : 'hover:bg-white/5'}`}
        >
          <SettingsIcon className="w-4 h-4" />
          General
        </button>
        <button
          onClick={() => setActiveTab('rendering')}
          className={`px-6 py-2 rounded-xl transition-all flex items-center gap-2 ${activeTab === 'rendering' ? 'bg-[var(--accent-primary)] shadow-lg' : 'hover:bg-white/5'}`}
        >
          <Cpu className="w-4 h-4" />
          Rendering
        </button>
        <button
          onClick={() => setActiveTab('wallpapers')}
          className={`px-6 py-2 rounded-xl transition-all flex items-center gap-2 ${activeTab === 'wallpapers' ? 'bg-[var(--accent-primary)] shadow-lg' : 'hover:bg-white/5'}`}
        >
          <Palette className="w-4 h-4" />
          Wallpapers
        </button>
        <button
          onClick={() => setActiveTab('audio')}
          className={`px-6 py-2 rounded-xl transition-all flex items-center gap-2 ${activeTab === 'audio' ? 'bg-[var(--accent-primary)] shadow-lg' : 'hover:bg-white/5'}`}
        >
          <Volume2 className="w-4 h-4" />
          Audio
        </button>
        <button
          onClick={() => setActiveTab('ai')}
          className={`px-6 py-2 rounded-xl transition-all flex items-center gap-2 ${activeTab === 'ai' ? 'bg-[var(--accent-primary)] shadow-lg' : 'hover:bg-white/5'}`}
        >
          <Sparkles className="w-4 h-4" />
          AI Enhancement
        </button>
        <button
          onClick={() => setActiveTab('performance')}
          className={`px-6 py-2 rounded-xl transition-all flex items-center gap-2 ${activeTab === 'performance' ? 'bg-[var(--accent-primary)] shadow-lg' : 'hover:bg-white/5'}`}
        >
          <Zap className="w-4 h-4" />
          Performance
        </button>
      </div>

      <motion.div
        key={activeTab}
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.3 }}
      >
        {activeTab === 'general' && (
          <div className="grid grid-cols-2 gap-6">
            <div className="rounded-3xl p-6 backdrop-blur-xl" style={{
              background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
              border: '1px solid var(--border-color)',
            }}>
              <h3 className="text-xl mb-4">Application</h3>
              <div className="space-y-4">
                <div className="flex items-center justify-between">
                  <span>Launch at Startup</span>
                  <div className="w-12 h-6 bg-[var(--accent-primary)] rounded-full p-1 cursor-pointer">
                    <div className="w-4 h-4 bg-white rounded-full ml-auto" />
                  </div>
                </div>
                <div className="flex items-center justify-between">
                  <span>Minimize to Tray</span>
                  <div className="w-12 h-6 bg-[var(--accent-primary)] rounded-full p-1 cursor-pointer">
                    <div className="w-4 h-4 bg-white rounded-full ml-auto" />
                  </div>
                </div>
                <div className="flex items-center justify-between">
                  <span>Check for Updates</span>
                  <div className="w-12 h-6 bg-white/20 rounded-full p-1 cursor-pointer">
                    <div className="w-4 h-4 bg-white rounded-full" />
                  </div>
                </div>
              </div>
            </div>

            <div className="rounded-3xl p-6 backdrop-blur-xl" style={{
              background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
              border: '1px solid var(--border-color)',
            }}>
              <h3 className="text-xl mb-4">Theme</h3>
              <div className="space-y-3">
                <button className="w-full py-3 rounded-xl bg-[var(--accent-primary)] text-left px-4">
                  Dark Mode
                </button>
                <button className="w-full py-3 rounded-xl bg-white/5 hover:bg-white/10 transition-all text-left px-4">
                  Light Mode
                </button>
                <button className="w-full py-3 rounded-xl bg-white/5 hover:bg-white/10 transition-all text-left px-4">
                  Auto (System)
                </button>
              </div>
            </div>
          </div>
        )}

        {activeTab === 'rendering' && (
          <div className="grid grid-cols-2 gap-6">
            <div className="rounded-3xl p-6 backdrop-blur-xl" style={{
              background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
              border: '1px solid var(--border-color)',
            }}>
              <h3 className="text-xl mb-4">Quality Settings</h3>
              <div className="space-y-4">
                <div>
                  <label className="block text-sm text-[var(--text-secondary)] mb-2">Video Quality</label>
                  <select className="w-full px-4 py-2 rounded-xl bg-white/5 border border-[var(--border-color)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-primary)]">
                    <option>Ultra (4K)</option>
                    <option>High (1440p)</option>
                    <option>Medium (1080p)</option>
                    <option>Low (720p)</option>
                  </select>
                </div>
                <div>
                  <label className="block text-sm text-[var(--text-secondary)] mb-2">FPS Limit</label>
                  <select className="w-full px-4 py-2 rounded-xl bg-white/5 border border-[var(--border-color)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-primary)]">
                    <option>Unlimited</option>
                    <option>144 FPS</option>
                    <option>120 FPS</option>
                    <option>60 FPS</option>
                    <option>30 FPS</option>
                  </select>
                </div>
              </div>
            </div>

            <div className="rounded-3xl p-6 backdrop-blur-xl" style={{
              background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
              border: '1px solid var(--border-color)',
            }}>
              <h3 className="text-xl mb-4">Hardware</h3>
              <div className="space-y-4">
                <div className="flex items-center justify-between">
                  <span>Hardware Acceleration</span>
                  <div className="w-12 h-6 bg-[var(--accent-primary)] rounded-full p-1 cursor-pointer">
                    <div className="w-4 h-4 bg-white rounded-full ml-auto" />
                  </div>
                </div>
                <div className="flex items-center justify-between">
                  <span>GPU Rendering</span>
                  <div className="w-12 h-6 bg-[var(--accent-primary)] rounded-full p-1 cursor-pointer">
                    <div className="w-4 h-4 bg-white rounded-full ml-auto" />
                  </div>
                </div>
                <div className="flex items-center justify-between">
                  <span>V-Sync</span>
                  <div className="w-12 h-6 bg-white/20 rounded-full p-1 cursor-pointer">
                    <div className="w-4 h-4 bg-white rounded-full" />
                  </div>
                </div>
              </div>
            </div>
          </div>
        )}

        {activeTab === 'wallpapers' && (
          <div className="grid grid-cols-2 gap-6">
            <div className="rounded-3xl p-6 backdrop-blur-xl" style={{
              background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
              border: '1px solid var(--border-color)',
            }}>
              <h3 className="text-xl mb-4">Display Options</h3>
              <div className="space-y-4">
                <div>
                  <label className="block text-sm text-[var(--text-secondary)] mb-2">Scaling Mode</label>
                  <select className="w-full px-4 py-2 rounded-xl bg-white/5 border border-[var(--border-color)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-primary)]">
                    <option>Fill</option>
                    <option>Fit</option>
                    <option>Stretch</option>
                    <option>Center</option>
                  </select>
                </div>
                <div className="flex items-center justify-between">
                  <span>Pause on Battery</span>
                  <div className="w-12 h-6 bg-[var(--accent-primary)] rounded-full p-1 cursor-pointer">
                    <div className="w-4 h-4 bg-white rounded-full ml-auto" />
                  </div>
                </div>
                <div className="flex items-center justify-between">
                  <span>Pause when Maximized</span>
                  <div className="w-12 h-6 bg-white/20 rounded-full p-1 cursor-pointer">
                    <div className="w-4 h-4 bg-white rounded-full" />
                  </div>
                </div>
              </div>
            </div>

            <div className="rounded-3xl p-6 backdrop-blur-xl" style={{
              background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
              border: '1px solid var(--border-color)',
            }}>
              <h3 className="text-xl mb-4">Library</h3>
              <div className="space-y-4">
                <div>
                  <label className="block text-sm text-[var(--text-secondary)] mb-2">Storage Location</label>
                  <div className="flex gap-2">
                    <input
                      type="text"
                      value="/Users/username/Wallpapers"
                      className="flex-1 px-4 py-2 rounded-xl bg-white/5 border border-[var(--border-color)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-primary)]"
                      readOnly
                    />
                    <button className="px-4 py-2 rounded-xl bg-[var(--accent-primary)] hover:shadow-lg transition-all">
                      Browse
                    </button>
                  </div>
                </div>
                <button className="w-full py-3 rounded-xl bg-white/5 hover:bg-white/10 border border-[var(--border-color)] transition-all">
                  Clear Cache
                </button>
              </div>
            </div>
          </div>
        )}

        {activeTab === 'audio' && (
          <div className="rounded-3xl p-6 backdrop-blur-xl max-w-2xl" style={{
            background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
            border: '1px solid var(--border-color)',
          }}>
            <h3 className="text-xl mb-4">Audio Settings</h3>
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <span>Enable Wallpaper Audio</span>
                <div className="w-12 h-6 bg-white/20 rounded-full p-1 cursor-pointer">
                  <div className="w-4 h-4 bg-white rounded-full" />
                </div>
              </div>
              <div>
                <label className="block text-sm text-[var(--text-secondary)] mb-2">Volume</label>
                <input
                  type="range"
                  min="0"
                  max="100"
                  defaultValue="50"
                  className="w-full"
                />
                <div className="text-sm text-center mt-1">50%</div>
              </div>
              <div className="flex items-center justify-between">
                <span>Mute when Idle</span>
                <div className="w-12 h-6 bg-[var(--accent-primary)] rounded-full p-1 cursor-pointer">
                  <div className="w-4 h-4 bg-white rounded-full ml-auto" />
                </div>
              </div>
            </div>
          </div>
        )}

        {activeTab === 'ai' && (
          <div className="grid grid-cols-2 gap-6">
            <div className="rounded-3xl p-6 backdrop-blur-xl" style={{
              background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
              border: '1px solid var(--border-color)',
            }}>
              <h3 className="text-xl mb-4">AI Upscaling</h3>
              <div className="space-y-4">
                <div className="flex items-center justify-between">
                  <span>Enable AI Enhancement</span>
                  <div className="w-12 h-6 bg-[var(--accent-primary)] rounded-full p-1 cursor-pointer">
                    <div className="w-4 h-4 bg-white rounded-full ml-auto" />
                  </div>
                </div>
                <div>
                  <label className="block text-sm text-[var(--text-secondary)] mb-2">Enhancement Level</label>
                  <select className="w-full px-4 py-2 rounded-xl bg-white/5 border border-[var(--border-color)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-primary)]">
                    <option>Maximum</option>
                    <option>High</option>
                    <option>Medium</option>
                    <option>Low</option>
                  </select>
                </div>
                <div className="flex items-center justify-between">
                  <span>Real-time Processing</span>
                  <div className="w-12 h-6 bg-white/20 rounded-full p-1 cursor-pointer">
                    <div className="w-4 h-4 bg-white rounded-full" />
                  </div>
                </div>
              </div>
            </div>

            <div className="rounded-3xl p-6 backdrop-blur-xl" style={{
              background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
              border: '1px solid var(--border-color)',
            }}>
              <h3 className="text-xl mb-4">Advanced AI</h3>
              <div className="space-y-4">
                <div className="flex items-center justify-between">
                  <span>Noise Reduction</span>
                  <div className="w-12 h-6 bg-[var(--accent-primary)] rounded-full p-1 cursor-pointer">
                    <div className="w-4 h-4 bg-white rounded-full ml-auto" />
                  </div>
                </div>
                <div className="flex items-center justify-between">
                  <span>Color Enhancement</span>
                  <div className="w-12 h-6 bg-[var(--accent-primary)] rounded-full p-1 cursor-pointer">
                    <div className="w-4 h-4 bg-white rounded-full ml-auto" />
                  </div>
                </div>
                <div className="flex items-center justify-between">
                  <span>Sharpening</span>
                  <div className="w-12 h-6 bg-white/20 rounded-full p-1 cursor-pointer">
                    <div className="w-4 h-4 bg-white rounded-full" />
                  </div>
                </div>
              </div>
            </div>
          </div>
        )}

        {activeTab === 'performance' && (
          <div className="rounded-3xl p-6 backdrop-blur-xl max-w-2xl" style={{
            background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
            border: '1px solid var(--border-color)',
          }}>
            <h3 className="text-xl mb-4">Multi-Monitor Configuration</h3>
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <span>Independent Wallpapers</span>
                <div className="w-12 h-6 bg-[var(--accent-primary)] rounded-full p-1 cursor-pointer">
                  <div className="w-4 h-4 bg-white rounded-full ml-auto" />
                </div>
              </div>
              <div className="flex items-center justify-between">
                <span>Sync Across Monitors</span>
                <div className="w-12 h-6 bg-white/20 rounded-full p-1 cursor-pointer">
                  <div className="w-4 h-4 bg-white rounded-full" />
                </div>
              </div>
              <div className="flex items-center justify-between">
                <span>Auto-detect Resolution</span>
                <div className="w-12 h-6 bg-[var(--accent-primary)] rounded-full p-1 cursor-pointer">
                  <div className="w-4 h-4 bg-white rounded-full ml-auto" />
                </div>
              </div>
              <div className="p-4 rounded-2xl bg-white/5 border border-[var(--border-color)] mt-4">
                <div className="text-sm text-[var(--text-secondary)] mb-2">Detected Monitors</div>
                <div className="space-y-2">
                  <div className="text-sm">Monitor 1: 3840x2160 @ 144Hz</div>
                  <div className="text-sm">Monitor 2: 2560x1440 @ 165Hz</div>
                  <div className="text-sm">Monitor 3: 1920x1080 @ 60Hz</div>
                </div>
              </div>
            </div>
          </div>
        )}
      </motion.div>
    </div>
  );
}
