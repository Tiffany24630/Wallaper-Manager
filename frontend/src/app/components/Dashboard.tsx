import {
  Bell,
  Cpu,
  Download,
  HardDrive,
  Image,
  Layers,
  Monitor,
  Play,
  Plus,
  RefreshCw,
  Search,
  Settings2,
  SlidersHorizontal,
  Sparkles,
  Star,
  Trash2,
  Upload,
  X,
  Zap,
  type LucideIcon,
} from "lucide-react";
import type { AppView, WallpaperFilter } from "../App";
import {
  previewUrl,
  type AppSettings,
  type BackendStatus,
  type Playlist,
  type Wallpaper,
} from "../../services/wallpaperService";

interface DashboardProps {
  activeView: AppView;
  backendAvailable: boolean;
  busy: boolean;
  filter: WallpaperFilter;
  filteredWallpapers: Wallpaper[];
  importOpen: boolean;
  importPath: string;
  message: string;
  playlistName: string;
  playlists: Playlist[];
  query: string;
  selectedPlaylist: Playlist | null;
  selectedPlaylistWallpapers: Wallpaper[];
  settings: AppSettings;
  showActivity: boolean;
  status: BackendStatus | null;
  wallpapers: Wallpaper[];
  onAddToPlaylist: (playlistId: string, wallpaperId: string) => void;
  onCreatePlaylist: () => void;
  onCycleFilter: () => void;
  onDeletePlaylist: (playlistId: string) => void;
  onFilterChange: (filter: WallpaperFilter) => void;
  onImport: () => void;
  onImportOpenChange: (open: boolean) => void;
  onImportPathChange: (path: string) => void;
  onImportFileChange: (file: File | null) => void;
  onPlaylistNameChange: (name: string) => void;
  onQueryChange: (query: string) => void;
  onRefresh: () => void;
  onSelectedPlaylistChange: (playlistId: string | null) => void;
  onSetActive: (wallpaperId: string) => void;
  onSettingsChange: (settings: AppSettings) => void;
  onToggleActivity: () => void;
  onToggleFavorite: (wallpaperId: string) => void;
  onViewChange: (view: AppView) => void;
}

const filters: Array<{ value: WallpaperFilter; label: string }> = [
  { value: "all", label: "All" },
  { value: "favorites", label: "Favorites" },
  { value: "image", label: "Images" },
  { value: "video", label: "Videos" },
  { value: "animated", label: "Animated" },
];

const viewCopy: Record<AppView, { title: string; subtitle: string }> = {
  dashboard: {
    title: "Dashboard",
    subtitle: "Manage active wallpapers, monitor rendering load, and keep playlists ready.",
  },
  library: {
    title: "Library",
    subtitle: "Search, favorite, import, and apply wallpapers from the local backend.",
  },
  playlists: {
    title: "Playlists",
    subtitle: "Create playlists and attach wallpapers stored by the backend.",
  },
  performance: {
    title: "Performance",
    subtitle: "Tune quality, FPS, and hardware behavior persisted in backend settings.",
  },
  settings: {
    title: "Settings",
    subtitle: "Application preferences saved to backend/config/state.json.",
  },
};

