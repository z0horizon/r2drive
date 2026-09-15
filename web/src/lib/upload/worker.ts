/**
 * Direct-to-R2 and Resumable Upload Engine for r2drive WebConsole.
 *
 * Implements:
 * - Single PUT uploads for small files (<10MB)
 * - Sliced multipart uploads for large files (>=10MB)
 * - Concurrency pool (max 4 parallel parts)
 * - Exponential backoff retry for transient failures (up to 5 attempts)
 * - Session persistence in IndexedDB for interrupted transfers
 * - Abort/cancellation lifecycle integration with server cleanup
 */

import {
  initUpload,
  resumeUpload,
  completeUpload,
  abortUpload,
  uploadViaProxy,
  type CompletedPart,
  type PresignedPart,
} from '../api/transfers';
import { uploadStore, type UploadItem } from '../stores/upload.svelte';
import { bucketStore } from '../stores/bucket.svelte';
import {
  saveSession,
  deleteSession,
  addCompletedPart,
  type UploadManifest,
} from './indexeddb';
import { formatBytes } from '../utils/format';

export const PART_SIZE = 10 * 1024 * 1024; // 10MB (10,485,760 bytes)
export const MAX_CONCURRENCY = 4;
export const MAX_RETRIES = 5;
export const BASE_RETRY_DELAY_MS =
  typeof process !== 'undefined' && process.env?.NODE_ENV === 'test' ? 10 : 300;

/**
 * Normalizes prefix and file name into a clean S3 object key.
 * Strips leading slashes and collapses multiple consecutive slashes.
 */
export function cleanObjectKey(prefix: string, fileName: string): string {
  const combined = `${prefix}/${fileName}`;
  return combined.split('/').filter(Boolean).join('/');
}

export interface ChunkPlan {
  part_number: number;
  start: number;
  end: number;
  size: number;
}

/**
 * Slices a total file size into part ranges according to partSize.
 */
export function calculateChunkPlan(fileSize: number, partSize: number = PART_SIZE): ChunkPlan[] {
  if (fileSize <= 0) return [];
  const partCount = Math.ceil(fileSize / partSize);
  const chunks: ChunkPlan[] = [];

  for (let i = 1; i <= partCount; i++) {
    const start = (i - 1) * partSize;
    const end = Math.min(fileSize, start + partSize);
    chunks.push({
      part_number: i,
      start,
      end,
      size: end - start,
    });
  }

  return chunks;
}

export function formatUploadErrorMessage(err: unknown): string {
  if (err instanceof Error) {
    if (err.name === 'AbortError' || err.message.includes('aborted')) {
      return err.message;
    }
    if (err.message === 'Failed to fetch' || err.name === 'TypeError') {
      const origin = typeof window !== 'undefined' ? window.location.origin : 'current origin';
      return `Failed to fetch: Request blocked by CORS or network error. Ensure CORS is configured on your Cloudflare R2 bucket for ${origin}.`;
    }
    return err.message;
  }
  return String(err);
}

/**
 * Detects whether an error is caused by a CORS restriction or network block.
 * When direct-to-R2 upload is blocked by browser CORS policies, browsers throw
 * specific error messages ("Failed to fetch", "NetworkError", "Load failed", etc.).
 */
export function isCorsOrNetworkError(err: unknown): boolean {
  if (!err || typeof err !== 'object') return false;
  const e = err as { name?: string; message?: string };
  if (e.name === 'AbortError' || e.message?.toLowerCase().includes('abort')) {
    return false;
  }
  const msg = (e.message || '').toLowerCase();
  return (
    msg.includes('failed to fetch') ||
    msg.includes('fetch failed') ||
    msg.includes('networkerror') ||
    msg.includes('load failed') ||
    msg.includes('cors') ||
    msg.includes('cross-origin') ||
    msg.includes('access-control-allow-origin')
  );
}

export async function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Uploads an individual chunk to a presigned R2 URL with exponential backoff retries.
 */
