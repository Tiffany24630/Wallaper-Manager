import { RouterProvider } from "react-router-dom";
import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { router } from "./routes";
import { useWallpaperStore } from "./store/wallpaperStore";

let smokeExitRequested = false;

export default function App() {
  const initializeActivationEvents = useWallpaperStore(state => state.initializeActivationEvents);
  useEffect(() => {
    const smokeWindow = window as Window & {
      __LUMINA_SMOKE__?: boolean;
      __LUMINA_SMOKE_EXIT__?: boolean;
      __LUMINA_SMOKE_TOKEN__?: string;
    };
    const isSmoke = smokeWindow.__LUMINA_SMOKE__ === true;
    if (isSmoke) {
      const diagnostic = invoke("record_smoke_diagnostic", {
        stage: "frontend_app_mounted",
        detail: null,
      }).catch(() => undefined);
      if (
        smokeWindow.__LUMINA_SMOKE_EXIT__ === true &&
        smokeWindow.__LUMINA_SMOKE_TOKEN__ &&
        !smokeExitRequested
      ) {
        smokeExitRequested = true;
        void diagnostic.finally(() =>
          invoke("request_smoke_exit", {
            token: smokeWindow.__LUMINA_SMOKE_TOKEN__,
            reason: "frontend_app_mounted",
            code: 0,
          }).catch(() => undefined),
        );
      }
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