export default function Dashboard(props: DashboardProps) {
  const {
    activeView,
    backendAvailable,
    busy,
    filter,
    filteredWallpapers,
    importOpen,
    importPath,
    message,
    playlistName,
    playlists,
    query,
    selectedPlaylist,
    selectedPlaylistWallpapers,
    settings,
    showActivity,
    status,
    wallpapers,
    onAddToPlaylist,
    onCreatePlaylist,
    onCycleFilter,
    onDeletePlaylist,
    onFilterChange,
    onImport,
    onImportOpenChange,
    onImportPathChange,
    onPlaylistNameChange,
    onQueryChange,
    onRefresh,
    onSelectedPlaylistChange,
    onSetActive,
    onSettingsChange,
    onToggleActivity,
    onToggleFavorite,
    onViewChange,
  } = props;

  const activeWallpaper = status?.activeWallpaper ?? wallpapers.find((wallpaper) => wallpaper.active);
  const activity = buildActivity(status, wallpapers, playlists, message);

  return (
    <section className="mx-auto flex w-full max-w-[1540px] flex-col gap-6 px-4 py-5 sm:px-6 lg:px-8 lg:py-7">
      <header className="flex flex-col gap-4 border-b border-white/10 pb-5 xl:flex-row xl:items-center xl:justify-between">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2 text-sm text-slate-400">
            <span
              className={`rounded-md border px-2 py-1 ${
                backendAvailable
                  ? "border-emerald-300/20 bg-emerald-300/10 text-emerald-200"
                  : "border-amber-300/20 bg-amber-300/10 text-amber-200"
              }`}
            >
              {backendAvailable ? "Backend live" : "Backend offline"}
            </span>
            <span>{message}</span>
          </div>

          <h1 className="mt-3 text-3xl font-semibold tracking-normal text-white sm:text-4xl">
            {viewCopy[activeView].title}
          </h1>
          <p className="mt-2 max-w-2xl text-sm leading-6 text-slate-400 sm:text-base">
            {viewCopy[activeView].subtitle}
          </p>
        </div>

        <div className="flex w-full flex-col gap-3 sm:flex-row xl:w-auto">
          <label className="relative min-w-0 flex-1 xl:w-80 xl:flex-none">
            <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-500" />
            <input
              type="search"
              value={query}
              onChange={(event) => onQueryChange(event.target.value)}
              placeholder="Search wallpapers"
              className="h-11 w-full rounded-lg border border-white/10 bg-white/[0.05] pl-10 pr-3 text-sm text-white placeholder:text-slate-500"
            />
          </label>

          <div className="flex gap-2">
            <button
              type="button"
              onClick={onToggleActivity}
              className="flex h-11 w-11 items-center justify-center rounded-lg border border-white/10 bg-white/[0.05] text-slate-300 transition hover:bg-white/10 hover:text-white"
              title="Toggle activity"
              aria-label="Toggle activity"
            >
              <Bell size={18} />
            </button>
            <button
              type="button"
              onClick={onCycleFilter}
              className="flex h-11 w-11 items-center justify-center rounded-lg border border-white/10 bg-white/[0.05] text-slate-300 transition hover:bg-white/10 hover:text-white"
              title={`Filter: ${filter}`}
              aria-label="Cycle wallpaper filter"
            >
              <SlidersHorizontal size={18} />
            </button>
            <button
              type="button"
              onClick={onRefresh}
              disabled={busy}
              className="flex h-11 w-11 items-center justify-center rounded-lg border border-white/10 bg-white/[0.05] text-slate-300 transition hover:bg-white/10 hover:text-white disabled:cursor-not-allowed disabled:opacity-50"
              title="Rescan backend"
              aria-label="Rescan backend"
            >
              <RefreshCw size={18} className={busy ? "animate-spin" : ""} />
            </button>
            <button
              type="button"
              onClick={() => onImportOpenChange(true)}
              className="inline-flex h-11 items-center gap-2 rounded-lg bg-cyan-300 px-4 text-sm font-semibold text-slate-950 transition hover:bg-cyan-200"
            >
              <Upload size={18} />
              Upload
            </button>
          </div>
        </div>
      </header>

      {activeView === "dashboard" && (
        <DashboardView
          activeWallpaper={activeWallpaper}
          activity={activity}
          playlists={playlists}
          settings={settings}
          showActivity={showActivity}
          status={status}
          wallpapers={wallpapers}
          onSetActive={onSetActive}
          onToggleFavorite={onToggleFavorite}
          onViewChange={onViewChange}
        />
      )}

      {activeView === "library" && (
        <LibraryView
          filter={filter}
          filteredWallpapers={filteredWallpapers}
          playlists={playlists}
          selectedPlaylist={selectedPlaylist}
          onAddToPlaylist={onAddToPlaylist}
          onFilterChange={onFilterChange}
          onSetActive={onSetActive}
          onToggleFavorite={onToggleFavorite}
        />
      )}

      {activeView === "playlists" && (
        <PlaylistsView
          activeWallpaper={activeWallpaper}
          playlistName={playlistName}
          playlists={playlists}
          selectedPlaylist={selectedPlaylist}
          selectedPlaylistWallpapers={selectedPlaylistWallpapers}
          wallpapers={wallpapers}
          onAddToPlaylist={onAddToPlaylist}
          onCreatePlaylist={onCreatePlaylist}
          onDeletePlaylist={onDeletePlaylist}
          onPlaylistNameChange={onPlaylistNameChange}
          onSelectedPlaylistChange={onSelectedPlaylistChange}
          onSetActive={onSetActive}
          onToggleFavorite={onToggleFavorite}
        />
      )}

      {activeView === "performance" && (
        <PerformanceView settings={settings} status={status} onSettingsChange={onSettingsChange} />
      )}

      {activeView === "settings" && (
        <SettingsView settings={settings} status={status} onSettingsChange={onSettingsChange} />
      )}

      {importOpen && (
        <ImportDialog
          busy={busy}
          importPath={importPath}
          onClose={() => onImportOpenChange(false)}
          onImport={onImport}
          onImportPathChange={onImportPathChange}
          onImportFileChange={onImportFileChange}
        />
      )}
    </section>
  );
}

