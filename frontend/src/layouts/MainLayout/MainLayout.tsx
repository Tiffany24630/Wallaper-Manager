import { Outlet } from "react-router-dom";
import { useEffect } from "react";
import Sidebar from "../../components/common/Sidebar/Sidebar";
import { useSettingsStore } from "../../store/settingsStore";

export default function MainLayout() {
  const { settings, loadSettings } = useSettingsStore();
  useEffect(() => { void loadSettings(); }, [loadSettings]);
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
