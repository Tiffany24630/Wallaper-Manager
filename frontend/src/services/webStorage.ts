import type { Wallpaper } from "../types/wallpaper";

const DB_NAME = "lumina-web";
const DB_VERSION = 2;
const WALLPAPERS = "wallpapers";
const TRASH = "trash";
const objectUrls = new Map<string, string>();

type StoredWallpaper = Omit<Wallpaper, "path" | "thumbnail" | "asset" | "thumbnailAsset"> & { blob: Blob };

function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION);
    request.onupgradeneeded = () => { const db = request.result; if (!db.objectStoreNames.contains(WALLPAPERS)) db.createObjectStore(WALLPAPERS, { keyPath: "id" }); if (!db.objectStoreNames.contains(TRASH)) db.createObjectStore(TRASH, { keyPath: "id" }); };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("No se pudo abrir el almacenamiento web"));
  });
}

async function readAll(): Promise<StoredWallpaper[]> {
  const db = await openDb();
  return new Promise((resolve, reject) => {
    const request = db.transaction(WALLPAPERS, "readonly").objectStore(WALLPAPERS).getAll();
    request.onsuccess = () => resolve(request.result as StoredWallpaper[]);
    request.onerror = () => reject(request.error);
  });
}

async function write(value: StoredWallpaper): Promise<void> {
  const db = await openDb();
  await new Promise<void>((resolve, reject) => {
    const request = db.transaction(WALLPAPERS, "readwrite").objectStore(WALLPAPERS).put(value);
    request.onsuccess = () => resolve();
    request.onerror = () => reject(request.error);
  });
}

async function moveToTrash(id: string): Promise<void> {
  const db = await openDb();
  const item = (await readAll()).find(value => value.id === id);
  if (!item) return;
  await new Promise<void>((resolve, reject) => {
    const transaction = db.transaction([WALLPAPERS, TRASH], "readwrite");
    transaction.objectStore(TRASH).put({ ...item, deletedAt: new Date().toISOString() });
    transaction.objectStore(WALLPAPERS).delete(id);
    transaction.oncomplete = () => resolve();
    transaction.onerror = () => reject(transaction.error);
  });
  const url = objectUrls.get(id); if (url) URL.revokeObjectURL(url); objectUrls.delete(id);
}

async function readTrash(): Promise<StoredWallpaper[]> {
  const db = await openDb();
  return new Promise((resolve, reject) => { const request = db.transaction(TRASH, "readonly").objectStore(TRASH).getAll(); request.onsuccess = () => resolve(request.result as StoredWallpaper[]); request.onerror = () => reject(request.error); });
}

function urlFor(id: string, blob: Blob): string {
  const current = objectUrls.get(id);
  if (current) return current;
  const url = URL.createObjectURL(blob);
  objectUrls.set(id, url);
  return url;
}

async function dimensions(blob: Blob): Promise<{ width?: number; height?: number }> {
  try {
    const bitmap = await createImageBitmap(blob);
    const result = { width: bitmap.width, height: bitmap.height };
    bitmap.close();
    return result;
  } catch {
    return {};
  }
}

async function hash(blob: Blob): Promise<string> {
  const bytes = await crypto.subtle.digest("SHA-256", await blob.arrayBuffer());
  return Array.from(new Uint8Array(bytes)).map(value => value.toString(16).padStart(2, "0")).join("");
}

function toWallpaper(item: StoredWallpaper): Wallpaper {
  const path = urlFor(item.id, item.blob);
  return { ...item, path, thumbnail: path, asset: { path, available: true, hash: item.hash, sizeBytes: item.sizeBytes, fileType: item.fileType }, thumbnailAsset: { path, available: true } };
}