function DashboardView({
  activeWallpaper,
  activity,
  playlists,
  settings,
  showActivity,
  status,
  wallpapers,
  onSetActive,
  onToggleFavorite,
  onViewChange,
}: {
  activeWallpaper?: Wallpaper | null;
  activity: string[];
  playlists: Playlist[];
  settings: AppSettings;
  showActivity: boolean;
  status: BackendStatus | null;
  wallpapers: Wallpaper[];
  onSetActive: (wallpaperId: string) => void;
  onToggleFavorite: (wallpaperId: string) => void;
  onViewChange: (view: AppView) => void;
}) {
  const stats = [
    {
      title: "Wallpapers",
      value: String(status?.totalWallpapers ?? wallpapers.length),
      detail: "Indexed locally",
      icon: Image,
      tone: "text-cyan-300",
      surface: "bg-cyan-300/10",
    },
    {
      title: "Playlists",
      value: String(status?.totalPlaylists ?? playlists.length),
      detail: "Saved in backend",
      icon: Layers,
      tone: "text-violet-300",
      surface: "bg-violet-300/10",
    },
    {
      title: "Backend",
      value: status?.ready ? "Ready" : "Offline",
      detail: status ? formatDuration(status.uptimeSeconds) : "Waiting for Tauri",
      icon: Cpu,
      tone: "text-emerald-300",
      surface: "bg-emerald-300/10",
    },
    {
      title: "FPS Limit",
      value: String(settings.fpsLimit),
      detail: settings.quality,
      icon: Zap,
      tone: "text-amber-300",
      surface: "bg-amber-300/10",
    },
  ];

  return (
    <>
      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
        {stats.map((stat) => (
          <StatCard key={stat.title} {...stat} />
        ))}
      </div>

      <div className="grid gap-6 xl:grid-cols-[minmax(0,1.55fr)_minmax(320px,.85fr)]">
        <CurrentWallpaper
          wallpaper={activeWallpaper}
          onSetActive={onSetActive}
          onOpenSettings={() => onViewChange("settings")}
        />

        <div className="grid gap-6 md:grid-cols-2 xl:grid-cols-1">
          <PerformanceCard settings={settings} status={status} />
          {showActivity && <ActivityCard activity={activity} />}
        </div>
      </div>

      <div>
        <div className="mb-3 flex items-center justify-between gap-3">
          <h2 className="text-lg font-semibold text-white">Recently Used</h2>
          <button
            type="button"
            onClick={() => onViewChange("playlists")}
            className="inline-flex items-center gap-2 rounded-lg border border-white/10 bg-white/[0.04] px-3 py-2 text-sm text-slate-300 transition hover:bg-white/10 hover:text-white"
          >
            <Plus size={16} />
            New playlist
          </button>
        </div>

        <div className="grid gap-4 md:grid-cols-2 2xl:grid-cols-3">
          {wallpapers.slice(0, 6).map((wallpaper) => (
            <WallpaperCard
              key={wallpaper.id}
              wallpaper={wallpaper}
              onSetActive={onSetActive}
              onToggleFavorite={onToggleFavorite}
            />
          ))}
          {!wallpapers.length && <EmptyState text="No wallpapers yet. Upload one to start." />}
        </div>
      </div>
    </>
  );
}

function LibraryView({
  filter,
  filteredWallpapers,
  playlists,
  selectedPlaylist,
  onAddToPlaylist,
  onFilterChange,
  onSetActive,
  onToggleFavorite,
}: {
  filter: WallpaperFilter;
  filteredWallpapers: Wallpaper[];
  playlists: Playlist[];
  selectedPlaylist: Playlist | null;
  onAddToPlaylist: (playlistId: string, wallpaperId: string) => void;
  onFilterChange: (filter: WallpaperFilter) => void;
  onSetActive: (wallpaperId: string) => void;
  onToggleFavorite: (wallpaperId: string) => void;
}) {
  return (
    <>
      <div className="flex flex-wrap gap-2 rounded-lg border border-white/10 bg-white/[0.04] p-2">
        {filters.map((item) => (
          <button
            key={item.value}
            type="button"
            onClick={() => onFilterChange(item.value)}
            className={`rounded-md px-3 py-2 text-sm transition ${
              filter === item.value
                ? "bg-cyan-300 text-slate-950"
                : "text-slate-300 hover:bg-white/10 hover:text-white"
            }`}
          >
            {item.label}
          </button>
        ))}
      </div>

      <div className="grid gap-4 md:grid-cols-2 2xl:grid-cols-3">
        {filteredWallpapers.map((wallpaper) => (
          <WallpaperCard
            key={wallpaper.id}
            playlist={selectedPlaylist}
            playlists={playlists}
            wallpaper={wallpaper}
            onAddToPlaylist={onAddToPlaylist}
            onSetActive={onSetActive}
            onToggleFavorite={onToggleFavorite}
          />
        ))}
        {!filteredWallpapers.length && <EmptyState text="No wallpapers match the current search." />}
      </div>
    </>
  );
}

