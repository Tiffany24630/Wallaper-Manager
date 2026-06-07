const express = require('express');
const cors = require('cors');
const multer = require('multer');
const fs = require('fs');
const path = require('path');
const { Pool } = require('pg');

const app = express();
const upload = multer({ dest: '/tmp/uploads' });

const DATA_DIR = path.resolve(__dirname, '../database');
const WALLPAPERS_DIR = path.join(DATA_DIR, 'wallpapers');

const pool = new Pool({
  connectionString: process.env.DATABASE_URL || 'postgresql://postgres:postgres@db:5432/wallpaper_manager',
});

app.use(cors());
app.use(express.json());
app.use('/files', express.static(WALLPAPERS_DIR));

async function ensureStructure() {
  fs.mkdirSync(WALLPAPERS_DIR, { recursive: true });
  await pool.query(`
    CREATE TABLE IF NOT EXISTS wallpapers (
      id TEXT PRIMARY KEY,
      title TEXT NOT NULL,
      path TEXT NOT NULL,
      relative_path TEXT NOT NULL,
      kind TEXT NOT NULL,
      size_bytes BIGINT NOT NULL,
      favorite BOOLEAN NOT NULL DEFAULT FALSE,
      active BOOLEAN NOT NULL DEFAULT FALSE,
      created_at TIMESTAMP WITH TIME ZONE DEFAULT now()
    );

    CREATE TABLE IF NOT EXISTS app_settings (
      key TEXT PRIMARY KEY,
      value TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS playlists (
      id TEXT PRIMARY KEY,
      name TEXT NOT NULL,
      wallpaper_ids TEXT[] NOT NULL DEFAULT '{}',
      created_at TIMESTAMP WITH TIME ZONE DEFAULT now()
    );
  `);
}

function kindFromExtension(filename) {
  const ext = path.extname(filename).toLowerCase().replace('.', '');
  const imageExtensions = ['jpg', 'jpeg', 'png', 'bmp', 'webp'];
  const animatedExtensions = ['gif'];
  const videoExtensions = ['mp4', 'webm', 'mov', 'mkv', 'avi'];

  if (imageExtensions.includes(ext)) return animatedExtensions.includes(ext) ? 'animated' : 'image';
  if (videoExtensions.includes(ext)) return 'video';
  return 'unknown';
}

function pathToId(filePath) {
  return filePath.replace(/[^a-zA-Z0-9_-]/g, '_');
}

function normalizeWallpaper(row) {
  return {
    id: row.id,
    title: row.title,
    kind: row.kind,
    path: row.relative_path,
    relativePath: row.relative_path,
    sizeBytes: Number(row.size_bytes),
    favorite: row.favorite,
    active: row.active,
    canApplyToDesktop: row.kind === 'image',
  };
}

app.get('/status', async (req, res) => {
  const { rows } = await pool.query('SELECT COUNT(*) AS total FROM wallpapers');
  const total = Number(rows[0]?.total || 0);
  res.json({ ready: true, backendRoot: DATA_DIR, wallpaperRoot: WALLPAPERS_DIR, configPath: path.join(DATA_DIR, 'config.json'), cacheRoot: path.join(DATA_DIR, 'cache'), logRoot: path.join(DATA_DIR, 'logs'), totalWallpapers: total, totalPlaylists: 0, activeWallpaper: null, uptimeSeconds: 0, supportedImages: ['jpg', 'jpeg', 'png', 'bmp', 'gif', 'webp'], supportedVideos: ['mp4', 'webm', 'mov', 'mkv', 'avi'] });
});

app.get('/wallpapers', async (req, res) => {
  const { rows } = await pool.query('SELECT * FROM wallpapers ORDER BY title');
  res.json(rows.map(normalizeWallpaper));
});

