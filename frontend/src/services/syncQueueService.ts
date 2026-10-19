import { invoke } from "@tauri-apps/api/core";

export type QueueItem = { id: string; operationType: string; state: string; attempts: number; lastError?: string };
export const syncQueueService = {
  list: () => invoke<QueueItem[]>("list_sync_queue"),
  enqueue: (operationType = "library_sync", entityId: string | null = null) => invoke<string>("enqueue_sync_operation", { operationType, entityId, revision: null }),
  claim: (id: string) => invoke<boolean>("claim_sync_operation", { id }),
  finish: (id: string, success: boolean, error?: string) => invoke("finish_sync_operation", { id, success, error: error?.slice(0, 500) }),
  retry: (id: string) => invoke("retry_sync_operation", { id }),
};
export async function notifyLocalChange(operationType: string, entityId: string) {
  if (!localStorage.getItem("lumina.account.authenticated")) return;
  if (operationType.endsWith("_delete")) {
    const kind = operationType.startsWith("cursor") ? "cursor" : operationType.startsWith("playlist") ? "playlist" : "wallpaper";
    await invoke("record_sync_tombstone", { entityId, kind }).catch(() => undefined);
  }
  await syncQueueService.enqueue(operationType, entityId).catch(() => undefined);
  window.dispatchEvent(new Event("lumina:sync-state"));
}