function PlaylistsView({
  activeWallpaper,
  playlistName,
  playlists,
  selectedPlaylist,
  selectedPlaylistWallpapers,
  wallpapers,
  onAddToPlaylist,
  onCreatePlaylist,
  onDeletePlaylist,
  onPlaylistNameChange,
  onSelectedPlaylistChange,
  onSetActive,
  onToggleFavorite,
}: {
  activeWallpaper?: Wallpaper | null;
  playlistName: string;
  playlists: Playlist[];
  selectedPlaylist: Playlist | null;
  selectedPlaylistWallpapers: Wallpaper[];
  wallpapers: Wallpaper[];
  onAddToPlaylist: (playlistId: string, wallpaperId: string) => void;
  onCreatePlaylist: () => void;
  onDeletePlaylist: (playlistId: string) => void;
  onPlaylistNameChange: (name: string) => void;
  onSelectedPlaylistChange: (playlistId: string | null) => void;
  onSetActive: (wallpaperId: string) => void;
  onToggleFavorite: (wallpaperId: string) => void;
}) {
  return (
    <div className="grid gap-6 xl:grid-cols-[360px_minmax(0,1fr)]">
      <section className="rounded-lg border border-white/10 bg-white/[0.055] p-5 shadow-panel backdrop-blur-xl">
        <h2 className="text-lg font-semibold text-white">Create playlist</h2>
        <div className="mt-4 flex gap-2">
          <input
            value={playlistName}
            onChange={(event) => onPlaylistNameChange(event.target.value)}
            placeholder="Evening rotation"
            className="h-11 min-w-0 flex-1 rounded-lg border border-white/10 bg-white/[0.05] px-3 text-sm text-white placeholder:text-slate-500"
          />
          <button
            type="button"
            onClick={onCreatePlaylist}
            className="inline-flex h-11 items-center gap-2 rounded-lg bg-cyan-300 px-4 text-sm font-semibold text-slate-950 transition hover:bg-cyan-200"
          >
            <Plus size={17} />
            Create
          </button>
        </div>

        <div className="mt-5 space-y-2">
          {playlists.map((playlist) => (
            <div
              key={playlist.id}
              className={`flex items-center justify-between gap-3 rounded-lg border px-3 py-2 ${
                selectedPlaylist?.id === playlist.id
                  ? "border-cyan-300/30 bg-cyan-300/10"
                  : "border-white/10 bg-white/[0.04]"
              }`}
            >
              <button
                type="button"
                onClick={() => onSelectedPlaylistChange(playlist.id)}
                className="min-w-0 flex-1 text-left"
              >
                <span className="block truncate text-sm font-semibold text-white">
                  {playlist.name}
                </span>
                <span className="text-xs text-slate-400">
                  {playlist.wallpaperIds.length} wallpapers
                </span>
              </button>
              <button
                type="button"
                onClick={() => onDeletePlaylist(playlist.id)}
                className="flex h-8 w-8 items-center justify-center rounded-md text-slate-400 transition hover:bg-rose-400/10 hover:text-rose-200"
                title="Delete playlist"
                aria-label="Delete playlist"
              >
                <Trash2 size={16} />
              </button>
            </div>
          ))}
          {!playlists.length && <p className="text-sm text-slate-400">No playlists yet.</p>}
        </div>
      </section>

      <section className="rounded-lg border border-white/10 bg-white/[0.055] p-5 shadow-panel backdrop-blur-xl">
        <div className="mb-4 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <div>
            <h2 className="text-lg font-semibold text-white">
              {selectedPlaylist?.name ?? "Select a playlist"}
            </h2>
            <p className="text-sm text-slate-400">
              {selectedPlaylist
                ? "Apply wallpapers or add the active wallpaper to this playlist."
                : "Create or select a playlist to manage it."}
            </p>
          </div>
          <button
            type="button"
            disabled={!selectedPlaylist || !activeWallpaper}
            onClick={() =>
              selectedPlaylist && activeWallpaper
                ? onAddToPlaylist(selectedPlaylist.id, activeWallpaper.id)
                : undefined
            }
            className="inline-flex h-10 items-center justify-center gap-2 rounded-lg border border-white/10 bg-white/[0.04] px-3 text-sm text-slate-300 transition hover:bg-white/10 hover:text-white disabled:cursor-not-allowed disabled:opacity-50"
          >
            <Plus size={16} />
            Add active
          </button>
        </div>

        <div className="grid gap-4 md:grid-cols-2">
          {selectedPlaylistWallpapers.map((wallpaper) => (
            <WallpaperCard
              key={wallpaper.id}
              wallpaper={wallpaper}
              onSetActive={onSetActive}
              onToggleFavorite={onToggleFavorite}
            />
          ))}
          {selectedPlaylist && !selectedPlaylistWallpapers.length && (
            <EmptyState text="This playlist is empty. Add the active wallpaper or use Library cards." />
          )}
          {!selectedPlaylist && (
            <EmptyState text={`${wallpapers.length} wallpapers available for playlist use.`} />
          )}
        </div>
      </section>
    </div>
  );
}

