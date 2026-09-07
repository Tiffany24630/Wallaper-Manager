import { convertFileSrc } from "@tauri-apps/api/core";
import { Sparkles, WandSparkles } from "lucide-react";
import { useEffect, useState } from "react";
import { useWallpaperStore } from "../../store/wallpaperStore";
import UpscaleComparisonDialog from "../../components/wallpaper/UpscaleComparisonDialog";

export default function AIUpscaling() {
  const store = useWallpaperStore();
  const { wallpapers, loadWallpapers, upscaleWallpaper } = store;
  const [selected, setSelected] = useState("");
  const [scale, setScale] = useState(4);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [comparison, setComparison] = useState<{ originalId: string; enhancedId: string } | null>(null);
  useEffect(() => { void loadWallpapers(); }, [loadWallpapers]);
  useEffect(() => { if (!selected && wallpapers[0]) setSelected(wallpapers[0].id); }, [wallpapers, selected]);
  const current = wallpapers.find(w => w.id === selected);
  async function enhance() { if (!selected) return; setBusy(true); setMessage(null); try { const enhancedId = await upscaleWallpaper(selected, scale); setComparison({ originalId: selected, enhancedId }); } catch (e) { setMessage(`Error: ${String(e)}`); } finally { setBusy(false); } }
  async function chooseVersion(choice: "original" | "enhanced") { if (!comparison) return; setBusy(true); try { const original = wallpapers.find(w => w.id === comparison.originalId); if (choice === "enhanced" && original?.active) await store.setActive(comparison.enhancedId); await store.deleteWallpaper(choice === "original" ? comparison.enhancedId : comparison.originalId); setSelected(choice === "original" ? comparison.originalId : comparison.enhancedId); setComparison(null); setMessage(choice === "original" ? "Se conservó la versión original." : "Se conservó la versión mejorada."); } catch (e) { setMessage(`Error: ${String(e)}`); } finally { setBusy(false); } }
  const original = comparison && wallpapers.find(w => w.id === comparison.originalId);
  const enhanced = comparison && wallpapers.find(w => w.id === comparison.enhancedId);

  return <div className="page">
    <header><div className="eyebrow">Procesamiento local</div><h1 className="page-title">Mejorar calidad</h1><p className="page-subtitle">Real-ESRGAN usa la GPU mediante Vulkan; si no está disponible, Lumina aplica escalado Lanczos de alta calidad. El proceso puede tardar varios minutos.</p></header>
    {message && <div className={message.startsWith("Error:") ? "status-error" : "status-success"}>{message}</div>}
    {wallpapers.length === 0 ? <div className="empty"><Sparkles className="mx-auto mb-3"/>Importa una imagen en la biblioteca para poder mejorarla.</div> : <section className="grid gap-5 lg:grid-cols-[1.25fr_.75fr]">
      <div className="panel overflow-hidden"><div className="aspect-video bg-black/25">{current?.thumbnail && <img src={convertFileSrc(current.thumbnail)} alt={current.name} className="h-full w-full object-contain"/>}</div><div className="p-4"><div className="truncate font-semibold">{current?.name}</div><div className="mt-1 text-sm muted">{current?.width && current?.height ? `${current.width} × ${current.height} → ${current.width * scale} × ${current.height * scale}` : "Se calculará la resolución al procesar"}</div></div></div>
      <div className="panel panel-pad self-start"><div className="flex items-center gap-2"><WandSparkles className="text-violet-300"/><h2 className="text-lg font-semibold">Opciones</h2></div><label className="mt-6 block text-sm muted">Imagen</label><select className="input mt-2" value={selected} onChange={e => setSelected(e.target.value)}>{wallpapers.map(w => <option key={w.id} value={w.id}>{w.name}</option>)}</select><label className="mt-5 block text-sm muted">Escala</label><div className="mt-2 grid grid-cols-2 gap-2">{[3,4].map(value => <button key={value} onClick={() => setScale(value)} className={`btn ${scale === value ? "btn-primary" : ""}`}>{value}×</button>)}</div><p className="mt-5 text-xs leading-5 muted">4× usa el modelo general Real-ESRGAN. 3× usa el modelo optimizado para ilustración/anime. Después podrás comparar y elegir una sola versión para conservar.</p><button className="btn btn-primary mt-6 w-full" onClick={() => void enhance()} disabled={busy}>{busy ? "Mejorando imagen…" : `Crear versión ${scale}×`}</button></div>
    </section>}
    {original && enhanced && <UpscaleComparisonDialog original={original} enhanced={enhanced} busy={busy} onChoose={choice => void chooseVersion(choice)}/>}
  </div>;
}