app.post('/wallpapers/import', upload.single('file'), async (req, res) => {
  if (!req.file) {
    return res.status(400).json({ error: 'Missing file upload' });
  }

  const name = path.basename(req.file.originalname);
  const destPath = path.join(WALLPAPERS_DIR, `${Date.now()}-${name}`);
  fs.mkdirSync(WALLPAPERS_DIR, { recursive: true });
  fs.renameSync(req.file.path, destPath);

  const kind = kindFromExtension(name);
  if (kind === 'unknown') {
    fs.unlinkSync(destPath);
    return res.status(400).json({ error: 'Unsupported wallpaper type' });
  }

  const id = pathToId(destPath);
  const stat = fs.statSync(destPath);
  const relative = `/files/${path.basename(destPath)}`;
  await pool.query(
    'INSERT INTO wallpapers (id, title, path, relative_path, kind, size_bytes) VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (id) DO NOTHING',
    [id, name, destPath, relative, kind, stat.size]
  );

  res.json({ id, title: name, path: relative, relativePath: relative, kind, sizeBytes: stat.size, favorite: false, active: false, canApplyToDesktop: kind === 'image' });
});

app.post('/wallpapers/:id/activate', async (req, res) => {
  const { id } = req.params;
  await pool.query('UPDATE wallpapers SET active = FALSE');
  await pool.query('UPDATE wallpapers SET active = TRUE WHERE id = $1', [id]);
  const { rows } = await pool.query('SELECT * FROM wallpapers WHERE id = $1', [id]);
  res.json(rows.length ? normalizeWallpaper(rows[0]) : null);
});

app.post('/wallpapers/:id/favorite', async (req, res) => {
  const { id } = req.params;
  const { rows } = await pool.query('SELECT favorite FROM wallpapers WHERE id = $1', [id]);
  if (!rows.length) return res.status(404).json({ error: 'Wallpaper not found' });

  const nextFavorite = !rows[0].favorite;
  await pool.query('UPDATE wallpapers SET favorite = $1 WHERE id = $2', [nextFavorite, id]);
  const updated = await pool.query('SELECT * FROM wallpapers WHERE id = $1', [id]);
  res.json(normalizeWallpaper(updated.rows[0]));
});

app.get('/playlists', async (req, res) => {
  const { rows } = await pool.query('SELECT * FROM playlists ORDER BY created_at');
  res.json(rows);
});

app.post('/playlists', async (req, res) => {
  const { name } = req.body;
  if (!name) return res.status(400).json({ error: 'Playlist name is required' });
  const id = pathToId(`${name}-${Date.now()}`);
  const { rows } = await pool.query('INSERT INTO playlists (id, name, wallpaper_ids) VALUES ($1, $2, $3) RETURNING *', [id, name, []]);
  res.json(rows[0]);
});

app.post('/playlists/:id/add', async (req, res) => {
  const { id } = req.params;
  const { wallpaperId } = req.body;
  if (!wallpaperId) return res.status(400).json({ error: 'Wallpaper id is required' });

  const { rows } = await pool.query('SELECT wallpaper_ids FROM playlists WHERE id = $1', [id]);
  if (!rows.length) return res.status(404).json({ error: 'Playlist not found' });

  const wallpaperIds = rows[0].wallpaper_ids || [];
  if (!wallpaperIds.includes(wallpaperId)) {
    wallpaperIds.push(wallpaperId);
    await pool.query('UPDATE playlists SET wallpaper_ids = $1 WHERE id = $2', [wallpaperIds, id]);
  }

  const updated = await pool.query('SELECT * FROM playlists WHERE id = $1', [id]);
  res.json(updated.rows[0]);
});

app.get('/settings', async (req, res) => {
  const { rows } = await pool.query('SELECT key, value FROM app_settings');
  const settings = rows.reduce((acc, row) => ({ ...acc, [row.key]: JSON.parse(row.value) }), {});
  res.json(settings);
});

app.post('/settings', async (req, res) => {
  const settings = req.body;
  for (const key of Object.keys(settings)) {
    await pool.query('INSERT INTO app_settings (key, value) VALUES ($1, $2) ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value', [key, JSON.stringify(settings[key])]);
  }
  res.json(settings);
});

app.post('/playlists/:id/delete', async (req, res) => {
  const { id } = req.params;
  await pool.query('DELETE FROM playlists WHERE id = $1', [id]);
  const { rows } = await pool.query('SELECT * FROM playlists ORDER BY created_at');
  res.json(rows);
});

app.listen(4000, async () => {
  await ensureStructure();
  console.log('Backend API listening on port 4000');
});