function PerformanceView({
  settings,
  status,
  onSettingsChange,
}: {
  settings: AppSettings;
  status: BackendStatus | null;
  onSettingsChange: (settings: AppSettings) => void;
}) {
  const gpuLoad = settings.hardwareAcceleration ? 12 : 4;
  const cpuLoad = settings.hardwareAcceleration ? 24 : 38;
  const ramLoad = status?.activeWallpaper ? 28 : 16;

  return (
    <div className="grid gap-6 xl:grid-cols-[minmax(0,1fr)_360px]">
      <section className="rounded-lg border border-white/10 bg-white/[0.055] p-5 shadow-panel backdrop-blur-xl">
        <div className="mb-6 flex items-center justify-between gap-3">
          <div>
            <h2 className="text-lg font-semibold text-white">Renderer load</h2>
            <p className="text-sm text-slate-400">Live controls backed by saved settings.</p>
          </div>
          <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-emerald-300/10 text-emerald-300">
            <Sparkles size={18} />
          </div>
        </div>
        <div className="space-y-5">
          <Bar label="CPU" width={`${cpuLoad}%`} tone="bg-cyan-300" />
          <Bar label="GPU" width={`${gpuLoad}%`} tone="bg-emerald-300" />
          <Bar label="RAM" width={`${ramLoad}%`} tone="bg-amber-300" />
        </div>
      </section>

      <section className="rounded-lg border border-white/10 bg-white/[0.055] p-5 shadow-panel backdrop-blur-xl">
        <h2 className="text-lg font-semibold text-white">FPS limiter</h2>
        <div className="mt-4 grid grid-cols-2 gap-2">
          {[30, 60, 120, 144].map((fps) => (
            <button
              key={fps}
              type="button"
              onClick={() => onSettingsChange({ ...settings, fpsLimit: fps })}
              className={`rounded-lg px-3 py-2 text-sm transition ${
                settings.fpsLimit === fps
                  ? "bg-cyan-300 text-slate-950"
                  : "bg-white/[0.05] text-slate-300 hover:bg-white/10 hover:text-white"
              }`}
            >
              {fps} FPS
            </button>
          ))}
        </div>
        <ToggleRow
          checked={settings.hardwareAcceleration}
          label="Hardware acceleration"
          onChange={(checked) => onSettingsChange({ ...settings, hardwareAcceleration: checked })}
        />
      </section>
    </div>
  );
}

function SettingsView({
  settings,
  status,
  onSettingsChange,
}: {
  settings: AppSettings;
  status: BackendStatus | null;
  onSettingsChange: (settings: AppSettings) => void;
}) {
  return (
    <div className="grid gap-6 xl:grid-cols-2">
      <section className="rounded-lg border border-white/10 bg-white/[0.055] p-5 shadow-panel backdrop-blur-xl">
        <h2 className="text-lg font-semibold text-white">Application</h2>
        <div className="mt-4 space-y-4">
          <ToggleRow
            checked={settings.launchAtStartup}
            label="Launch at startup"
            onChange={(checked) => onSettingsChange({ ...settings, launchAtStartup: checked })}
          />
          <ToggleRow
            checked={settings.minimizeToTray}
            label="Minimize to tray"
            onChange={(checked) => onSettingsChange({ ...settings, minimizeToTray: checked })}
          />
          <ToggleRow
            checked={settings.pauseOnBattery}
            label="Pause on battery"
            onChange={(checked) => onSettingsChange({ ...settings, pauseOnBattery: checked })}
          />
          <ToggleRow
            checked={settings.pauseWhenMaximized}
            label="Pause when maximized"
            onChange={(checked) => onSettingsChange({ ...settings, pauseWhenMaximized: checked })}
          />
        </div>
      </section>

      <section className="rounded-lg border border-white/10 bg-white/[0.055] p-5 shadow-panel backdrop-blur-xl">
        <h2 className="text-lg font-semibold text-white">Wallpaper rendering</h2>
        <div className="mt-4 grid gap-4">
          <label className="grid gap-2 text-sm text-slate-300">
            Quality
            <select
              value={settings.quality}
              onChange={(event) => onSettingsChange({ ...settings, quality: event.target.value })}
              className="h-11 rounded-lg border border-white/10 bg-slate-950 px-3 text-white"
            >
              <option>Ultra</option>
              <option>High</option>
              <option>Medium</option>
              <option>Low</option>
            </select>
          </label>
          <label className="grid gap-2 text-sm text-slate-300">
            Scaling mode
            <select
              value={settings.scalingMode}
              onChange={(event) =>
                onSettingsChange({ ...settings, scalingMode: event.target.value })
              }
              className="h-11 rounded-lg border border-white/10 bg-slate-950 px-3 text-white"
            >
              <option>Fill</option>
              <option>Fit</option>
              <option>Stretch</option>
              <option>Center</option>
            </select>
          </label>
          <div className="rounded-lg border border-white/10 bg-white/[0.04] p-4">
            <p className="text-sm text-slate-400">Config path</p>
            <p className="mt-2 break-all text-sm text-slate-200">
              {status?.configPath ?? "Waiting for backend"}
            </p>
          </div>
        </div>
      </section>
    </div>
  );
}

