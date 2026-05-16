import { Sparkles, Zap, Shield, Heart, Github, Twitter, Globe, Star } from 'lucide-react';
import { motion } from 'motion/react';

const features = [
  { icon: Zap, title: 'High Performance', description: 'GPU-accelerated rendering for smooth 4K/8K wallpapers at 144+ FPS' },
  { icon: Sparkles, title: 'AI Enhancement', description: 'Built-in AI upscaling and enhancement for all your wallpapers' },
  { icon: Shield, title: 'Low Resource Usage', description: 'Optimized engine that uses minimal CPU and RAM' },
  { icon: Heart, title: 'Made with Love', description: 'Crafted by enthusiasts for the desktop customization community' },
];

const team = [
  { name: 'Alex Chen', role: 'Lead Developer', avatar: 'https://images.unsplash.com/photo-1507003211169-0a1dd7228f2d?w=200' },
  { name: 'Sarah Kim', role: 'UI/UX Designer', avatar: 'https://images.unsplash.com/photo-1494790108377-be9c29b29330?w=200' },
  { name: 'Marcus Rodriguez', role: 'Graphics Engineer', avatar: 'https://images.unsplash.com/photo-1500648767791-00dcc994a43e?w=200' },
  { name: 'Emma Thompson', role: 'AI Specialist', avatar: 'https://images.unsplash.com/photo-1438761681033-6461ffad8d80?w=200' },
];