export const webStorage = {
  async list(): Promise<Wallpaper[]> { return (await readAll()).map(toWallpaper).sort((a, b) => (b.createdAt ?? "").localeCompare(a.createdAt ?? "")); },
  async importFile(file: File): Promise<string> {
    const id = crypto.randomUUID();
    const digest = await hash(file);
    const size = file.size;
    const measure = await dimensions(file);
    const item: StoredWallpaper = { id, name: file.name, favorite: false, active: false, sizeBytes: size, width: measure.width, height: measure.height, fileType: file.name.split(".").pop()?.toLowerCase(), hash: digest, createdAt: new Date().toISOString(), resourceType: "static", blob: file };
    await write(item);
    urlFor(id, file);
    return id;
  },
  async update(id: string, changes: Partial<StoredWallpaper>): Promise<void> { const item = (await readAll()).find(value => value.id === id); if (item) await write({ ...item, ...changes }); },
  async delete(id: string): Promise<void> { await moveToTrash(id); },
  async trash(): Promise<Wallpaper[]> { return (await readTrash()).map(toWallpaper); },
  async restore(id: string): Promise<void> { const db = await openDb(); const item = (await readTrash()).find(value => value.id === id); if (!item) return; await new Promise<void>((resolve, reject) => { const transaction = db.transaction([WALLPAPERS, TRASH], "readwrite"); transaction.objectStore(WALLPAPERS).put(item); transaction.objectStore(TRASH).delete(id); transaction.oncomplete = () => resolve(); transaction.onerror = () => reject(transaction.error); }); },
  async organize(id: string, tags: string[], folder: string): Promise<void> { const item = (await readAll()).find(value => value.id === id); if (item) await write({ ...item, tags: [...new Set(tags.map(tag => tag.trim().toLowerCase()).filter(Boolean))].slice(0, 20), folder: folder.trim().slice(0, 80) || undefined }); },
  async getBlob(id: string): Promise<Blob | undefined> { return (await readAll()).find(item => item.id === id)?.blob; },
  async rotate(id: string, degrees: 0 | 90 | 180 | 270): Promise<void> {
    const item = (await readAll()).find(value => value.id === id);
    if (!item || degrees === 0) return;
    const image = await createImageBitmap(item.blob);
    const canvas = document.createElement("canvas");
    const quarterTurn = degrees === 90 || degrees === 270;
    canvas.width = quarterTurn ? image.height : image.width;
    canvas.height = quarterTurn ? image.width : image.height;
    const context = canvas.getContext("2d");
    if (!context) { image.close(); throw new Error("El navegador no permite editar esta imagen."); }
    context.translate(canvas.width / 2, canvas.height / 2);
    context.rotate((degrees * Math.PI) / 180);
    context.drawImage(image, -image.width / 2, -image.height / 2);
    image.close();
    const blob = await new Promise<Blob>((resolve, reject) => canvas.toBlob(value => value ? resolve(value) : reject(new Error("No se pudo generar la imagen rotada.")), item.blob.type || "image/png", 0.95));
    const digest = await hash(blob);
    await write({ ...item, blob, hash: digest, sizeBytes: blob.size, width: canvas.width, height: canvas.height });
    const oldUrl = objectUrls.get(id);
    if (oldUrl) URL.revokeObjectURL(oldUrl);
    objectUrls.set(id, URL.createObjectURL(blob));
  },
  async adjust(id: string, values: { brightness: number; contrast: number; saturation: number }): Promise<void> {
    const item = (await readAll()).find(value => value.id === id);
    if (!item) return;
    const image = await createImageBitmap(item.blob);
    const canvas = document.createElement("canvas"); canvas.width = image.width; canvas.height = image.height;
    const context = canvas.getContext("2d");
    if (!context) { image.close(); throw new Error("El navegador no permite editar esta imagen."); }
    context.filter = `brightness(${values.brightness}%) contrast(${values.contrast}%) saturate(${values.saturation}%)`;
    context.drawImage(image, 0, 0); image.close();
    const blob = await new Promise<Blob>((resolve, reject) => canvas.toBlob(value => value ? resolve(value) : reject(new Error("No se pudo guardar el ajuste.")), item.blob.type || "image/png", 0.95));
    await write({ ...item, blob, hash: await hash(blob), sizeBytes: blob.size });
    const oldUrl = objectUrls.get(id); if (oldUrl) URL.revokeObjectURL(oldUrl); objectUrls.set(id, URL.createObjectURL(blob));
  },
  async exportMetadata(): Promise<string> {
    const wallpapers = (await readAll()).map(({ blob, ...item }) => item);
    const playlists = JSON.parse(localStorage.getItem("lumina.web.playlists") || "[]");
    return JSON.stringify({ version: 1, exportedAt: new Date().toISOString(), wallpapers, playlists }, null, 2);
  },
  async importMetadata(json: string): Promise<number> {
    const data = JSON.parse(json) as { wallpapers?: Array<{ id: string; name?: string; favorite?: boolean; tags?: string[]; folder?: string }>; playlists?: unknown[] };
    const current = await readAll(); const incoming = new Map((data.wallpapers ?? []).map(item => [item.id, item])); let changed = 0;
    for (const item of current) { const update = incoming.get(item.id); if (!update) continue; await write({ ...item, name: typeof update.name === "string" ? update.name : item.name, favorite: Boolean(update.favorite), tags: Array.isArray(update.tags) ? update.tags.filter(tag => typeof tag === "string") : item.tags, folder: typeof update.folder === "string" ? update.folder : item.folder }); changed += 1; }
    if (Array.isArray(data.playlists)) localStorage.setItem("lumina.web.playlists", JSON.stringify(data.playlists));
    return changed;
  },
};

export function webFileType(path: string): string { return path.split(".").pop()?.toLowerCase() || "png"; }
