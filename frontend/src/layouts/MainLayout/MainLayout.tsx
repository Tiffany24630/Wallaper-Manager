import { Outlet } from "react-router-dom";
import { useEffect } from "react";
import Sidebar from "../../components/common/Sidebar/Sidebar";
import { useSettingsStore } from "../../store/settingsStore";
import { syncCoordinator } from "../../services/syncCoordinator";
import { accountService } from "../../services/accountService";

export default function MainLayout() {
  const { settings, loadSettings } = useSettingsStore();
  useEffect(() => { void loadSettings(); }, [loadSettings]);
  useEffect(() => { void accountService.initialize().finally(() => syncCoordinator.start()); return () => syncCoordinator.stop(); }, []);
  useEffect(() => {
    const root = document.documentElement;
    root.dataset.accent = settings?.accent_color ?? "violet";
    root.style.fontSize = `${settings?.ui_scale ?? 100}%`;
  }, [settings]);

  return (
    <div className="app-shell">
      <Sidebar />

      <main className="app-content">
        <Outlet />
      </main>
    </div>
  );
}