export async function uploadPartWithRetry(
  url: string,
  chunk: Blob,
  signal?: AbortSignal,
  maxRetries = MAX_RETRIES,
  baseDelayMs = BASE_RETRY_DELAY_MS
): Promise<string> {
  let lastError: unknown;

  for (let attempt = 1; attempt <= maxRetries; attempt++) {
    if (signal?.aborted) {
      throw new DOMException('Upload aborted by user', 'AbortError');
    }

    try {
      const res = await fetch(url, {
        method: 'PUT',
        body: chunk,
        signal,
      });

      if (res.ok) {
        const rawEtag = res.headers.get('ETag') || res.headers.get('etag');
        const etag = rawEtag ? rawEtag.trim() : '';
        if (!etag) {
          throw new Error(
            'Upload response missing ETag header. Verify R2 CORS ExposeHeaders includes ETag'
          );
        }
        return etag;
      }

      // Do not retry permanent 4xx errors (e.g. 400, 403, 404, 405) - only retry 5xx or network errors (plus 408/429)
      if (res.status >= 400 && res.status < 500 && res.status !== 408 && res.status !== 429) {
        throw new Error(`HTTP ${res.status}: ${res.statusText}`);
      }

      lastError = new Error(`HTTP ${res.status}: ${res.statusText}`);
    } catch (err) {
      if (signal?.aborted || (err instanceof DOMException && err.name === 'AbortError')) {
        throw err;
      }
      // Fail immediately on missing ETag or non-retryable 4xx
      if (
        err instanceof Error &&
        (err.message.includes('missing ETag header') ||
          err.message.startsWith('HTTP 400:') ||
          err.message.startsWith('HTTP 403:') ||
          err.message.startsWith('HTTP 404:') ||
          err.message.startsWith('HTTP 405:'))
      ) {
        throw err;
      }
      // Do not retry on CORS or network block error: fail immediately to allow fast fallback
      if (isCorsOrNetworkError(err)) {
        throw err;
      }
      lastError = err;
    }

    if (attempt < maxRetries) {
      // Exponential backoff: baseDelay * 2^(attempt - 1) + jitter
      const delay = baseDelayMs * Math.pow(2, attempt - 1) + Math.random() * 50;
      await sleep(delay);
    }
  }

  throw new Error(
    `Part upload failed after ${maxRetries} attempts: ${formatUploadErrorMessage(lastError)}`
  );
}

interface ActiveTransfer {
  controller: AbortController;
  userAborted: boolean;
}

/**
 * Active transfer registry keyed by store item ID.
 */
const activeControllers = new Map<string, ActiveTransfer>();

/**
 * Cancels an active or queued transfer by its item ID.
 * Signals AbortController, sends abort request to server if multipart,
 * removes IndexedDB session, and marks item as aborted in uploadStore.
 */
export async function cancelUpload(id: string): Promise<void> {
  const entry = activeControllers.get(id);
  if (entry) {
    entry.userAborted = true;
    entry.controller.abort();
  }

  const item = uploadStore.items.find((i) => i.id === id);
  if (item) {
    if (item.uploadId) {
      try {
        await abortUpload(item.profile, item.uploadId);
      } catch (err) {
        console.warn(`Failed to abort upload session '${item.uploadId}' on server:`, err);
      }
      try {
        await deleteSession(item.uploadId);
      } catch (err) {
        console.warn(`Failed to remove IndexedDB session for '${item.uploadId}':`, err);
      }
    }
    uploadStore.markAborted(id);
  }
}

/**
 * Executes fallback upload via server streaming proxy when direct R2 transfer
 * is blocked by CORS or network policies.
 */
