import { Cpu, Activity, Zap, HardDrive, TrendingUp, Settings } from 'lucide-react';
import { LineChart, Line, AreaChart, Area, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer } from 'recharts';
import { motion } from 'framer-motion';

const cpuData = [
  { time: '0s', value: 20 },
  { time: '10s', value: 35 },
  { time: '20s', value: 28 },
  { time: '30s', value: 42 },
  { time: '40s', value: 38 },
  { time: '50s', value: 45 },
  { time: '60s', value: 32 },
];

const gpuData = [
  { time: '0s', value: 10 },
  { time: '10s', value: 15 },
  { time: '20s', value: 12 },
  { time: '30s', value: 18 },
  { time: '40s', value: 14 },
  { time: '50s', value: 20 },
  { time: '60s', value: 16 },
];

export default function Performance() {
  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-4xl mb-2">Performance Monitor</h1>
        <p className="text-[var(--text-secondary)]">Real-time system diagnostics and optimization</p>
      </div>

      <div className="grid grid-cols-4 gap-6">
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          className="rounded-3xl p-6 backdrop-blur-xl"
          style={{
            background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
            border: '1px solid var(--border-color)',
          }}
        >
          <Cpu className="w-8 h-8 mb-4 text-[var(--accent-primary)]" />
          <div className="text-3xl mb-1">24%</div>
          <div className="text-sm text-[var(--text-secondary)]">CPU Usage</div>
        </motion.div>

        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.1 }}
          className="rounded-3xl p-6 backdrop-blur-xl"
          style={{
            background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
            border: '1px solid var(--border-color)',
          }}
        >
          <Activity className="w-8 h-8 mb-4 text-blue-400" />
          <div className="text-3xl mb-1">12%</div>
          <div className="text-sm text-[var(--text-secondary)]">GPU Usage</div>
        </motion.div>

        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.2 }}
          className="rounded-3xl p-6 backdrop-blur-xl"
          style={{
            background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
            border: '1px solid var(--border-color)',
          }}
        >
          <HardDrive className="w-8 h-8 mb-4 text-green-400" />
          <div className="text-3xl mb-1">8.2GB</div>
          <div className="text-sm text-[var(--text-secondary)]">RAM Usage</div>
        </motion.div>

        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.3 }}
          className="rounded-3xl p-6 backdrop-blur-xl"
          style={{
            background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
            border: '1px solid var(--border-color)',
          }}
        >
          <TrendingUp className="w-8 h-8 mb-4 text-yellow-400" />
          <div className="text-3xl mb-1">60</div>
          <div className="text-sm text-[var(--text-secondary)]">FPS</div>
        </motion.div>
      </div>

      <div className="grid grid-cols-2 gap-6">
        <div className="rounded-3xl p-6 backdrop-blur-xl" style={{
          background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
          border: '1px solid var(--border-color)',
        }}>
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-xl">CPU Usage</h3>
            <div className="px-3 py-1 rounded-full bg-[var(--accent-primary)]/20 text-sm">24%</div>
          </div>
          <ResponsiveContainer width="100%" height={200}>
            <AreaChart data={cpuData}>
              <defs>
                <linearGradient id="cpuGradient" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor="#9563DE" stopOpacity={0.8}/>
                  <stop offset="95%" stopColor="#9563DE" stopOpacity={0}/>
                </linearGradient>
              </defs>
              <CartesianGrid strokeDasharray="3 3" stroke="rgba(255,255,255,0.1)" />
              <XAxis dataKey="time" stroke="#D9C8E8" />
              <YAxis stroke="#D9C8E8" />
              <Tooltip contentStyle={{ backgroundColor: '#5E3AA2', border: '1px solid rgba(255,255,255,0.1)', borderRadius: '12px' }} />
              <Area type="monotone" dataKey="value" stroke="#9563DE" fillOpacity={1} fill="url(#cpuGradient)" />
            </AreaChart>
          </ResponsiveContainer>
        </div>

        <div className="rounded-3xl p-6 backdrop-blur-xl" style={{
          background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
          border: '1px solid var(--border-color)',
        }}>
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-xl">GPU Usage</h3>
            <div className="px-3 py-1 rounded-full bg-blue-500/20 text-sm">12%</div>
          </div>
          <ResponsiveContainer width="100%" height={200}>
            <LineChart data={gpuData}>
              <CartesianGrid strokeDasharray="3 3" stroke="rgba(255,255,255,0.1)" />
              <XAxis dataKey="time" stroke="#D9C8E8" />
              <YAxis stroke="#D9C8E8" />
              <Tooltip contentStyle={{ backgroundColor: '#5E3AA2', border: '1px solid rgba(255,255,255,0.1)', borderRadius: '12px' }} />
              <Line type="monotone" dataKey="value" stroke="#60a5fa" strokeWidth={3} dot={{ fill: '#60a5fa', r: 4 }} />
            </LineChart>
          </ResponsiveContainer>
        </div>
      </div>

      <div className="grid grid-cols-3 gap-6">
        <div className="rounded-3xl p-6 backdrop-blur-xl" style={{
          background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
          border: '1px solid var(--border-color)',
        }}>
          <div className="flex items-center gap-2 mb-4">
            <Zap className="w-5 h-5 text-[var(--accent-primary)]" />
            <h3>FPS Limiter</h3>
          </div>
          <div className="space-y-4">
            <div className="flex gap-2">
              <button className="flex-1 py-2 rounded-xl bg-[var(--accent-primary)] text-sm">60 FPS</button>
              <button className="flex-1 py-2 rounded-xl bg-white/5 hover:bg-white/10 transition-all text-sm">120 FPS</button>
            </div>
            <div className="flex gap-2">
              <button className="flex-1 py-2 rounded-xl bg-white/5 hover:bg-white/10 transition-all text-sm">144 FPS</button>
              <button className="flex-1 py-2 rounded-xl bg-white/5 hover:bg-white/10 transition-all text-sm">Unlimited</button>
            </div>
          </div>
        </div>

        <div className="rounded-3xl p-6 backdrop-blur-xl" style={{
          background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
          border: '1px solid var(--border-color)',
        }}>
          <div className="flex items-center gap-2 mb-4">
            <Settings className="w-5 h-5 text-[var(--accent-primary)]" />
            <h3>Rendering Mode</h3>
          </div>
          <div className="space-y-2">
            <button className="w-full py-2 rounded-xl bg-[var(--accent-primary)] text-sm text-left px-4">
              Hardware Accelerated
            </button>
            <button className="w-full py-2 rounded-xl bg-white/5 hover:bg-white/10 transition-all text-sm text-left px-4">
              Software Rendering
            </button>
            <button className="w-full py-2 rounded-xl bg-white/5 hover:bg-white/10 transition-all text-sm text-left px-4">
              Auto (Recommended)
            </button>
          </div>
        </div>

        <div className="rounded-3xl p-6 backdrop-blur-xl" style={{
          background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
          border: '1px solid var(--border-color)',
        }}>
          <h3 className="mb-4">Power Settings</h3>
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <span className="text-sm">Hardware Acceleration</span>
              <div className="w-12 h-6 bg-[var(--accent-primary)] rounded-full p-1 cursor-pointer">
                <div className="w-4 h-4 bg-white rounded-full ml-auto" />
              </div>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-sm">Gaming Mode</span>
              <div className="w-12 h-6 bg-white/20 rounded-full p-1 cursor-pointer">
                <div className="w-4 h-4 bg-white rounded-full" />
              </div>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-sm">Power Saving</span>
              <div className="w-12 h-6 bg-white/20 rounded-full p-1 cursor-pointer">
                <div className="w-4 h-4 bg-white rounded-full" />
              </div>
            </div>
          </div>
        </div>
      </div>

      <div className="rounded-3xl p-6 backdrop-blur-xl" style={{
        background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
        border: '1px solid var(--border-color)',
      }}>
        <h3 className="text-xl mb-4">System Diagnostics</h3>
        <div className="grid grid-cols-4 gap-4">
          <div className="p-4 rounded-2xl bg-white/5">
            <div className="text-sm text-[var(--text-secondary)] mb-1">Renderer</div>
            <div>NVIDIA RTX 4090</div>
          </div>
          <div className="p-4 rounded-2xl bg-white/5">
            <div className="text-sm text-[var(--text-secondary)] mb-1">VRAM Usage</div>
            <div>2.4 GB / 24 GB</div>
          </div>
          <div className="p-4 rounded-2xl bg-white/5">
            <div className="text-sm text-[var(--text-secondary)] mb-1">Resolution</div>
            <div>7680x4320</div>
          </div>
          <div className="p-4 rounded-2xl bg-white/5">
            <div className="text-sm text-[var(--text-secondary)] mb-1">Refresh Rate</div>
            <div>144 Hz</div>
          </div>
        </div>
      </div>
    </div>
  );
}
