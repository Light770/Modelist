const DB_NAME = 'modelist_local_store';
const STORE_NAME = 'autosave';
const AUTOSAVE_KEY = 'latest_project';

/** Saved garments (the user's garment library), keyed by entry id. */
export const LIBRARY_STORE = 'garment_library';

export function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, 2);
    req.onupgradeneeded = () => {
      if (!req.result.objectStoreNames.contains(STORE_NAME)) {
        req.result.createObjectStore(STORE_NAME);
      }
      if (!req.result.objectStoreNames.contains(LIBRARY_STORE)) {
        req.result.createObjectStore(LIBRARY_STORE, { keyPath: 'id' });
      }
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

/** Stores the current project (a `.modelist` container) as the local autosave. */
export async function saveAutosave(bytes: Uint8Array): Promise<void> {
  try {
    const db = await openDb();
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction(STORE_NAME, 'readwrite');
      tx.objectStore(STORE_NAME).put({ bytes, savedAt: Date.now() }, AUTOSAVE_KEY);
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
  } catch {
    // Non-fatal in private browsing or restricted contexts
  }
}

/** The local autosave: container bytes, or JSON text from builds before the container. */
export async function loadAutosave(): Promise<{ bytes?: Uint8Array; jsonText?: string; savedAt: number } | null> {
  try {
    const db = await openDb();
    return await new Promise((resolve, reject) => {
      const tx = db.transaction(STORE_NAME, 'readonly');
      const req = tx.objectStore(STORE_NAME).get(AUTOSAVE_KEY);
      req.onsuccess = () => resolve(req.result ?? null);
      req.onerror = () => reject(req.error);
    });
  } catch {
    return null;
  }
}

export function downloadTextFile(filename: string, content: string, mime = 'text/plain;charset=utf-8') {
  const blob = new Blob([content], { type: mime });
  downloadBytesFile(filename, blob);
}

export function downloadBytesFile(filename: string, data: Blob | Uint8Array) {
  const blob =
    data instanceof Blob
      ? data
      : new Blob([new Uint8Array(data)], { type: 'application/octet-stream' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

function cleanFileName(name: string): string {
  return name.replace(/[^a-zA-Z0-9_-]+/g, '_').toLowerCase() || 'pattern';
}

type SavePicker = (opts: unknown) => Promise<{
  name: string;
  createWritable: () => Promise<{ write: (d: string | Blob | Uint8Array) => Promise<void>; close: () => Promise<void> }>;
}>;

/** Saves bytes or text through the save dialog (download where the API is missing). */
async function saveThroughPicker(
  filename: string,
  data: Uint8Array | string,
  description: string,
  mime: string,
  exts: string[],
): Promise<string> {
  const w = window as unknown as { showSaveFilePicker?: SavePicker };
  if (typeof w.showSaveFilePicker === 'function') {
    try {
      const handle = await w.showSaveFilePicker({
        suggestedName: filename,
        types: [{ description, accept: { [mime]: exts } }],
      });
      const writable = await handle.createWritable();
      await writable.write(data);
      await writable.close();
      return handle.name;
    } catch (e) {
      if ((e as Error)?.name === 'AbortError') throw e;
    }
  }
  if (typeof data === 'string') downloadTextFile(filename, data, mime);
  else downloadBytesFile(filename, new Blob([new Uint8Array(data)], { type: mime }));
  return filename;
}

/** Saves a project as a `.modelist` container. */
export async function saveProjectToDisk(suggestedName: string, bytes: Uint8Array): Promise<string> {
  return saveThroughPicker(
    `${cleanFileName(suggestedName)}.modelist`,
    bytes,
    'Modelist project',
    'application/vnd.modelist+zip',
    ['.modelist'],
  );
}

/** Saves the whole project as one JSON document (debugging, version control). */
export async function saveProjectJsonToDisk(suggestedName: string, jsonText: string): Promise<string> {
  return saveThroughPicker(
    `${cleanFileName(suggestedName)}.modelist.json`,
    jsonText,
    'Modelist project (JSON)',
    'application/json',
    ['.json'],
  );
}

/** Asks for a file and returns its bytes. */
export async function openBinaryFileFromDisk(acceptExts: string): Promise<{ name: string; bytes: Uint8Array } | null> {
  return new Promise((resolve) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = acceptExts;
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) {
        resolve(null);
        return;
      }
      resolve({ name: file.name, bytes: new Uint8Array(await file.arrayBuffer()) });
    };
    input.click();
  });
}

export async function openTextFileFromDisk(acceptExts: string): Promise<{ name: string; text: string } | null> {
  return new Promise((resolve) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = acceptExts;
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) {
        resolve(null);
        return;
      }
      const text = await file.text();
      resolve({ name: file.name, text });
    };
    input.click();
  });
}
