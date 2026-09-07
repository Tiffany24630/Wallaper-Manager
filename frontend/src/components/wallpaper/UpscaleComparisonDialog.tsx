import { convertFileSrc } from "@tauri-apps/api/core";
import { Check, Images } from "lucide-react";
import type { Wallpaper } from "../../types/wallpaper";

interface Props {
  original: Wallpaper;
  enhanced: Wallpaper;
  busy?: boolean;
  onChoose: (choice: "original" | "enhanced") => void;
}

function Preview({ wallpaper, label }: { wallpaper: Wallpaper; label: string }) {
  return <article className="comparison-option">
    <div className="comparison-image">
      <img src={convertFileSrc(wallpaper.path)} alt={`${label}: ${wallpaper.name}`} />
    </div>
    <div className="p-4">
      <div className="font-semibold">{label}</div>
      <div className="mt-1 text-sm muted">{wallpaper.width} × {wallpaper.height}</div>
    </div>
  </article>;
}

export default function UpscaleComparisonDialog({ original, enhanced, busy, onChoose }: Props) {
  return <div className="modal-backdrop" role="dialog" aria-modal="true" aria-labelledby="comparison-title">
    <div className="modal-card">
      <div className="flex items-start gap-3">
        <Images className="mt-1 shrink-0 text-violet-300" />
        <div><h2 id="comparison-title" className="text-xl font-semibold">Elige la versión que quieres guardar</h2><p className="mt-1 text-sm muted">Amplía ambas vistas para comparar detalles. La versión descartada se eliminará de la biblioteca.</p></div>
      </div>
      <div className="mt-5 grid gap-4 md:grid-cols-2">
        <div><Preview wallpaper={original} label="Original"/><button disabled={busy} className="btn mt-3 w-full" onClick={() => onChoose("original")}><Check size={16}/> Guardar original</button></div>
        <div><Preview wallpaper={enhanced} label="Mejorada con IA"/><button disabled={busy} className="btn btn-primary mt-3 w-full" onClick={() => onChoose("enhanced")}><Check size={16}/> Guardar mejorada</button></div>
      </div>
    </div>
  </div>;
}