async function executeProxyFallback(
  item: UploadItem,
  file: File | Blob,
  profile: string,
  key: string,
  signal: AbortSignal
): Promise<UploadItem> {
  bucketStore.corsStatus = 'blocked';

  if (!uploadStore.proxyFallbackPreference) {
    throw new Error('Upload blocked by CORS. Server proxy fallback is disabled by user settings.');
  }
  const serverPolicy = bucketStore.serverFallbackPolicy;
  if (!serverPolicy.enabled) {
    throw new Error('Upload blocked by CORS. Server proxy fallback is disabled by server configuration.');
  }
  if (file.size > serverPolicy.max_payload_bytes) {
    throw new Error(
      `Upload blocked by CORS. File size (${formatBytes(file.size)}) exceeds server proxy upload limit of ${formatBytes(serverPolicy.max_payload_bytes)}.`
    );
  }

  const proxyStartTime = Date.now();
  uploadStore.setFallback(item.id);
  uploadStore.updateProgress(item.id, 0, 0, 0);

  await uploadViaProxy(
    profile,
    key,
    file,
    (loaded, total) => {
      const progress = total > 0 ? Math.min(99, Math.round((loaded / total) * 100)) : 0;
      const elapsedSec = Math.max(0.1, (Date.now() - proxyStartTime) / 1000);
      const speed = Math.round(loaded / elapsedSec);
      uploadStore.updateProgress(item.id, progress, speed, loaded);
    },
    signal
  );

  const elapsedSec = Math.max(0.1, (Date.now() - proxyStartTime) / 1000);
  const speed = Math.round(file.size / elapsedSec);
  uploadStore.updateProgress(item.id, 100, speed, file.size);
  uploadStore.markComplete(item.id);
  await bucketStore.refresh();
  return item;
}

/**
 * Uploads a file directly to Cloudflare R2.
 * Uses single PUT for files < 10MB, or multipart chunking for files >= 10MB.
 */
export async function uploadFile(
  file: File,
  profile: string,
  prefix: string
): Promise<UploadItem> {
  const key = cleanObjectKey(prefix, file.name);
  const item = uploadStore.add({
    file,
    key,
    profile,
    name: file.name,
  });

  const controller = new AbortController();
  activeControllers.set(item.id, { controller, userAborted: false });
  const startTime = Date.now();

  try {
    if (bucketStore.corsStatus === 'blocked') {
      return await executeProxyFallback(item, file, profile, key, controller.signal);
    }

    if (file.size < PART_SIZE) {
      // Small file single PUT upload
      uploadStore.updateProgress(item.id, 0, 0, 0);

      const initRes = await initUpload(profile, key, file.size, file.type || undefined);
      if (initRes.mode !== 'single') {
        throw new Error(`Expected single upload mode for ${file.size} bytes, got ${initRes.mode}`);
      }

      const headers: Record<string, string> = {};
      if (file.type) {
        headers['Content-Type'] = file.type;
      }

      try {
        const res = await fetch(initRes.upload_url, {
          method: 'PUT',
          body: file,
          headers,
          signal: controller.signal,
        });

        if (!res.ok) {
          throw new Error(`Single upload failed: HTTP ${res.status} ${res.statusText}`);
        }

        const elapsedSec = Math.max(0.1, (Date.now() - startTime) / 1000);
        const speed = Math.round(file.size / elapsedSec);
        uploadStore.updateProgress(item.id, 100, speed, file.size);
        uploadStore.markComplete(item.id);
        await bucketStore.refresh();
        return item;
      } catch (directErr) {
        if (controller.signal.aborted || (directErr instanceof DOMException && directErr.name === 'AbortError')) {
          throw directErr;
        }
        if (isCorsOrNetworkError(directErr)) {
          return await executeProxyFallback(item, file, profile, key, controller.signal);
        }
        throw directErr;
      }
    }

    // Large file multipart upload (>= 10MB)
    const initRes = await initUpload(profile, key, file.size, file.type || undefined);
    if (initRes.mode !== 'multipart') {
      throw new Error(`Expected multipart upload mode for ${file.size} bytes, got ${initRes.mode}`);
    }

    uploadStore.setUploadId(item.id, initRes.upload_id);

    const partSize = initRes.part_size || PART_SIZE;
    const manifest: UploadManifest = {
      uploadId: initRes.upload_id,
      key,
      profile,
      fileSize: file.size,
      partSize,
      completedParts: [],
      createdAt: Date.now(),
    };
    await saveSession(manifest);

    const completedParts: CompletedPart[] = [];
    let uploadedBytes = 0;

    try {
      await executePartsPool(
        file,
        initRes.parts,
        partSize,
        controller,
        async (partNum, etag, chunkSize) => {
          completedParts.push({ part_number: partNum, etag });
          await addCompletedPart(initRes.upload_id, { part_number: partNum, etag });

          uploadedBytes += chunkSize;
          const progress = Math.min(99, Math.round((uploadedBytes / file.size) * 100));
          const elapsedSec = Math.max(0.1, (Date.now() - startTime) / 1000);
          const speed = Math.round(uploadedBytes / elapsedSec);
          uploadStore.updateProgress(item.id, progress, speed, uploadedBytes);
        }
      );

      // All parts uploaded successfully
      completedParts.sort((a, b) => a.part_number - b.part_number);
      await completeUpload(profile, initRes.upload_id, completedParts, key);

      await deleteSession(initRes.upload_id);
      uploadStore.markComplete(item.id);
      await bucketStore.refresh();
      return item;
    } catch (directErr) {
      if (controller.signal.aborted || (directErr instanceof DOMException && directErr.name === 'AbortError')) {
        throw directErr;
      }
      if (isCorsOrNetworkError(directErr)) {
        await Promise.allSettled([
          abortUpload(profile, initRes.upload_id),
          deleteSession(initRes.upload_id),
        ]);

        return await executeProxyFallback(item, file, profile, key, controller.signal);
      }
      throw directErr;
    }
  } catch (err) {
    const entry = activeControllers.get(item.id);
    const isUserAborted = entry?.userAborted || false;

    if (isUserAborted) {
      uploadStore.markAborted(item.id);
    } else {
      const msg = formatUploadErrorMessage(err);
      uploadStore.markFailed(item.id, msg);
    }
    throw err;
  } finally {
    activeControllers.delete(item.id);
  }
}

