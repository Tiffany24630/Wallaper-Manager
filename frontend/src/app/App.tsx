import Sidebar from "./components/Sidebar";
import Dashboard from "./components/Dashboard";

export default function App() {
  return (
    <div className="min-h-screen bg-ink-950 text-slate-100 antialiased">
      <div className="flex min-h-screen flex-col lg:h-screen lg:flex-row lg:overflow-hidden">
        <Sidebar />

        <main className="min-h-0 flex-1 overflow-y-auto">
          <Dashboard />
        </main>
      </div>
    </div>
  );
}
