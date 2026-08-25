import { Outlet } from "react-router-dom";
import Sidebar from "../../components/common/Sidebar/Sidebar";

export default function MainLayout() {
  return (
    <div className="app-shell">
      <Sidebar />

      <main className="app-content">
        <Outlet />
      </main>
    </div>
  );
}