/**
 * Resumes an interrupted multipart upload session using its persisted manifest.
 */
export async function resumeInterruptedUpload(
  file: File,
  manifest: UploadManifest
): Promise<UploadItem> {
  let item = uploadStore.items.find((i) => i.uploadId === manifest.uploadId);
  if (!item) {
    item = uploadStore.add({
      file,
      key: manifest.key,
      profile: manifest.profile,
      name: file.name,
    });
    uploadStore.setUploadId(item.id, manifest.uploadId);
  }

  const controller = new AbortController();
  activeControllers.set(item.id, { controller, userAborted: false });
  const startTime = Date.now();

  try {
    const resumeRes = await resumeUpload(manifest.profile, manifest.uploadId);
    const completedParts: CompletedPart[] = [...resumeRes.completed_parts];

    // Compute initial transferred bytes from already completed parts
    let uploadedBytes = 0;
    for (const cp of completedParts) {
      const start = (cp.part_number - 1) * manifest.partSize;
      const end = Math.min(manifest.fileSize, start + manifest.partSize);
      uploadedBytes += Math.max(0, end - start);
    }
    const initialUploadedBytes = uploadedBytes;

    const initialProgress = Math.min(99, Math.round((uploadedBytes / manifest.fileSize) * 100));
    uploadStore.updateProgress(item.id, initialProgress, 0, uploadedBytes);

    try {
      if (resumeRes.remaining_parts.length > 0) {
        await executePartsPool(
          file,
          resumeRes.remaining_parts,
          manifest.partSize,
          controller,
          async (partNum, etag, chunkSize) => {
            completedParts.push({ part_number: partNum, etag });
            await addCompletedPart(manifest.uploadId, { part_number: partNum, etag });

            uploadedBytes += chunkSize;
            const progress = Math.min(99, Math.round((uploadedBytes / manifest.fileSize) * 100));
            const elapsedSec = Math.max(0.1, (Date.now() - startTime) / 1000);
            const bytesTransferredThisSession = uploadedBytes - initialUploadedBytes;
            const speed = Math.round(bytesTransferredThisSession / elapsedSec);
            uploadStore.updateProgress(item.id, progress, speed, uploadedBytes);
          }
        );
      }

      completedParts.sort((a, b) => a.part_number - b.part_number);
      await completeUpload(manifest.profile, manifest.uploadId, completedParts, manifest.key);

      await deleteSession(manifest.uploadId);
      const elapsedSec = Math.max(0.1, (Date.now() - startTime) / 1000);
      const finalBytesThisSession = manifest.fileSize - initialUploadedBytes;
      const finalSpeed = Math.round(finalBytesThisSession / elapsedSec);
      uploadStore.updateProgress(item.id, 100, finalSpeed, manifest.fileSize);
      uploadStore.markComplete(item.id);
      await bucketStore.refresh();
      return item;
    } catch (directErr) {
      if (controller.signal.aborted || (directErr instanceof DOMException && directErr.name === 'AbortError')) {
        throw directErr;
      }
      if (isCorsOrNetworkError(directErr)) {
        await Promise.allSettled([
          abortUpload(manifest.profile, manifest.uploadId),
          deleteSession(manifest.uploadId),
        ]);

        return await executeProxyFallback(item, file, manifest.profile, manifest.key, controller.signal);
      }
      throw directErr;
    }
  } catch (err) {
    const entry = activeControllers.get(item.id);
    const isUserAborted = entry?.userAborted || false;

    if (isUserAborted) {
      uploadStore.markAborted(item.id);
    } else {
      const msg = formatUploadErrorMessage(err);
      uploadStore.markFailed(item.id, msg);
    }
    throw err;
  } finally {
    activeControllers.delete(item.id);
  }
}