function CurrentWallpaper({
  wallpaper,
  onOpenSettings,
  onSetActive,
}: {
  wallpaper?: Wallpaper | null;
  onOpenSettings: () => void;
  onSetActive: (wallpaperId: string) => void;
}) {
  const imageSrc = previewUrl(wallpaper);

  return (
    <article className="overflow-hidden rounded-lg border border-white/10 bg-white/[0.055] shadow-panel backdrop-blur-xl">
      <div className="relative aspect-[16/9] min-h-[300px] overflow-hidden bg-gradient-to-br from-cyan-500 via-slate-900 to-emerald-500">
        {imageSrc && (
          <img
            src={imageSrc}
            alt={wallpaper?.title}
            className="h-full w-full object-cover"
            onError={(event) => {
              event.currentTarget.style.display = "none";
            }}
          />
        )}
        <div className="absolute inset-0 bg-gradient-to-t from-slate-950/80 via-slate-950/20 to-transparent" />
        <div className="absolute left-4 right-4 top-4 flex items-center justify-between gap-3">
          <span className="rounded-md border border-emerald-300/30 bg-emerald-400/15 px-2.5 py-1 text-xs font-medium text-emerald-100 backdrop-blur">
            {wallpaper ? "Active wallpaper" : "No active wallpaper"}
          </span>
          <div className="flex gap-2">
            <button
              type="button"
              disabled={!wallpaper}
              onClick={() => wallpaper && onSetActive(wallpaper.id)}
              className="flex h-9 w-9 items-center justify-center rounded-lg bg-slate-950/50 text-white backdrop-blur transition hover:bg-slate-900 disabled:cursor-not-allowed disabled:opacity-50"
              title="Apply again"
              aria-label="Apply again"
            >
              <Play size={17} />
            </button>
            <button
              type="button"
              onClick={onOpenSettings}
              className="flex h-9 w-9 items-center justify-center rounded-lg bg-slate-950/50 text-white backdrop-blur transition hover:bg-slate-900"
              title="Wallpaper settings"
              aria-label="Wallpaper settings"
            >
              <Settings2 size={17} />
            </button>
          </div>
        </div>
        <div className="absolute bottom-0 left-0 right-0 p-5">
          <p className="text-sm text-cyan-100">
            {wallpaper?.relativePath ?? "Import or select a wallpaper from Library"}
          </p>
          <h2 className="mt-2 text-2xl font-semibold text-white sm:text-3xl">
            {wallpaper?.title ?? "Nothing selected"}
          </h2>
          <div className="mt-4 grid gap-3 text-sm text-slate-200 sm:grid-cols-3">
            <Metric icon={Monitor} label="Type" value={wallpaper?.kind ?? "None"} />
            <Metric icon={Zap} label="Desktop" value={wallpaper?.canApplyToDesktop ? "Applies" : "Select only"} />
            <Metric icon={HardDrive} label="Size" value={formatBytes(wallpaper?.sizeBytes ?? 0)} />
          </div>
        </div>
      </div>
    </article>
  );
}

function WallpaperCard({
  playlist,
  playlists = [],
  wallpaper,
  onAddToPlaylist,
  onSetActive,
  onToggleFavorite,
}: {
  playlist?: Playlist | null;
  playlists?: Playlist[];
  wallpaper: Wallpaper;
  onAddToPlaylist?: (playlistId: string, wallpaperId: string) => void;
  onSetActive: (wallpaperId: string) => void;
  onToggleFavorite: (wallpaperId: string) => void;
}) {
  const imageSrc = previewUrl(wallpaper);

  return (
    <article className={`group overflow-hidden rounded-lg border bg-white/[0.05] transition hover:-translate-y-0.5 hover:border-white/20 hover:bg-white/[0.075] ${
      wallpaper.active ? "border-cyan-300/50" : "border-white/10"
    }`}>
      <div className={`relative aspect-video overflow-hidden ${kindSurface(wallpaper.kind)}`}>
        {imageSrc && (
          <img
            src={imageSrc}
            alt={wallpaper.title}
            className="h-full w-full object-cover transition duration-500 group-hover:scale-105"
            onError={(event) => {
              event.currentTarget.style.display = "none";
            }}
          />
        )}
        <div className="absolute inset-x-0 bottom-0 h-20 bg-gradient-to-t from-slate-950/70 to-transparent" />
        <span className="absolute left-3 top-3 rounded-md border border-white/10 bg-slate-950/50 px-2 py-1 text-xs text-slate-200 backdrop-blur">
          {wallpaper.kind}
        </span>
        <button
          type="button"
          onClick={() => onToggleFavorite(wallpaper.id)}
          className="absolute right-3 top-3 flex h-8 w-8 items-center justify-center rounded-lg bg-slate-950/50 text-white backdrop-blur transition hover:bg-slate-900"
          title={wallpaper.favorite ? "Remove favorite" : "Add favorite"}
          aria-label={wallpaper.favorite ? "Remove favorite" : "Add favorite"}
        >
          <Star size={16} className={wallpaper.favorite ? "fill-amber-300 text-amber-300" : ""} />
        </button>
      </div>
      <div className="grid gap-3 p-4">
        <div className="min-w-0">
          <h3 className="truncate text-base font-semibold text-white">{wallpaper.title}</h3>
          <p className="mt-1 truncate text-sm text-slate-400">
            {wallpaper.relativePath} - {formatBytes(wallpaper.sizeBytes)}
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            onClick={() => onSetActive(wallpaper.id)}
            className="inline-flex h-9 flex-1 items-center justify-center rounded-lg bg-cyan-300 px-3 text-sm font-semibold text-slate-950 transition hover:bg-cyan-200"
          >
            {wallpaper.canApplyToDesktop ? "Apply" : "Select"}
          </button>
          {playlist && onAddToPlaylist && (
            <button
              type="button"
              onClick={() => onAddToPlaylist(playlist.id, wallpaper.id)}
              className="inline-flex h-9 items-center justify-center rounded-lg border border-white/10 bg-white/[0.04] px-3 text-sm text-slate-300 transition hover:bg-white/10 hover:text-white"
            >
              Add
            </button>
          )}
          {!playlist && playlists.length > 0 && onAddToPlaylist && (
            <button
              type="button"
              onClick={() => onAddToPlaylist(playlists[0].id, wallpaper.id)}
              className="inline-flex h-9 items-center justify-center rounded-lg border border-white/10 bg-white/[0.04] px-3 text-sm text-slate-300 transition hover:bg-white/10 hover:text-white"
              title={`Add to ${playlists[0].name}`}
            >
              Playlist
            </button>
          )}
        </div>
      </div>
    </article>
  );
}

