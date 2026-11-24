/**
 * The user's garment library: garments saved from a document, kept locally (IndexedDB) as
 * single-garment `.modelist` files. Inserting one refits it to the open document's body.
 */
import { LIBRARY_STORE, openDb } from './localFiles';

export interface SavedGarment {
  /** Library entry id (not a pattern ID). */
  id: string;
  name: string;
  template_id: string;
  saved_at: number;
  /** PNG preview of the canvas when it was saved (may be empty). */
  thumbnail: Uint8Array;
  /** The garment as a single-garment `.modelist` container. */
  bytes: Uint8Array;
}

async function tx<T>(mode: IDBTransactionMode, run: (store: IDBObjectStore) => IDBRequest<T>): Promise<T> {
  const db = await openDb();
  return new Promise<T>((resolve, reject) => {
    const req = run(db.transaction(LIBRARY_STORE, mode).objectStore(LIBRARY_STORE));
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

/** Saved garments, most recent first (empty when storage is unavailable). */
export async function listSavedGarments(): Promise<SavedGarment[]> {
  try {
    const all = await tx<SavedGarment[]>('readonly', (s) => s.getAll() as IDBRequest<SavedGarment[]>);
    return all.sort((a, b) => b.saved_at - a.saved_at);
  } catch {
    return [];
  }
}

export async function saveGarment(entry: Omit<SavedGarment, 'id' | 'saved_at'>): Promise<SavedGarment> {
  const full: SavedGarment = { ...entry, id: crypto.randomUUID(), saved_at: Date.now() };
  await tx('readwrite', (s) => s.put(full));
  return full;
}

export async function getSavedGarment(id: string): Promise<SavedGarment | null> {
  return (await tx<SavedGarment | undefined>('readonly', (s) => s.get(id) as IDBRequest<SavedGarment | undefined>)) ?? null;
}

export async function deleteSavedGarment(id: string): Promise<void> {
  await tx('readwrite', (s) => s.delete(id));
}

/** Object URL of a saved garment's preview (revoke when done), or null. */
export function thumbnailUrl(g: SavedGarment): string | null {
  if (!g.thumbnail || g.thumbnail.length === 0) return null;
  return URL.createObjectURL(new Blob([new Uint8Array(g.thumbnail)], { type: 'image/png' }));
}
