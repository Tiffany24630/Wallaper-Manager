import { RouterProvider } from "react-router-dom";
import { useEffect } from "react";
import { router } from "./routes";
import { useWallpaperStore } from "./store/wallpaperStore";

export default function App() {
  const initializeActivationEvents = useWallpaperStore(state => state.initializeActivationEvents);
  useEffect(() => {
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