function StatCard({
  title,
  value,
  detail,
  icon: Icon,
  tone,
  surface,
}: {
  title: string;
  value: string;
  detail: string;
  icon: LucideIcon;
  tone: string;
  surface: string;
}) {
  return (
    <div className="rounded-lg border border-white/10 bg-white/[0.055] p-5 shadow-panel backdrop-blur-xl">
      <div className="flex items-center justify-between">
        <span className="text-sm text-slate-400">{title}</span>
        <div className={`flex h-9 w-9 items-center justify-center rounded-lg ${surface} ${tone}`}>
          <Icon size={18} />
        </div>
      </div>
      <div className="mt-5 truncate text-3xl font-semibold tracking-normal text-white">
        {value}
      </div>
      <p className="mt-1 truncate text-sm text-slate-500">{detail}</p>
    </div>
  );
}

function PerformanceCard({
  settings,
  status,
}: {
  settings: AppSettings;
  status: BackendStatus | null;
}) {
  return (
    <section className="rounded-lg border border-white/10 bg-white/[0.055] p-5 shadow-panel backdrop-blur-xl">
      <div className="flex items-center justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold text-white">Performance</h2>
          <p className="mt-1 text-sm text-slate-400">Renderer load</p>
        </div>
        <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-emerald-300/10 text-emerald-300">
          <Sparkles size={18} />
        </div>
      </div>
      <div className="mt-6 space-y-5">
        <Bar label="CPU" width={settings.hardwareAcceleration ? "24%" : "38%"} tone="bg-cyan-300" />
        <Bar label="GPU" width={settings.hardwareAcceleration ? "12%" : "4%"} tone="bg-emerald-300" />
        <Bar label="RAM" width={status?.activeWallpaper ? "28%" : "16%"} tone="bg-amber-300" />
      </div>
    </section>
  );
}

function ActivityCard({ activity }: { activity: string[] }) {
  return (
    <section className="rounded-lg border border-white/10 bg-white/[0.055] p-5 shadow-panel backdrop-blur-xl">
      <div className="mb-4 flex items-center justify-between">
        <div>
          <h2 className="text-lg font-semibold text-white">Activity</h2>
          <p className="mt-1 text-sm text-slate-400">Latest events</p>
        </div>
        <button
          type="button"
          onClick={() => downloadActivity(activity)}
          className="flex h-10 w-10 items-center justify-center rounded-lg border border-white/10 bg-white/[0.04] text-slate-300 transition hover:bg-white/10 hover:text-white"
          title="Download activity"
          aria-label="Download activity"
        >
          <Download size={17} />
        </button>
      </div>
      <div className="space-y-3">
        {activity.map((item, index) => (
          <div key={`${item}-${index}`} className="flex items-start gap-3">
            <span className="mt-1 flex h-6 w-6 shrink-0 items-center justify-center rounded-md bg-white/[0.06] text-xs text-cyan-200">
              {index + 1}
            </span>
            <p className="text-sm leading-6 text-slate-300">{item}</p>
          </div>
        ))}
      </div>
    </section>
  );
}

function Bar({ label, width, tone }: { label: string; width: string; tone: string }) {
  return (
    <div>
      <div className="mb-2 flex items-center justify-between text-sm">
        <span className="text-slate-400">{label}</span>
        <span className="text-slate-100">{width}</span>
      </div>
      <div className="h-2 overflow-hidden rounded-full bg-slate-800">
        <div className={`h-full rounded-full ${tone}`} style={{ width }} />
      </div>
    </div>
  );
}

function Metric({
  icon: Icon,
  label,
  value,
}: {
  icon: LucideIcon;
  label: string;
  value: string;
}) {
  return (
    <div className="flex items-center gap-2 rounded-lg bg-slate-950/40 px-3 py-2 backdrop-blur">
      <Icon size={16} className="shrink-0 text-cyan-200" />
      <span className="min-w-0">
        <span className="block text-xs text-slate-400">{label}</span>
        <span className="block truncate font-medium text-white">{value}</span>
      </span>
    </div>
  );
}

