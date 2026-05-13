import MainLayout from "../../layouts/MainLayout/MainLayout";

export default function PerformancePage() {
  return (
    <MainLayout>
      <h1 className="text-3xl font-bold mb-6">
        Performance
      </h1>

      <div className="grid grid-cols-3 gap-6">
        <div className="bg-zinc-800 p-6 rounded-2xl">
          CPU Usage
        </div>

        <div className="bg-zinc-800 p-6 rounded-2xl">
          GPU Usage
        </div>

        <div className="bg-zinc-800 p-6 rounded-2xl">
          RAM Usage
        </div>
      </div>
    </MainLayout>
  );
}