export default function About() {
  return (
    <div className="space-y-8">
      <div className="text-center max-w-3xl mx-auto">
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          className="inline-flex items-center gap-2 px-4 py-2 rounded-full bg-gradient-to-r from-[var(--accent-primary)] to-[var(--accent-secondary)] mb-6"
        >
          <Sparkles className="w-4 h-4" />
          <span className="text-sm">Version 2.4.0</span>
        </motion.div>
        <h1 className="text-5xl mb-4">Wallpaper Manager</h1>
        <p className="text-xl text-[var(--text-secondary)] mb-8">
          The most advanced desktop wallpaper management system. Transform your workspace with stunning animated wallpapers, AI enhancement, and unparalleled performance.
        </p>
        <div className="flex items-center justify-center gap-4">
          <button className="px-8 py-4 rounded-full bg-gradient-to-r from-[var(--accent-primary)] to-[var(--accent-secondary)] hover:shadow-lg hover:shadow-purple-500/50 transition-all">
            Get Premium
          </button>
          <button className="px-8 py-4 rounded-full bg-white/5 hover:bg-white/10 border border-[var(--border-color)] transition-all">
            Documentation
          </button>
        </div>
      </div>

      <div className="grid grid-cols-4 gap-6">
        {features.map((feature, index) => (
          <motion.div
            key={feature.title}
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ delay: index * 0.1 }}
            className="rounded-3xl p-6 backdrop-blur-xl"
            style={{
              background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
              border: '1px solid var(--border-color)',
            }}
          >
            <div className="w-12 h-12 rounded-2xl bg-gradient-to-br from-[var(--accent-primary)] to-[var(--accent-secondary)] flex items-center justify-center mb-4">
              <feature.icon className="w-6 h-6" />
            </div>
            <h3 className="text-xl mb-2">{feature.title}</h3>
            <p className="text-sm text-[var(--text-secondary)]">{feature.description}</p>
          </motion.div>
        ))}
      </div>

      <div className="rounded-3xl p-8 backdrop-blur-xl" style={{
        background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
        border: '1px solid var(--border-color)',
      }}>
        <h2 className="text-3xl mb-6 text-center">Meet the Team</h2>
        <div className="grid grid-cols-4 gap-6">
          {team.map((member, index) => (
            <motion.div
              key={member.name}
              initial={{ opacity: 0, scale: 0.9 }}
              animate={{ opacity: 1, scale: 1 }}
              transition={{ delay: index * 0.1 }}
              className="text-center group"
            >
              <div className="w-32 h-32 mx-auto mb-4 rounded-3xl overflow-hidden border-2 border-[var(--accent-primary)] group-hover:scale-105 transition-transform duration-300">
                <img
                  src={member.avatar}
                  alt={member.name}
                  className="w-full h-full object-cover"
                />
              </div>
              <div className="text-lg mb-1">{member.name}</div>
              <div className="text-sm text-[var(--text-secondary)]">{member.role}</div>
            </motion.div>
          ))}
        </div>
      </div>

      <div className="grid grid-cols-2 gap-6">
        <div className="rounded-3xl p-8 backdrop-blur-xl" style={{
          background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
          border: '1px solid var(--border-color)',
        }}>
          <h2 className="text-3xl mb-4">Our Mission</h2>
          <p className="text-[var(--text-secondary)] mb-4">
            We believe your desktop should be as unique and dynamic as you are. Wallpaper Manager was created to give users complete control over their visual workspace, combining cutting-edge technology with an intuitive interface.
          </p>
          <p className="text-[var(--text-secondary)]">
            From gamers to creative professionals, millions of users worldwide trust Wallpaper Manager to bring their screens to life with stunning visuals and buttery-smooth performance.
          </p>
        </div>

        <div className="rounded-3xl p-8 backdrop-blur-xl" style={{
          background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
          border: '1px solid var(--border-color)',
        }}>
          <h2 className="text-3xl mb-4">Statistics</h2>
          <div className="space-y-4">
            <div>
              <div className="flex justify-between items-center mb-2">
                <span className="text-[var(--text-secondary)]">Active Users</span>
                <span className="text-2xl">2.4M+</span>
              </div>
              <div className="h-2 rounded-full bg-white/10 overflow-hidden">
                <div className="h-full w-[85%] bg-gradient-to-r from-[var(--accent-primary)] to-[var(--accent-secondary)] rounded-full" />
              </div>
            </div>
            <div>
              <div className="flex justify-between items-center mb-2">
                <span className="text-[var(--text-secondary)]">Wallpapers Hosted</span>
                <span className="text-2xl">500K+</span>
              </div>
              <div className="h-2 rounded-full bg-white/10 overflow-hidden">
                <div className="h-full w-[70%] bg-gradient-to-r from-blue-500 to-cyan-500 rounded-full" />
              </div>
            </div>
            <div>
              <div className="flex justify-between items-center mb-2">
                <span className="text-[var(--text-secondary)]">GitHub Stars</span>
                <span className="text-2xl">48K+</span>
              </div>
              <div className="h-2 rounded-full bg-white/10 overflow-hidden">
                <div className="h-full w-[60%] bg-gradient-to-r from-green-500 to-emerald-500 rounded-full" />
              </div>
            </div>
          </div>
        </div>
      </div>

      <div className="rounded-3xl p-8 backdrop-blur-xl text-center" style={{
        background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
        border: '1px solid var(--border-color)',
      }}>
        <h2 className="text-3xl mb-4">Connect With Us</h2>
        <p className="text-[var(--text-secondary)] mb-6 max-w-2xl mx-auto">
          Join our community of wallpaper enthusiasts. Share your creations, get support, and stay updated with the latest features.
        </p>
        <div className="flex items-center justify-center gap-4">
          <button className="w-14 h-14 rounded-2xl bg-white/10 hover:bg-white/20 border border-[var(--border-color)] transition-all flex items-center justify-center group">
            <Github className="w-6 h-6 group-hover:scale-110 transition-transform" />
          </button>
          <button className="w-14 h-14 rounded-2xl bg-white/10 hover:bg-white/20 border border-[var(--border-color)] transition-all flex items-center justify-center group">
            <Twitter className="w-6 h-6 group-hover:scale-110 transition-transform" />
          </button>
          <button className="w-14 h-14 rounded-2xl bg-white/10 hover:bg-white/20 border border-[var(--border-color)] transition-all flex items-center justify-center group">
            <Globe className="w-6 h-6 group-hover:scale-110 transition-transform" />
          </button>
          <button className="w-14 h-14 rounded-2xl bg-white/10 hover:bg-white/20 border border-[var(--border-color)] transition-all flex items-center justify-center group">
            <Star className="w-6 h-6 group-hover:scale-110 transition-transform" />
          </button>
        </div>
      </div>

      <div className="rounded-3xl p-8 backdrop-blur-xl text-center" style={{
        background: 'linear-gradient(135deg, rgba(149, 99, 222, 0.2), rgba(153, 123, 248, 0.1))',
        border: '1px solid var(--border-color)',
      }}>
        <div className="text-sm text-[var(--text-secondary)] mb-2">© 2026 Wallpaper Manager</div>
        <div className="text-sm text-[var(--text-secondary)]">
          Made with <Heart className="w-4 h-4 inline text-red-400" /> for desktop enthusiasts worldwide
        </div>
      </div>
    </div>
  );
}
