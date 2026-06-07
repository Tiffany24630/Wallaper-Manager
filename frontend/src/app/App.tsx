import { useCallback, useEffect, useMemo, useState } from "react";
import Sidebar from "./components/Sidebar";
import Dashboard from "./components/Dashboard";
import {
  addWallpaperToPlaylist,
  createPlaylist,
  defaultSettings,
  deletePlaylist,
  getBackendSnapshot,
  importWallpaper,
  isTauriRuntime,
  saveSettings,
  setActiveWallpaper,
  toggleFavorite,
  type AppSettings,
  type BackendStatus,
  type Playlist,
  type Wallpaper,
  type WallpaperKind,
} from "../services/wallpaperService";

export type AppView = "dashboard" | "library" | "playlists" | "performance" | "settings";
export type WallpaperFilter = "all" | "favorites" | WallpaperKind;

export default function App() {
  const [activeView, setActiveView] = useState<AppView>("dashboard");
  const [status, setStatus] = useState<BackendStatus | null>(null);
  const [wallpapers, setWallpapers] = useState<Wallpaper[]>([]);
  const [playlists, setPlaylists] = useState<Playlist[]>([]);
  const [settings, setSettings] = useState<AppSettings>(defaultSettings);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<WallpaperFilter>("all");
  const [message, setMessage] = useState("Starting Lumina...");
  const [busy, setBusy] = useState(false);
  const [importOpen, setImportOpen] = useState(false);
  const [importPath, setImportPath] = useState("");
  const [importFile, setImportFile] = useState<File | null>(null);
  const [playlistName, setPlaylistName] = useState("");
  const [selectedPlaylistId, setSelectedPlaylistId] = useState<string | null>(null);
  const [showActivity, setShowActivity] = useState(true);

  const refresh = useCallback(async () => {
    setBusy(true);
    try {
      const snapshot = await getBackendSnapshot();
      setStatus(snapshot.status);
      setWallpapers(snapshot.wallpapers);
      setPlaylists(snapshot.playlists);
      setSettings(snapshot.settings);
      setSelectedPlaylistId((current) => current ?? snapshot.playlists[0]?.id ?? null);
      setMessage("Backend synced");
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const filteredWallpapers = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase();

    return wallpapers.filter((wallpaper) => {
      const matchesQuery =
        !normalizedQuery ||
        wallpaper.title.toLowerCase().includes(normalizedQuery) ||
        wallpaper.relativePath.toLowerCase().includes(normalizedQuery);
      const matchesFilter =
        filter === "all" ||
        (filter === "favorites" && wallpaper.favorite) ||
        wallpaper.kind === filter;

      return matchesQuery && matchesFilter;
    });
  }, [filter, query, wallpapers]);

  const selectedPlaylist = useMemo(
    () => playlists.find((playlist) => playlist.id === selectedPlaylistId) ?? null,
    [playlists, selectedPlaylistId]
  );

  const selectedPlaylistWallpapers = useMemo(() => {
    if (!selectedPlaylist) return [];
    return wallpapers.filter((wallpaper) =>
      selectedPlaylist.wallpaperIds.includes(wallpaper.id)
    );
  }, [selectedPlaylist, wallpapers]);

  async function handleImport() {
    if (isTauriRuntime()) {
      if (!importPath.trim()) {
        setMessage("Pega una ruta local antes de importar.");
        return;
      }
    } else {
      if (!importFile) {
        setMessage("Selecciona un archivo para importar.");
        return;
      }
    }

    setBusy(true);
    try {
      const wallpaper = isTauriRuntime()
        ? await importWallpaper(importPath)
        : await importWallpaper(importFile as File);
      setMessage(`${wallpaper.title} imported`);
      setImportPath("");
      setImportFile(null);
      setImportOpen(false);
      await refresh();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleSetActive(wallpaperId: string) {
    setBusy(true);
    try {
      const result = await setActiveWallpaper(wallpaperId);
      setMessage(result.message);
      await refresh();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleToggleFavorite(wallpaperId: string) {
    try {
      const wallpaper = await toggleFavorite(wallpaperId);
      setMessage(wallpaper.favorite ? "Saved as favorite" : "Removed from favorites");
      await refresh();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }

  async function handleCreatePlaylist() {
    if (!playlistName.trim()) {
      setMessage("Escribe un nombre para la playlist.");
      return;
    }

    setBusy(true);
    try {
      const playlist = await createPlaylist(playlistName);
      setPlaylistName("");
      setMessage(`${playlist.name} created`);
      await refresh();
      setSelectedPlaylistId(playlist.id);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleDeletePlaylist(playlistId: string) {
    setBusy(true);
    try {
      const nextPlaylists = await deletePlaylist(playlistId);
      setPlaylists(nextPlaylists);
      setSelectedPlaylistId(nextPlaylists[0]?.id ?? null);
      setMessage("Playlist deleted");
      await refresh();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function handleAddToPlaylist(playlistId: string, wallpaperId: string) {
    try {
      const playlist = await addWallpaperToPlaylist(playlistId, wallpaperId);
      setMessage(`Added to ${playlist.name}`);
      await refresh();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }

  async function handleSettingsChange(nextSettings: AppSettings) {
    setSettings(nextSettings);
    try {
      const savedSettings = await saveSettings(nextSettings);
      setSettings(savedSettings);
      setMessage("Settings saved");
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }

  function cycleFilter() {
    const filters: WallpaperFilter[] = ["all", "favorites", "image", "video", "animated"];
    const nextFilter = filters[(filters.indexOf(filter) + 1) % filters.length];
    setFilter(nextFilter);
    setMessage(`Filter: ${nextFilter}`);
  }

  return (
    <div className="min-h-screen bg-ink-950 text-slate-100 antialiased">
      <div className="flex min-h-screen flex-col lg:h-screen lg:flex-row lg:overflow-hidden">
        <Sidebar activeView={activeView} status={status} onViewChange={setActiveView} />

        <main className="min-h-0 flex-1 overflow-y-auto">
          <Dashboard
            activeView={activeView}
            backendAvailable={isTauriRuntime()}
            busy={busy}
            filter={filter}
            filteredWallpapers={filteredWallpapers}
            importOpen={importOpen}
            importPath={importPath}
            message={message}
            playlistName={playlistName}
            playlists={playlists}
            query={query}
            selectedPlaylist={selectedPlaylist}
            selectedPlaylistWallpapers={selectedPlaylistWallpapers}
            settings={settings}
            showActivity={showActivity}
            status={status}
            wallpapers={wallpapers}
            onAddToPlaylist={handleAddToPlaylist}
            onCreatePlaylist={handleCreatePlaylist}
            onCycleFilter={cycleFilter}
            onDeletePlaylist={handleDeletePlaylist}
            onFilterChange={setFilter}
            onImport={handleImport}
            onImportOpenChange={setImportOpen}
            onImportPathChange={setImportPath}
            onImportFileChange={setImportFile}
            onPlaylistNameChange={setPlaylistName}
            onQueryChange={setQuery}
            onRefresh={refresh}
            onSelectedPlaylistChange={setSelectedPlaylistId}
            onSetActive={handleSetActive}
            onSettingsChange={handleSettingsChange}
            onToggleActivity={() => setShowActivity((value) => !value)}
            onToggleFavorite={handleToggleFavorite}
            onViewChange={setActiveView}
          />
        </main>
      </div>
    </div>
  );
}