/**
 * Uploads parts in parallel with a concurrency pool of up to MAX_CONCURRENCY.
 * If any part upload fails, aborts all sibling workers immediately via internal pool controller.
 */
async function executePartsPool(
  file: File | Blob,
  parts: PresignedPart[],
  partSize: number,
  controller: AbortController,
  onPartComplete: (partNumber: number, etag: string, chunkSize: number) => Promise<void>
): Promise<void> {
  let currentIndex = 0;
  let poolError: unknown = null;

  // Internal pool abort controller to cancel sibling workers without marking parent transfer as user aborted
  const poolAbortController = new AbortController();

  // Forward external parent abort (e.g. user cancellation) to the pool
  const onParentAbort = () => {
    poolAbortController.abort();
  };
  if (controller.signal.aborted) {
    poolAbortController.abort();
  } else {
    controller.signal.addEventListener('abort', onParentAbort, { once: true });
  }

  const workerCount = Math.min(MAX_CONCURRENCY, parts.length);
  const workers = Array.from({ length: workerCount }, async () => {
    while (currentIndex < parts.length && !poolError) {
      if (poolAbortController.signal.aborted) {
        throw new DOMException('Upload aborted', 'AbortError');
      }

      const index = currentIndex++;
      const part = parts[index];
      const start = (part.part_number - 1) * partSize;
      const end = Math.min(file.size, start + partSize);
      const chunk = file.slice(start, end);

      try {
        const etag = await uploadPartWithRetry(part.url, chunk, poolAbortController.signal);
        await onPartComplete(part.part_number, etag, chunk.size);
      } catch (err) {
        poolError = err;
        // Abort all in-flight sibling workers in this pool
        poolAbortController.abort();
        throw err;
      }
    }
  });

  try {
    await Promise.all(workers);
  } finally {
    controller.signal.removeEventListener('abort', onParentAbort);
  }
}
