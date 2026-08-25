import { Cpu, Database, HardDrive, Info } from "lucide-react";
import { useEffect, useState } from "react";
import { useSystemStore } from "../../store/systemStore";
import { formatBytes, formatPercent } from "../../utils/format";

type Sample = { cpu: number; memory: number };
const percent = (used: number, total: number) => total ? Math.min(100, used / total * 100) : 0;

function Chart({ samples, field, color }: { samples: Sample[]; field: keyof Sample; color: string }) {
  const points = samples.map((sample, index) => `${samples.length <= 1 ? 0 : index / (samples.length - 1) * 100},${100 - sample[field]}`).join(" ");
  return <svg viewBox="0 0 100 100" preserveAspectRatio="none" className="h-32 w-full overflow-visible"><path d="M0 25H100M0 50H100M0 75H100" stroke="rgba(255,255,255,.06)" strokeWidth=".5"/><polyline points={points} fill="none" stroke={color} strokeWidth="2" vectorEffect="non-scaling-stroke" strokeLinejoin="round"/></svg>;
}

export default function PerformancePage() {
  const { systemInfo, error, startPolling, stopPolling } = useSystemStore();
  const [samples, setSamples] = useState<Sample[]>([]);
  useEffect(() => { startPolling(); return stopPolling; }, [startPolling, stopPolling]);
  useEffect(() => { if (!systemInfo) return; setSamples(old => [...old, { cpu: systemInfo.cpuUsage, memory: percent(systemInfo.usedMemory, systemInfo.totalMemory) }].slice(-30)); }, [systemInfo]);
  const memory = systemInfo ? percent(systemInfo.usedMemory, systemInfo.totalMemory) : 0;
  const storage = systemInfo ? percent(systemInfo.usedStorage, systemInfo.totalStorage) : 0;

  return <div className="page">
    <header><div className="eyebrow">Telemetría local</div><h1 className="page-title">Rendimiento</h1><p className="page-subtitle">Muestras actualizadas cada cinco segundos; no se envía información fuera del equipo.</p></header>
    {error && <div className="status-error">{error}</div>}
    <section className="metric-grid">
      <div className="panel metric"><Cpu size={18} className="text-violet-300"/><div className="metric-label mt-3">CPU total</div><div className="metric-value">{formatPercent(systemInfo?.cpuUsage ?? 0)}</div></div>
      <div className="panel metric"><Database size={18} className="text-cyan-300"/><div className="metric-label mt-3">RAM total</div><div className="metric-value">{formatPercent(memory)}</div></div>
      <div className="panel metric"><HardDrive size={18} className="text-emerald-300"/><div className="metric-label mt-3">Discos</div><div className="metric-value">{formatPercent(storage)}</div></div>
      <div className="panel metric"><Info size={18} className="text-amber-300"/><div className="metric-label mt-3">Uso de Lumina</div><div className="metric-value">{formatBytes(systemInfo?.appMemory ?? 0)}</div></div>
    </section>
    <section className="grid gap-5 lg:grid-cols-2">
      <div className="panel panel-pad"><div className="mb-5 flex items-end justify-between"><div><h2 className="font-semibold">Historial de CPU</h2><p className="text-xs muted">Últimas {samples.length} muestras</p></div><strong>{formatPercent(systemInfo?.cpuUsage ?? 0)}</strong></div><Chart samples={samples} field="cpu" color="#8b7cff"/></div>
      <div className="panel panel-pad"><div className="mb-5 flex items-end justify-between"><div><h2 className="font-semibold">Historial de memoria</h2><p className="text-xs muted">{systemInfo ? `${formatBytes(systemInfo.usedMemory)} / ${formatBytes(systemInfo.totalMemory)}` : "Leyendo…"}</p></div><strong>{formatPercent(memory)}</strong></div><Chart samples={samples} field="memory" color="#22bec6"/></div>
    </section>
    <div className="panel panel-pad text-sm muted"><strong className="text-slate-200">Sobre la GPU:</strong> Windows y `sysinfo` no exponen una métrica GPU consistente en esta implementación. Lumina no muestra porcentajes inventados; Real-ESRGAN informará un error si Vulkan no está disponible.</div>
  </div>;
}
