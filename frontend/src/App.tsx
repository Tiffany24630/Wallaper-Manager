import { RouterProvider } from "react-router-dom";
import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { router } from "./routes";
import { useWallpaperStore } from "./store/wallpaperStore";
import { accountService } from "./services/accountService";
import { syncCoordinator } from "./services/syncCoordinator";
import { syncQueueService } from "./services/syncQueueService";

let smokeExitRequested = false;
let smokeE2EStarted = false;

type SmokeE2EConfig = { email: string; password: string };

async function waitForSmokeSync(queueId: string) {
  const deadline = Date.now() + 90_000;
  while (Date.now() < deadline) {
    const item = (await syncQueueService.list()).find(candidate => candidate.id === queueId);
    if (item?.state === "completed") return;
    if (item?.state === "failed") throw new Error(item.lastError || "La sincronización smoke falló");
    await new Promise(resolve => window.setTimeout(resolve, 100));
  }
  throw new Error("La sincronización smoke excedió el tiempo límite");
}

function smokeError(error: unknown, config: SmokeE2EConfig): string {
  return String(error)
    .replaceAll(config.password, "[redacted]")
    .replaceAll(config.email, "[synthetic-account]")
    .slice(0, 500);
}

async function runSmokeE2E(config: SmokeE2EConfig, token: string) {
  try {
    await accountService.login(config.email, config.password);
    await accountService.registerDevice("Lumina Windows Smoke");
    const queueId = await syncCoordinator.requestNow();
    await waitForSmokeSync(queueId);
    const remote = await accountService.remoteResources();
    const file = remote.files.find(candidate => candidate.kind === "wallpaper");
    if (!file) throw new Error("No se recibió ningún wallpaper remoto");
    const record = remote.records.find(candidate => candidate.record_id === file.record_id && candidate.kind === "wallpaper" && !candidate.deleted);
    if (!record) throw new Error("El archivo remoto no tiene un registro wallpaper válido");
    const payload = record.payload;
    const extension = String(payload.file_type || file.extension || "png");
    const name = String(payload.name || file.original_name || "Wallpaper smoke");
    const restoredId = await accountService.restoreWallpaper(file.id, file.record_id, name, extension, file.sha256);
    const secondQueueId = await syncCoordinator.requestNow();
    await waitForSmokeSync(secondQueueId);
    await invoke("record_smoke_diagnostic", {
      stage: "e2e_sync_completed",
      detail: JSON.stringify({
        kind: file.kind,
        record_id: file.record_id,
        revision: record.revision,
        file_id: file.id,
        restored_id: restoredId,
        size_bytes: payload.size_bytes ?? null,
        sha256: file.sha256,
        second_sync_completed: true,
      }),
    });
    await accountService.logout();
    await invoke("request_smoke_exit", { token, reason: "e2e_sync_completed", code: 0 });
  } catch (error) {
    await invoke("record_smoke_diagnostic", {
      stage: "e2e_sync_failed",
      detail: smokeError(error, config),
    }).catch(() => undefined);
    await accountService.logout().catch(() => undefined);
    await invoke("request_smoke_exit", { token, reason: "e2e_sync_failed", code: 1 }).catch(() => undefined);
  } finally {
    const smokeWindow = window as Window & { __LUMINA_SMOKE_E2E__?: SmokeE2EConfig };
    delete smokeWindow.__LUMINA_SMOKE_E2E__;
  }
}

export default function App() {
  const initializeActivationEvents = useWallpaperStore(state => state.initializeActivationEvents);
  useEffect(() => {
    const smokeWindow = window as Window & {
      __LUMINA_SMOKE__?: boolean;
      __LUMINA_SMOKE_EXIT__?: boolean;
      __LUMINA_SMOKE_TOKEN__?: string;
      __LUMINA_SMOKE_E2E__?: SmokeE2EConfig;
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
      if (smokeWindow.__LUMINA_SMOKE_E2E__ && smokeWindow.__LUMINA_SMOKE_TOKEN__ && !smokeE2EStarted) {
        smokeE2EStarted = true;
        void runSmokeE2E(smokeWindow.__LUMINA_SMOKE_E2E__, smokeWindow.__LUMINA_SMOKE_TOKEN__);
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
