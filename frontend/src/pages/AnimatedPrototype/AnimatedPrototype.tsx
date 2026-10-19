import { convertFileSrc } from "@tauri-apps/api/core";
import { useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";

export default function AnimatedPrototype() {
  const [params] = useSearchParams();
  const [failed, setFailed] = useState(false);
  const mediaPath = params.get("media");
  const mediaUrl = useMemo(() => mediaPath ? convertFileSrc(mediaPath, "asset") : null, [mediaPath]);
  const isGif = mediaPath?.toLowerCase().endsWith(".gif") ?? false;

  if (!mediaUrl || failed) {
    return <main style={{ width: "100vw", height: "100vh", display: "grid", placeItems: "center", background: "#000", color: "#fca5a5", fontFamily: "sans-serif" }}>No se pudo cargar el medio local del prototipo.</main>;
  }

  return <main style={{ width: "100vw", height: "100vh", overflow: "hidden", background: "#000", pointerEvents: "none" }}>
    {isGif ? <img src={mediaUrl} alt="" onError={() => setFailed(true)} style={{ width: "100%", height: "100%", objectFit: "cover", display: "block" }} /> : <video src={mediaUrl} autoPlay loop muted playsInline controls={false} onError={() => setFailed(true)} style={{ width: "100%", height: "100%", objectFit: "cover", display: "block" }} />}
  </main>;
}
