import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "./tauri";

export type QueueItem = { id: string; operationType: string; state: string; attempts: number; lastError?: string };
export const syncQueueService = {
  list: () => isTauri() ? invoke<QueueItem[]>("list_sync_queue") : Promise.resolve<QueueItem[]>([]),
  enqueue: (operationType = "library_sync", entityId: string | null = null) => isTauri() ? invoke<string>("enqueue_sync_operation", { operationType, entityId, revision: null }) : Promise.resolve(`web-${Date.now()}`),
  claim: (id: string) => invoke<boolean>("claim_sync_operation", { id }),
  finish: (id: string, success: boolean, error?: string) => invoke("finish_sync_operation", { id, success, error: error?.slice(0, 500) }),
  retry: (id: string) => invoke("retry_sync_operation", { id }),
};
export async function notifyLocalChange(operationType: string, entityId: string) {
  if (!localStorage.getItem("lumina.account.authenticated")) return;
  const errors: string[] = [];
  if (operationType.endsWith("_delete")) {
    const kind = operationType.startsWith("cursor") ? "cursor" : operationType.startsWith("playlist") ? "playlist" : "wallpaper";
    try { await invoke("record_sync_tombstone", { entityId, kind }); } catch (error) { errors.push(`No se pudo registrar la eliminación: ${String(error)}`); }
  }
  try { await syncQueueService.enqueue(operationType, entityId); } catch (error) { errors.push(`No se pudo encolar la sincronización: ${String(error)}`); }
  window.dispatchEvent(new Event("lumina:sync-state"));
  if (errors.length > 0) throw new Error(errors.join("; "));
}
