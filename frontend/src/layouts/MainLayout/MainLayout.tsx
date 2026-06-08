import { Outlet } from "react-router-dom";
import Sidebar from "../../components/common/Sidebar/Sidebar";

export default function MainLayout() {
  return (
    <div className="flex min-h-screen">
      <Sidebar />

      <main className="flex-1 bg-zinc-950">
        <Outlet />
      </main>
    </div>
  );
}