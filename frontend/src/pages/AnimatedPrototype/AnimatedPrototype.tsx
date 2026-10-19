import { convertFileSrc } from "@tauri-apps/api/core";
import { useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";

export default function AnimatedPrototype() {
  const [params] = useSearchParams();
  const [failed, setFailed] = useState(false);
  const [status, setStatus] = useState("Preparando medio local…");
  const mediaPath = params.get("media");
  const mediaUrl = useMemo(() => mediaPath ? convertFileSrc(mediaPath, "asset") : null, [mediaPath]);
  const isGif = mediaPath?.toLowerCase().endsWith(".gif") ?? false;

  if (!mediaUrl || failed) {
    return <main style={{ width: "100vw", height: "100vh", display: "grid", placeItems: "center", background: "#000", color: "#fca5a5", fontFamily: "sans-serif" }}>No se pudo cargar el medio local del prototipo.</main>;
  }

  return <main style={{ width: "100vw", height: "100vh", position: "relative", overflow: "hidden", background: "#000", pointerEvents: "none" }}>
    {isGif ? <img src={mediaUrl} alt="" onLoad={() => setStatus("GIF cargado; reproducción animada disponible")} onError={() => { setStatus("Error al leer el GIF local"); setFailed(true); }} style={{ width: "100%", height: "100%", objectFit: "cover", display: "block" }} /> : <video src={mediaUrl} autoPlay loop muted playsInline controls={false} onLoadedData={() => setStatus("MP4 cargado; esperando reproducción")} onCanPlay={() => setStatus("MP4 listo para reproducir")} onPlay={() => setStatus("MP4 reproduciéndose en loop y sin audio")} onError={() => { setStatus("Error al leer o reproducir el MP4 local"); setFailed(true); }} style={{ width: "100%", height: "100%", objectFit: "cover", display: "block" }} />}
    <div style={{ position: "absolute", top: 12, left: 12, padding: "6px 10px", borderRadius: 8, background: "rgba(0,0,0,.72)", color: "#dbeafe", font: "12px sans-serif" }}>{status}</div>
  </main>;
}
