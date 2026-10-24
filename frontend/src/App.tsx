import { RouterProvider } from "react-router-dom";
import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { router } from "./routes";
import { useWallpaperStore } from "./store/wallpaperStore";

export default function App() {
  const initializeActivationEvents = useWallpaperStore(state => state.initializeActivationEvents);
  useEffect(() => {
    const isSmoke = (window as Window & { __LUMINA_SMOKE__?: boolean }).__LUMINA_SMOKE__ === true;
    if (isSmoke) {
      void invoke("record_smoke_diagnostic", {
        stage: "frontend_app_mounted",
        detail: null,
      }).catch(() => undefined);
    }
    let stop: (() => void) | undefined;
    let disposed = false;
    void initializeActivationEvents().then(cleanup => {
      if (disposed) cleanup();
      else stop = cleanup;
    });
    return () => { disposed = true; stop?.(); };
  }, [initializeActivationEvents]);
  return <RouterProvider router={router} />;
}
