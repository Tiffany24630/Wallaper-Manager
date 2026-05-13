import { Routes, Route } from "react-router-dom";

import HomePage from "../pages/Home/Home";
import LibraryPage from "../pages/Library/Library";
import SettingsPage from "../pages/Settings/Settings";
import PlaylistsPage from "../pages/Playlists/Playlists";
import PerformancePage from "../pages/Performance/Performance";

export function AppRouter() {
  return (
    <Routes>
      <Route path="/" element={<HomePage />} />
      <Route path="/library" element={<LibraryPage />} />
      <Route path="/playlists" element={<PlaylistsPage />} />
      <Route path="/settings" element={<SettingsPage />} />
      <Route path="/performance" element={<PerformancePage />} />
    </Routes>
  );
}