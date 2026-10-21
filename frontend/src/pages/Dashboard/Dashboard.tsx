import { Activity, Clock3, Database, Image, MonitorUp, RefreshCw } from "lucide-react";
import { useEffect } from "react";
import { useSystemStore } from "../../store/systemStore";
import { formatBytes, formatPercent, formatUptime } from "../../utils/format";
import { animatedPrototypeService } from "../../services/animatedPrototypeService";
import { monitorService } from "../../services/monitorService";
import { useState } from "react";

function safePercent(used: number, total: number) { return total > 0 ? Math.min(100, (used / total) * 100) : 0; }

export default function Dashboard() {
  const { systemInfo, loading, error, refresh, startPolling, stopPolling } = useSystemStore();
  const [prototypeBusy, setPrototypeBusy] = useState(false);
  const [prototypeMessage, setPrototypeMessage] = useState<string | null>(null);
  const runPrototypeTest = () => { setPrototypeBusy(true); setPrototypeMessage(null); void monitorService.getAll().then(monitors => { const monitor = monitors.find(item => item.primary) ?? monitors[0]; if (!monitor) throw new Error("No se detectaron monitores"); return animatedPrototypeService.openGeneratedTest(monitor.id); }).then(() => setPrototypeMessage("GIF generado y ventana WebView2 abierta.")).catch(error => setPrototypeMessage(String(error))).finally(() => setPrototypeBusy(false)); };
  useEffect(() => { startPolling(); return stopPolling; }, [startPolling, stopPolling]);
  const memory = systemInfo ? safePercent(systemInfo.usedMemory, systemInfo.totalMemory) : 0;
  const storage = systemInfo ? safePercent(systemInfo.usedStorage, systemInfo.totalStorage) : 0;

  return <div className="page">
    <header className="page-header">
      <div><div className="eyebrow">Resumen del dispositivo</div><h1 className="page-title">Tu escritorio, bajo control</h1><p className="page-subtitle">Datos medidos localmente y estado actual de la biblioteca.</p></div>
      <div className="flex flex-wrap gap-3"><button className="btn btn-secondary" onClick={runPrototypeTest} disabled={prototypeBusy}>Probar GIF WebView2</button><button className="btn" onClick={refresh} disabled={loading}><RefreshCw size={16} className={loading ? "animate-spin" : ""} /> Actualizar</button></div>
    </header>
    {error && <div className="status-error">No se pudo leer el sistema: {error}</div>}
    {prototypeMessage && <div className="status-success">{prototypeMessage}</div>}

    <section className="metric-grid">
      <div className="panel metric"><div className="metric-label flex items-center gap-2"><Activity size={15}/> CPU</div><div className="metric-value">{systemInfo ? formatPercent(systemInfo.cpuUsage) : "—"}</div><div className="mt-3 progress"><span style={{width:`${systemInfo?.cpuUsage ?? 0}%`}} /></div></div>
      <div className="panel metric"><div className="metric-label flex items-center gap-2"><Database size={15}/> Memoria</div><div className="metric-value">{formatPercent(memory)}</div><div className="mt-1 text-xs muted">{systemInfo ? `${formatBytes(systemInfo.usedMemory)} de ${formatBytes(systemInfo.totalMemory)}` : "Leyendo…"}</div></div>
      <div className="panel metric"><div className="metric-label flex items-center gap-2"><MonitorUp size={15}/> Almacenamiento</div><div className="metric-value">{formatPercent(storage)}</div><div className="mt-1 text-xs muted">{systemInfo ? `${formatBytes(systemInfo.usedStorage)} de ${formatBytes(systemInfo.totalStorage)}` : "Leyendo…"}</div></div>
      <div className="panel metric"><div className="metric-label flex items-center gap-2"><Clock3 size={15}/> Tiempo encendido</div><div className="metric-value">{systemInfo ? formatUptime(systemInfo.uptime) : "—"}</div></div>
    </section>
    <section className="panel panel-pad"><div className="flex flex-wrap items-center justify-between gap-3"><div><h2 className="text-lg font-semibold">Prueba técnica WebView2</h2><p className="mt-1 text-sm muted">Genera un GIF local de dos fotogramas y abre el reproductor aislado.</p></div><button className="btn btn-secondary" onClick={runPrototypeTest} disabled={prototypeBusy}>{prototypeBusy ? "Generando…" : "Abrir GIF autónomo"}</button></div></section>

    <section className="grid gap-4 lg:grid-cols-[1.4fr_1fr]">
      <div className="panel panel-pad">
        <h2 className="text-lg font-semibold">Dispositivo</h2>
        <dl className="mt-5 grid gap-4 sm:grid-cols-2">
          <div><dt className="text-xs muted">Equipo</dt><dd className="mt-1">{systemInfo?.hostname || "—"}</dd></div>
          <div><dt className="text-xs muted">Sistema operativo</dt><dd className="mt-1">{systemInfo?.osName || "—"}</dd></div>
          <div><dt className="text-xs muted">Procesador</dt><dd className="mt-1">{systemInfo?.cpuName || "—"}</dd></div>
          <div><dt className="text-xs muted">Procesadores lógicos</dt><dd className="mt-1">{systemInfo?.logicalCores ?? "—"}</dd></div>
          <div><dt className="text-xs muted">Memoria de Lumina</dt><dd className="mt-1">{systemInfo ? formatBytes(systemInfo.appMemory) : "—"}</dd></div>
        </dl>
      </div>
      <div className="panel panel-pad">
        <div className="flex items-center gap-2"><Image size={18} className="text-violet-300"/><h2 className="text-lg font-semibold">Biblioteca</h2></div>
        <div className="mt-5 text-4xl font-bold">{systemInfo?.wallpaperCount ?? 0}</div><div className="muted text-sm">fondos guardados</div>
        <div className="mt-5 border-t border-white/10 pt-4"><div className="text-xs muted">Fondo activo</div><div className="mt-1 truncate font-medium">{systemInfo?.activeWallpaper || "Ninguno"}</div></div>
      </div>
    </section>
  </div>;
}
