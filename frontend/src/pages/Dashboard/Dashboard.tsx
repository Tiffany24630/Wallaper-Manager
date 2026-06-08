import { useEffect } from "react";
import StatCard from "../../components/common/StatCard/StatCard";
import { useSystemStore } from "../../store/systemStore";
import {formatBytes, formatPercent, formatUptime,} from "../../utils/format";

export default function Dashboard() {
  const {
    systemInfo,
    loading,
    refresh,
  } = useSystemStore();

  useEffect(() => {
    refresh();

    const interval = setInterval(
      refresh,
      5000
    );

    return () => clearInterval(interval);
  }, []);

  if (loading || !systemInfo) {
    return (
      <div className="p-6">
        Cargando información...
      </div>
    );
  }

  return (
    <div className="p-6 space-y-8">
      <div>
        <h1 className="text-3xl font-bold">
          Dashboard
        </h1>

        <p className="text-zinc-500 mt-2">
          Estado actual del sistema
        </p>
      </div>

      <div
        className="
          grid
          grid-cols-1
          md:grid-cols-2
          xl:grid-cols-4
          gap-4
        "
      >
        <StatCard
          title="CPU"
          value={formatPercent(
            systemInfo.cpuUsage
          )}
        />

        <StatCard
          title="RAM"
          value={formatPercent(
            (systemInfo.usedMemory /
              systemInfo.totalMemory) *
              100
          )}
          subtitle={`${formatBytes(
            systemInfo.usedMemory
          )} / ${formatBytes(
            systemInfo.totalMemory
          )}`}
        />

        <StatCard
          title="Almacenamiento"
          value={formatPercent(
            (systemInfo.usedStorage /
              systemInfo.totalStorage) *
              100
          )}
          subtitle={`${formatBytes(
            systemInfo.usedStorage
          )} / ${formatBytes(
            systemInfo.totalStorage
          )}`}
        />

        <StatCard
          title="Uptime"
          value={formatUptime(
            systemInfo.uptime
          )}
        />
      </div>

      <div
        className="
          rounded-xl
          border
          border-zinc-800
          bg-zinc-900
          p-6
        "
      >
        <h2 className="text-xl font-semibold">
          Sistema
        </h2>

        <div className="mt-4 space-y-2">
          <p>
            <strong>Host:</strong>{" "}
            {systemInfo.hostname}
          </p>

          <p>
            <strong>Sistema:</strong>{" "}
            {systemInfo.osName}
          </p>

          <p>
            <strong>Wallpapers:</strong>{" "}
            {systemInfo.wallpaperCount}
          </p>

          <p>
            <strong>Activo:</strong>{" "}
            {systemInfo.activeWallpaper ??
              "Ninguno"}
          </p>
        </div>
      </div>
    </div>
  );
}