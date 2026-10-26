import { accountService } from "./accountService";
import { syncQueueService, type QueueItem } from "./syncQueueService";

let started = false;
let stopped = false;
let wake: (() => void) | null = null;
let processing = false;
let authHandler: (() => void) | null = null;

const delay = (milliseconds: number) => new Promise<void>(resolve => {
  const timer = window.setTimeout(resolve, milliseconds);
  wake = () => { window.clearTimeout(timer); wake = null; resolve(); };
});

async function processNext() {
  if (processing || !accountService.hasSession()) return false;
  let items: QueueItem[];
  try { items = await syncQueueService.list(); } catch (error) {
    window.dispatchEvent(new CustomEvent("lumina:sync-error", { detail: String(error) }));
    return false;
  }
  const item = items.find(candidate => candidate.state === "pending" || candidate.state === "retry");
  if (!item) return false;
  let claimed = false;
  try { claimed = await syncQueueService.claim(item.id); } catch (error) {
    window.dispatchEvent(new CustomEvent("lumina:sync-error", { detail: String(error) }));
    return false;
  }
  if (!claimed) return false;
  processing = true; window.dispatchEvent(new Event("lumina:sync-state"));
  const sessionAtStart = accountService.hasSession();
  try {
    const result = await accountService.syncLibrary();
    const sessionStillValid = sessionAtStart && accountService.hasSession();
    await syncQueueService.finish(item.id, sessionStillValid && result.errors.length === 0, sessionStillValid ? result.errors.join("; ") : "La sesión terminó durante la sincronización");
  } catch (error) {
    try { await syncQueueService.finish(item.id, false, String(error)); }
    catch (finishError) { window.dispatchEvent(new CustomEvent("lumina:sync-error", { detail: `${String(error)}; no se pudo guardar el estado de reintento: ${String(finishError)}` })); }
  } finally {
    processing = false; window.dispatchEvent(new Event("lumina:sync-state"));
  }
  return true;
}

async function loop() {
  while (!stopped) {
    const processed = await processNext();
    if (!processed) await delay(accountService.hasSession() ? 30_000 : 60_000);
  }
}

export const syncCoordinator = {
  start() { if (started) return; started = true; stopped = false; authHandler = () => wake?.(); window.addEventListener("lumina:auth-changed", authHandler); void loop(); },
  stop() { stopped = true; started = false; if (authHandler) window.removeEventListener("lumina:auth-changed", authHandler); authHandler = null; wake?.(); wake = null; },
  async requestNow() { if (!accountService.hasSession()) throw new Error("Inicia sesión para sincronizar la biblioteca"); const id = await syncQueueService.enqueue(); wake?.(); window.dispatchEvent(new Event("lumina:sync-state")); return id; },
  isProcessing: () => processing,
};
