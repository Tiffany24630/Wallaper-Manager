import { Search, Upload, Filter, Star, Play, Image as ImageIcon } from 'lucide-react';
import { motion } from 'motion/react';
import { useState } from 'react';

const wallpapers = [
  { id: 1, url: 'https://images.unsplash.com/photo-1705254613735-1abb457f8a60?w=600', title: 'Abstract Art', resolution: '4800x4800', type: 'image', tags: ['abstract', 'colorful'], favorite: true },
  { id: 2, url: 'https://images.unsplash.com/photo-1651136044251-bac298a4f1e7?w=600', title: 'Colorful Wires', resolution: '6000x4000', type: 'image', tags: ['colorful', 'abstract'], favorite: false },
  { id: 3, url: 'https://images.unsplash.com/photo-1656427833582-b276ee575f16?w=600', title: 'Pattern', resolution: '7680x4320', type: 'video', duration: '0:45', tags: ['pattern', '4k'], favorite: true },
  { id: 4, url: 'https://images.unsplash.com/photo-1641160858304-6aded85fa2c4?w=600', title: 'Pink Purple', resolution: '7680x4320', type: 'video', duration: '1:20', tags: ['gradient', 'purple'], favorite: false },
  { id: 5, url: 'https://images.unsplash.com/photo-1656188505561-19f1a1b6cda8?w=600', title: 'Gradient', resolution: '7680x4320', type: 'image', tags: ['gradient', '4k'], favorite: false },
  { id: 6, url: 'https://images.unsplash.com/photo-1641326038434-01b0217c18f1?w=600', title: 'Blue Pink', resolution: '7680x4320', type: 'video', duration: '2:10', tags: ['gradient', 'blue'], favorite: true },
  { id: 7, url: 'https://images.unsplash.com/photo-1656427868828-79a829b92b2b?w=600', title: 'Pattern 2', resolution: '7680x4320', type: 'image', tags: ['pattern', 'abstract'], favorite: false },
  { id: 8, url: 'https://images.unsplash.com/photo-1640735853641-5d799222afbe?w=600', title: 'Multicolor', resolution: '7680x4320', type: 'video', duration: '1:05', tags: ['colorful', '4k'], favorite: true },
];

export default function Library() {
  const [filter, setFilter] = useState('all');

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-4xl mb-2">Wallpaper Library</h1>
          <p className="text-[var(--text-secondary)]">Browse and manage your collection</p>
        </div>
        <button className="px-6 py-3 rounded-full bg-gradient-to-r from-[var(--accent-primary)] to-[var(--accent-secondary)] hover:shadow-lg hover:shadow-purple-500/50 transition-all flex items-center gap-2">
          <Upload className="w-5 h-5" />
          Upload Wallpaper
        </button>
      </div>

      <div className="flex gap-4 items-center">
        <div className="flex-1 relative">
          <Search className="absolute left-4 top-1/2 -translate-y-1/2 w-5 h-5 text-[var(--text-secondary)]" />
          <input
            type="text"
            placeholder="Search wallpapers..."
            className="w-full pl-12 pr-4 py-3 rounded-2xl bg-white/5 border border-[var(--border-color)] backdrop-blur-sm focus:outline-none focus:ring-2 focus:ring-[var(--accent-primary)] transition-all"
          />
        </div>
        <div className="flex gap-2 p-1 rounded-2xl bg-white/5 border border-[var(--border-color)]">
          <button
            onClick={() => setFilter('all')}
            className={`px-6 py-2 rounded-xl transition-all ${filter === 'all' ? 'bg-[var(--accent-primary)] shadow-lg' : 'hover:bg-white/5'}`}
          >
            All
          </button>
          <button
            onClick={() => setFilter('videos')}
            className={`px-6 py-2 rounded-xl transition-all flex items-center gap-2 ${filter === 'videos' ? 'bg-[var(--accent-primary)] shadow-lg' : 'hover:bg-white/5'}`}
          >
            <Play className="w-4 h-4" />
            Videos
          </button>
          <button
            onClick={() => setFilter('images')}
            className={`px-6 py-2 rounded-xl transition-all flex items-center gap-2 ${filter === 'images' ? 'bg-[var(--accent-primary)] shadow-lg' : 'hover:bg-white/5'}`}
          >
            <ImageIcon className="w-4 h-4" />
            Images
          </button>
          <button
            onClick={() => setFilter('favorites')}
            className={`px-6 py-2 rounded-xl transition-all flex items-center gap-2 ${filter === 'favorites' ? 'bg-[var(--accent-primary)] shadow-lg' : 'hover:bg-white/5'}`}
          >
            <Star className="w-4 h-4" />
            Favorites
          </button>
        </div>
        <button className="p-3 rounded-2xl bg-white/5 border border-[var(--border-color)] hover:bg-white/10 transition-all">
          <Filter className="w-5 h-5" />
        </button>
      </div>

      <div className="grid grid-cols-4 gap-6">
        {wallpapers
          .filter(w => {
            if (filter === 'all') return true;
            if (filter === 'videos') return w.type === 'video';
            if (filter === 'images') return w.type === 'image';
            if (filter === 'favorites') return w.favorite;
            return true;
          })
          .map((wallpaper, index) => (
            <motion.div
              key={wallpaper.id}
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: index * 0.05 }}
              className="group cursor-pointer rounded-3xl p-4 backdrop-blur-xl transition-all hover:scale-[1.02]"
              style={{
                background: 'linear-gradient(135deg, rgba(94, 58, 162, 0.4), rgba(117, 89, 203, 0.2))',
                border: '1px solid var(--border-color)',
              }}
            >
              <div className="relative aspect-video rounded-2xl overflow-hidden mb-3">
                <img
                  src={wallpaper.url}
                  alt={wallpaper.title}
                  className="w-full h-full object-cover transition-transform duration-500 group-hover:scale-110"
                />
                {wallpaper.type === 'video' && (
                  <div className="absolute top-2 right-2 px-2 py-1 rounded-lg bg-black/50 backdrop-blur-sm text-xs flex items-center gap-1">
                    <Play className="w-3 h-3" />
                    {wallpaper.duration}
                  </div>
                )}
                <div className="absolute top-2 left-2">
                  <button className="w-8 h-8 rounded-lg bg-black/50 backdrop-blur-sm flex items-center justify-center hover:bg-black/70 transition-colors">
                    <Star className={`w-4 h-4 ${wallpaper.favorite ? 'fill-yellow-400 text-yellow-400' : ''}`} />
                  </button>
                </div>
                <div className="absolute inset-0 bg-gradient-to-t from-black/60 to-transparent opacity-0 group-hover:opacity-100 transition-opacity duration-300" />
                <div className="absolute bottom-2 left-1/2 -translate-x-1/2 opacity-0 group-hover:opacity-100 transition-opacity duration-300">
                  <button className="px-4 py-2 rounded-full bg-gradient-to-r from-[var(--accent-primary)] to-[var(--accent-secondary)] text-sm hover:shadow-lg hover:shadow-purple-500/50 transition-all">
                    Apply
                  </button>
                </div>
              </div>
              <div>
                <div className="mb-1">{wallpaper.title}</div>
                <div className="text-sm text-[var(--text-secondary)]">{wallpaper.resolution}</div>
                <div className="flex gap-1 mt-2">
                  {wallpaper.tags.map(tag => (
                    <span key={tag} className="px-2 py-1 rounded-lg bg-white/10 text-xs text-[var(--text-secondary)]">
                      {tag}
                    </span>
                  ))}
                </div>
              </div>
            </motion.div>
          ))}
      </div>
    </div>
  );
}
