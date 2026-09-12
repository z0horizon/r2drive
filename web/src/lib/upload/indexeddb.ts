/**
 * IndexedDB helper for persisting multipart upload manifests in r2drive WebConsole.
 * Enables resumable transfers across page reloads and browser sessions.
 */

export interface CompletedPartManifest {
  part_number: number;
  etag: string;
}

export interface UploadManifest {
  uploadId: string;
  key: string;
  profile: string;
  fileSize: number;
  partSize: number;
  completedParts: CompletedPartManifest[];
  createdAt: number;
}

const DB_NAME = 'r2drive_transfers';
const DB_VERSION = 1;
const STORE_NAME = 'sessions';

/**
 * Checks if IndexedDB is supported and accessible in the current environment.
 */
function isIndexedDBAvailable(): boolean {
  return typeof window !== 'undefined' && typeof window.indexedDB !== 'undefined';
}

/**
 * Opens the IndexedDB database instance with the sessions store schema.
 * Safely resolves null if IndexedDB is not available or blocked.
 */
function openDB(): Promise<IDBDatabase | null> {
  if (!isIndexedDBAvailable()) {
    return Promise.resolve(null);
  }

  return new Promise((resolve) => {
    try {
      const request = window.indexedDB.open(DB_NAME, DB_VERSION);

      request.onupgradeneeded = (event) => {
        const db = (event.target as IDBOpenDBRequest).result;
        if (!db.objectStoreNames.contains(STORE_NAME)) {
          db.createObjectStore(STORE_NAME, { keyPath: 'uploadId' });
        }
      };

      request.onsuccess = () => {
        resolve(request.result);
      };

      request.onerror = () => {
        resolve(null);
      };

      request.onblocked = () => {
        resolve(null);
      };
    } catch {
      resolve(null);
    }
  });
}

/**
 * Persists or updates an upload session manifest.
 */
export async function saveSession(manifest: UploadManifest): Promise<void> {
  const db = await openDB();
  if (!db) return;

  return new Promise((resolve, reject) => {
    try {
      const tx = db.transaction(STORE_NAME, 'readwrite');
      const store = tx.objectStore(STORE_NAME);
      const req = store.put(manifest);

      req.onsuccess = () => resolve();
      req.onerror = () => reject(req.error || new Error('Failed to save session to IndexedDB'));
    } catch (err) {
      reject(err);
    }
  });
}

/**
 * Retrieves an upload session manifest by its upload ID.
 */
export async function getSession(uploadId: string): Promise<UploadManifest | null> {
  const db = await openDB();
  if (!db) return null;

  return new Promise((resolve, reject) => {
    try {
      const tx = db.transaction(STORE_NAME, 'readonly');
      const store = tx.objectStore(STORE_NAME);
      const req = store.get(uploadId);

      req.onsuccess = () => resolve((req.result as UploadManifest) ?? null);
      req.onerror = () => reject(req.error || new Error('Failed to get session from IndexedDB'));
    } catch (err) {
      reject(err);
    }
  });
}

/**
 * Lists all active persisted upload sessions.
 */
export async function listSessions(): Promise<UploadManifest[]> {
  const db = await openDB();
  if (!db) return [];

  return new Promise((resolve, reject) => {
    try {
      const tx = db.transaction(STORE_NAME, 'readonly');
      const store = tx.objectStore(STORE_NAME);
      const req = store.getAll();

      req.onsuccess = () => resolve((req.result as UploadManifest[]) ?? []);
      req.onerror = () => reject(req.error || new Error('Failed to list sessions from IndexedDB'));
    } catch (err) {
      reject(err);
    }
  });
}

/**
 * Deletes an upload session from IndexedDB.
 */
export async function deleteSession(uploadId: string): Promise<void> {
  const db = await openDB();
  if (!db) return;

  return new Promise((resolve, reject) => {
    try {
      const tx = db.transaction(STORE_NAME, 'readwrite');
      const store = tx.objectStore(STORE_NAME);
      const req = store.delete(uploadId);

      req.onsuccess = () => resolve();
      req.onerror = () => reject(req.error || new Error('Failed to delete session from IndexedDB'));
    } catch (err) {
      reject(err);
    }
  });
}

/**
 * Appends or updates a completed part in the persisted session manifest.
 */
export async function addCompletedPart(
  uploadId: string,
  part: { part_number: number; etag: string }
): Promise<void> {
  const session = await getSession(uploadId);
  if (!session) return;

  const existingIdx = session.completedParts.findIndex(
    (p) => p.part_number === part.part_number
  );
  if (existingIdx >= 0) {
    session.completedParts[existingIdx] = part;
  } else {
    session.completedParts.push(part);
  }
  session.completedParts.sort((a, b) => a.part_number - b.part_number);

  await saveSession(session);
}
