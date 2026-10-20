import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { animatedPrototypeService } from "../../services/animatedPrototypeService";

export default function AnimatedPrototype() {
  const [params] = useSearchParams();
  const [failed, setFailed] = useState(false);
  const [status, setStatus] = useState("Preparando medio local…");
  const reportedError = useRef(false);
  const imageRef = useRef<HTMLImageElement>(null);
  const mediaPath = params.get("media");
  const surfaceId = params.get("surface") ?? "animated-prototype-unknown";
  const mediaErrorTest = params.get("mediaErrorTest") === "1";
  const mediaUrl = useMemo(
    () => (mediaPath ? convertFileSrc(mediaPath, "asset") : null),
    [mediaPath],
  );
  const isGif = mediaPath?.toLowerCase().endsWith(".gif") ?? false;

  const reportMediaError = (reason: string) => {
    if (reportedError.current) return;
    reportedError.current = true;
    setStatus(`Error de media: ${reason}`);
    setFailed(true);
    void animatedPrototypeService
      .reportMediaError(surfaceId, "media", reason)
      .catch((error) => console.error("No se pudo informar el error de media a Rust", error));
  };

  useEffect(() => {
    if (!mediaErrorTest || !isGif || !mediaUrl) return;
    const timer = window.setTimeout(() => {
      if (imageRef.current && !reportedError.current) {
        setStatus("Provocando error de media controlado…");
        imageRef.current.src = "lumina-diagnostic-invalid://media.gif";
      }
    }, 350);
    return () => window.clearTimeout(timer);
  }, [isGif, mediaErrorTest, mediaUrl]);

  if (!mediaUrl || failed) {
    return (
      <main style={{ width: "100vw", height: "100vh", display: "grid", placeItems: "center", background: "#000", color: "#fca5a5", fontFamily: "sans-serif" }}>
        No se pudo cargar el medio local del prototipo.
      </main>
    );
  }

  return (
    <main style={{ width: "100vw", height: "100vh", position: "relative", overflow: "hidden", background: "#000", pointerEvents: "none" }}>
      {isGif ? (
        <img
          ref={imageRef}
          src={mediaUrl}
          alt=""
          onLoad={() => setStatus("GIF cargado; reproducción animada disponible")}
          onError={() => reportMediaError("img_error")}
          style={{ width: "100%", height: "100%", objectFit: "cover", display: "block" }}
        />
      ) : (
        <video
          src={mediaUrl}
          autoPlay
          loop
          muted
          playsInline
          controls={false}
          onLoadedData={() => setStatus("MP4 cargado; esperando reproducción")}
          onCanPlay={() => setStatus("MP4 listo para reproducir")}
          onPlay={() => setStatus("MP4 reproduciéndose en loop y sin audio")}
          onError={() => reportMediaError("video_error")}
          style={{ width: "100%", height: "100%", objectFit: "cover", display: "block" }}
        />
      )}
      <div style={{ position: "absolute", top: 12, left: 12, padding: "6px 10px", borderRadius: 8, background: "rgba(0,0,0,.72)", color: "#dbeafe", font: "12px sans-serif" }}>
        {status}
      </div>
    </main>
  );
}
