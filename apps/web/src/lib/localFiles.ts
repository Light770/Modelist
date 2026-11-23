const DB_NAME = 'modelist_local_store';
const STORE_NAME = 'autosave';
const AUTOSAVE_KEY = 'latest_project';

function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, 1);
    req.onupgradeneeded = () => {
      if (!req.result.objectStoreNames.contains(STORE_NAME)) {
        req.result.createObjectStore(STORE_NAME);
      }
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

export async function saveAutosave(jsonText: string): Promise<void> {
  try {
    const db = await openDb();
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction(STORE_NAME, 'readwrite');
      tx.objectStore(STORE_NAME).put({ jsonText, savedAt: Date.now() }, AUTOSAVE_KEY);
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
  } catch {
    // Non-fatal in private browsing or restricted contexts
  }
}

export async function loadAutosave(): Promise<{ jsonText: string; savedAt: number } | null> {
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

export async function saveProjectToDisk(suggestedName: string, jsonText: string): Promise<string> {
  const cleanName = suggestedName.replace(/[^a-zA-Z0-9_-]+/g, '_').toLowerCase() || 'pattern';
  const filename = `${cleanName}.modelist.json`;

  const w = window as unknown as {
    showSaveFilePicker?: (opts: unknown) => Promise<{
      name: string;
      createWritable: () => Promise<{ write: (s: string) => Promise<void>; close: () => Promise<void> }>;
    }>;
  };

  if (typeof w.showSaveFilePicker === 'function') {
    try {
      const handle = await w.showSaveFilePicker({
        suggestedName: filename,
        types: [
          {
            description: 'Modelist Project Archive',
            accept: { 'application/json': ['.modelist.json', '.modelist', '.json'] },
          },
        ],
      });
      const writable = await handle.createWritable();
      await writable.write(jsonText);
      await writable.close();
      return handle.name;
    } catch (e) {
      if ((e as Error)?.name === 'AbortError') throw e;
    }
  }

  downloadTextFile(filename, jsonText, 'application/json');
  return filename;
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
