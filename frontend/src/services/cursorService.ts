import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { CursorResource } from "../types/cursor";
import { notifyLocalChange } from "./syncQueueService";

export const cursorService = {
  async list(): Promise<CursorResource[]> { return invoke<CursorResource[]>("list_cursors"); },
  async select(): Promise<string | null> {
    const selected = await open({ multiple: false, filters: [{ name: "Cursores e imágenes", extensions: ["cur", "ani", "png", "jpg", "jpeg", "webp", "bmp"] }] });
    return selected as string | null;
  },
  async import(path: string, name?: string, hotspotX = 0, hotspotY = 0): Promise<string> {
    const id = await invoke<string>("import_cursor", { sourcePath: path, name, hotspotX, hotspotY }); await notifyLocalChange("cursor_create", id); return id;
  },
  async rename(id: string, name: string): Promise<void> { await invoke("rename_cursor", { cursorId: id, name }); await notifyLocalChange("cursor_update", id); },
  async activate(id: string): Promise<void> { return invoke("activate_cursor", { cursorId: id }); },
  async restore(): Promise<void> { return invoke("restore_default_cursor"); },
  async remove(id: string): Promise<void> { await invoke("delete_cursor", { cursorId: id }); await notifyLocalChange("cursor_delete", id); },
};
