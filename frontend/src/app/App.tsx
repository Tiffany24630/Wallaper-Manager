import Sidebar from "./components/Sidebar";
import Dashboard from "./components/Dashboard";

export default function App() {
  return (
    <div className="h-screen w-screen overflow-hidden bg-[#140F2D] text-white">
      <div className="flex h-full">
        <Sidebar />

        <main className="flex-1 overflow-y-auto">
          <div className="p-8">
            <Dashboard />
          </div>
        </main>
      </div>
    </div>
  );
}