function ToggleRow({
  checked,
  label,
  onChange,
}: {
  checked: boolean;
  label: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="flex items-center justify-between gap-3 rounded-lg border border-white/10 bg-white/[0.04] p-3 text-sm text-slate-200">
      <span>{label}</span>
      <button
        type="button"
        onClick={() => onChange(!checked)}
        className={`flex h-6 w-11 items-center rounded-full p-1 transition ${
          checked ? "justify-end bg-cyan-300" : "justify-start bg-slate-700"
        }`}
        aria-pressed={checked}
      >
        <span className="h-4 w-4 rounded-full bg-white" />
      </button>
    </label>
  );
}

function ImportDialog({
  busy,
  importPath,
  onClose,
  onImport,
  onImportPathChange,
  onImportFileChange,
}: {
  busy: boolean;
  importPath: string;
  onClose: () => void;
  onImport: () => void;
  onImportPathChange: (path: string) => void;
  onImportFileChange: (file: File | null) => void;
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/70 p-4 backdrop-blur">
      <div className="w-full max-w-xl rounded-lg border border-white/10 bg-slate-950 p-5 shadow-panel">
        <div className="mb-4 flex items-start justify-between gap-4">
          <div>
            <h2 className="text-lg font-semibold text-white">Import wallpaper</h2>
            <p className="mt-1 text-sm text-slate-400">
              Paste a local file path or choose a file from your browser.
            </p>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="flex h-9 w-9 items-center justify-center rounded-lg border border-white/10 bg-white/[0.04] text-slate-300 transition hover:bg-white/10 hover:text-white"
            aria-label="Close import dialog"
          >
            <X size={17} />
          </button>
        </div>
        <input
          value={importPath}
          onChange={(event) => onImportPathChange(event.target.value)}
          placeholder="C:\\Users\\Tiffa\\Pictures\\wallpaper.jpg"
          className="h-11 w-full rounded-lg border border-white/10 bg-white/[0.05] px-3 text-sm text-white placeholder:text-slate-500"
        />
        <div className="mt-3">
          <label className="mb-2 block text-sm text-slate-300">Or choose a local file</label>
          <input
            type="file"
            accept="image/*,video/*"
            className="w-full rounded-lg border border-white/10 bg-white/[0.05] px-3 py-2 text-sm text-slate-200"
            onChange={(event) => onImportFileChange(event.target.files?.[0] ?? null)}
          />
        </div>
        <div className="mt-4 flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="h-10 rounded-lg border border-white/10 bg-white/[0.04] px-4 text-sm text-slate-300 transition hover:bg-white/10 hover:text-white"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={onImport}
            disabled={busy}
            className="h-10 rounded-lg bg-cyan-300 px-4 text-sm font-semibold text-slate-950 transition hover:bg-cyan-200 disabled:cursor-not-allowed disabled:opacity-50"
          >
            Import
          </button>
        </div>
      </div>
    </div>
  );
}

function EmptyState({ text }: { text: string }) {
  return (
    <div className="col-span-full flex min-h-44 items-center justify-center rounded-lg border border-dashed border-white/15 bg-white/[0.035] p-6 text-center text-sm text-slate-400">
      {text}
    </div>
  );
}

function kindSurface(kind: Wallpaper["kind"]) {
  if (kind === "video") {
    return "bg-gradient-to-br from-fuchsia-500 via-slate-900 to-rose-500";
  }
  if (kind === "animated") {
    return "bg-gradient-to-br from-emerald-400 via-slate-900 to-cyan-400";
  }
  return "bg-gradient-to-br from-cyan-400 via-slate-900 to-blue-500";
}

function buildActivity(
  status: BackendStatus | null,
  wallpapers: Wallpaper[],
  playlists: Playlist[],
  message: string
) {
  return [
    message,
    `${wallpapers.length} wallpapers indexed`,
    `${playlists.length} playlists available`,
    status?.activeWallpaper
      ? `${status.activeWallpaper.title} is active`
      : "No active wallpaper selected",
  ];
}

function downloadActivity(activity: string[]) {
  const blob = new Blob([JSON.stringify({ activity }, null, 2)], {
    type: "application/json",
  });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = "lumina-activity.json";
  anchor.click();
  URL.revokeObjectURL(url);
}

                <div className="mt-3">
                  <label className="mb-2 block text-sm text-slate-300">Or choose a local file</label>
                  <input
                    type="file"
                    accept="image/*,video/*"
                    className="w-full rounded-lg border border-white/10 bg-white/[0.05] px-3 py-2 text-sm text-slate-200"
                    onChange={(event) => onImportFileChange(event.target.files?.[0] ?? null)}
                  />
                </div>
function formatBytes(bytes: number) {
  if (!bytes) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unitIndex = 0;

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  return `${value.toFixed(value >= 10 || unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`;
}

function formatDuration(seconds: number) {
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  const rest = seconds % 60;
  return `${minutes}m ${rest}s`;
}
