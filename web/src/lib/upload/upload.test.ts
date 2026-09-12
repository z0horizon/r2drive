import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  cleanObjectKey,
  calculateChunkPlan,
  uploadPartWithRetry,
  cancelUpload,
  uploadFile,
  resumeInterruptedUpload,
  PART_SIZE,
  MAX_CONCURRENCY,
  MAX_RETRIES,
} from './worker';
import {
  saveSession,
  getSession,
  listSessions,
  deleteSession,
  addCompletedPart,
  type UploadManifest,
} from './indexeddb';
import { uploadStore } from '../stores/upload.svelte';
import { bucketStore } from '../stores/bucket.svelte';

describe('Object Key Sanitization (cleanObjectKey)', () => {
  it('handles empty prefix and root keys', () => {
    expect(cleanObjectKey('', 'file.txt')).toBe('file.txt');
    expect(cleanObjectKey('/', 'file.txt')).toBe('file.txt');
    expect(cleanObjectKey('///', 'file.txt')).toBe('file.txt');
  });

  it('normalizes single directory prefixes', () => {
    expect(cleanObjectKey('photos', 'cat.png')).toBe('photos/cat.png');
    expect(cleanObjectKey('photos/', 'cat.png')).toBe('photos/cat.png');
    expect(cleanObjectKey('/photos/', 'cat.png')).toBe('photos/cat.png');
  });

  it('collapses multiple consecutive slashes and strips leading slashes', () => {
    expect(cleanObjectKey('//nested///folder//', 'data.json')).toBe('nested/folder/data.json');
    expect(cleanObjectKey('a/b/c', 'file.tar.gz')).toBe('a/b/c/file.tar.gz');
  });

  it('handles special characters and spaces in filenames', () => {
    expect(cleanObjectKey('docs/', 'My Document (2026).pdf')).toBe('docs/My Document (2026).pdf');
  });
});

describe('Chunk Plan Calculation (calculateChunkPlan)', () => {
  it('returns empty array for zero or negative file size', () => {
    expect(calculateChunkPlan(0)).toEqual([]);
    expect(calculateChunkPlan(-100)).toEqual([]);
  });

  it('returns single chunk for file smaller than part size', () => {
    const size = 5 * 1024 * 1024; // 5MB
    const chunks = calculateChunkPlan(size, 10 * 1024 * 1024);
    expect(chunks).toHaveLength(1);
    expect(chunks[0]).toEqual({
      part_number: 1,
      start: 0,
      end: size,
      size,
    });
  });

  it('splits exact multiple of part size into equal chunks', () => {
    const partSize = 10 * 1024 * 1024; // 10MB
    const size = 20 * 1024 * 1024;     // 20MB
    const chunks = calculateChunkPlan(size, partSize);

    expect(chunks).toHaveLength(2);
    expect(chunks[0]).toEqual({
      part_number: 1,
      start: 0,
      end: 10 * 1024 * 1024,
      size: 10 * 1024 * 1024,
    });
    expect(chunks[1]).toEqual({
      part_number: 2,
      start: 10 * 1024 * 1024,
      end: 20 * 1024 * 1024,
      size: 10 * 1024 * 1024,
    });
  });

  it('handles trailing partial chunk for uneven sizes', () => {
    const partSize = 10 * 1024 * 1024; // 10MB
    const size = 25 * 1024 * 1024;     // 25MB
    const chunks = calculateChunkPlan(size, partSize);

    expect(chunks).toHaveLength(3);
    expect(chunks[0].size).toBe(partSize);
    expect(chunks[1].size).toBe(partSize);
    expect(chunks[2]).toEqual({
      part_number: 3,
      start: 20 * 1024 * 1024,
      end: 25 * 1024 * 1024,
      size: 5 * 1024 * 1024,
    });
  });

  it('uses default PART_SIZE, MAX_CONCURRENCY, and MAX_RETRIES constants', () => {
    expect(PART_SIZE).toBe(10485760);
    expect(MAX_CONCURRENCY).toBe(4);
    expect(MAX_RETRIES).toBe(5);
    const chunks = calculateChunkPlan(PART_SIZE * 2);
    expect(chunks).toHaveLength(2);
  });
});

describe('Upload Part Retry and Backoff (uploadPartWithRetry)', () => {
  const originalFetch = globalThis.fetch;

  beforeEach(() => {
    vi.restoreAllMocks();
  });

  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  it('returns ETag immediately on first successful PUT', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(
      new Response(null, {
        status: 200,
        headers: { ETag: '"part-1-etag"' },
      })
    );

    const blob = new Blob(['sample-chunk-bytes']);
    const etag = await uploadPartWithRetry('https://r2.example.com/part1', blob);

    expect(etag).toBe('"part-1-etag"');
    expect(globalThis.fetch).toHaveBeenCalledTimes(1);
    expect(globalThis.fetch).toHaveBeenCalledWith(
      'https://r2.example.com/part1',
      expect.objectContaining({
        method: 'PUT',
        body: blob,
      })
    );
  });

  it('handles lowercase etag response header', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(
      new Response(null, {
        status: 200,
        headers: { etag: '"lowercase-etag"' },
      })
    );

    const blob = new Blob(['chunk']);
    const etag = await uploadPartWithRetry('https://r2.example.com/part1', blob);
    expect(etag).toBe('"lowercase-etag"');
  });

  it('retries on HTTP 500 error and recovers on subsequent attempt', async () => {
    let callCount = 0;
    globalThis.fetch = vi.fn().mockImplementation(() => {
      callCount++;
      if (callCount < 3) {
        return Promise.resolve(new Response('Internal Server Error', { status: 500, statusText: 'Internal Server Error' }));
      }
      return Promise.resolve(new Response(null, { status: 200, headers: { ETag: '"recovered-etag"' } }));
    });

    const blob = new Blob(['data']);
    const etag = await uploadPartWithRetry(
      'https://r2.example.com/part1',
      blob,
      undefined,
      5,
      10 // 10ms base delay for fast tests
    );

    expect(callCount).toBe(3);
    expect(etag).toBe('"recovered-etag"');
  });

  it('retries on network fetch rejection and recovers', async () => {
    let callCount = 0;
    globalThis.fetch = vi.fn().mockImplementation(() => {
      callCount++;
      if (callCount === 1) {
        return Promise.reject(new TypeError('Failed to fetch'));
      }
      return Promise.resolve(new Response(null, { status: 200, headers: { ETag: '"recovered-after-net-err"' } }));
    });

    const blob = new Blob(['data']);
    const etag = await uploadPartWithRetry(
      'https://r2.example.com/part1',
      blob,
      undefined,
      3,
      10
    );

    expect(callCount).toBe(2);
    expect(etag).toBe('"recovered-after-net-err"');
  });

  it('fails after exhausting max retries', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(
      new Response('Gateway Timeout', { status: 504, statusText: 'Gateway Timeout' })
    );

    const blob = new Blob(['data']);
    await expect(
      uploadPartWithRetry('https://r2.example.com/part1', blob, undefined, 3, 5)
    ).rejects.toThrow(/Part upload failed after 3 attempts/);

    expect(globalThis.fetch).toHaveBeenCalledTimes(3);
  });

  it('aborts immediately without retrying when AbortSignal is triggered', async () => {
    const controller = new AbortController();
    controller.abort();

    const blob = new Blob(['data']);
    await expect(
      uploadPartWithRetry('https://r2.example.com/part1', blob, controller.signal, 5, 10)
    ).rejects.toThrow(/aborted/i);
  });
});

describe('IndexedDB Persistence Fallback & Mock (indexeddb.ts)', () => {
  it('safely handles non-browser environment when window.indexedDB is undefined', async () => {
    const sampleManifest: UploadManifest = {
      uploadId: 'up-100',
      key: 'test.mp4',
      profile: 'primary',
      fileSize: 20000000,
      partSize: 10485760,
      completedParts: [{ part_number: 1, etag: '"etag1"' }],
      createdAt: Date.now(),
    };

    // When IndexedDB is not available (Node.js default environment)
    await expect(saveSession(sampleManifest)).resolves.toBeUndefined();
    await expect(getSession('up-100')).resolves.toBeNull();
    await expect(listSessions()).resolves.toEqual([]);
    await expect(deleteSession('up-100')).resolves.toBeUndefined();
    await expect(addCompletedPart('up-100', { part_number: 2, etag: '"etag2"' })).resolves.toBeUndefined();
  });

  it('performs CRUD operations when IndexedDB is available', async () => {
    // In-memory mock IndexedDB store
    const store = new Map<string, UploadManifest>();

    const mockIDB = {
      open: vi.fn().mockImplementation(() => {
        const req: any = {
          result: {
            objectStoreNames: {
              contains: vi.fn().mockReturnValue(true),
            },
            transaction: vi.fn().mockImplementation(() => ({
              objectStore: vi.fn().mockImplementation(() => ({
                put: vi.fn().mockImplementation((manifest: UploadManifest) => {
                  store.set(manifest.uploadId, JSON.parse(JSON.stringify(manifest)));
                  const r: any = {};
                  setTimeout(() => r.onsuccess?.(), 0);
                  return r;
                }),
                get: vi.fn().mockImplementation((id: string) => {
                  const item = store.get(id);
                  const r: any = { result: item ? JSON.parse(JSON.stringify(item)) : undefined };
                  setTimeout(() => r.onsuccess?.(), 0);
                  return r;
                }),
                getAll: vi.fn().mockImplementation(() => {
                  const r: any = { result: Array.from(store.values()).map(v => JSON.parse(JSON.stringify(v))) };
                  setTimeout(() => r.onsuccess?.(), 0);
                  return r;
                }),
                delete: vi.fn().mockImplementation((id: string) => {
                  store.delete(id);
                  const r: any = {};
                  setTimeout(() => r.onsuccess?.(), 0);
                  return r;
                }),
              })),
            })),
          },
        };
        setTimeout(() => req.onsuccess?.(), 0);
        return req;
      }),
    };

    (globalThis as any).window = { indexedDB: mockIDB };

    try {
      const manifest: UploadManifest = {
        uploadId: 'test-upload-uuid',
        key: 'videos/recording.mp4',
        profile: 'primary',
        fileSize: 30000000,
        partSize: 10485760,
        completedParts: [{ part_number: 1, etag: '"part1"' }],
        createdAt: 1700000000000,
      };

      await saveSession(manifest);

      const retrieved = await getSession('test-upload-uuid');
      expect(retrieved).not.toBeNull();
      expect(retrieved?.uploadId).toBe('test-upload-uuid');
      expect(retrieved?.completedParts).toHaveLength(1);

      // Add completed part
      await addCompletedPart('test-upload-uuid', { part_number: 2, etag: '"part2"' });
      const updated = await getSession('test-upload-uuid');
      expect(updated?.completedParts).toHaveLength(2);
      expect(updated?.completedParts[1]).toEqual({ part_number: 2, etag: '"part2"' });

      // List sessions
      const all = await listSessions();
      expect(all).toHaveLength(1);
      expect(all[0].uploadId).toBe('test-upload-uuid');

      // Delete session
      await deleteSession('test-upload-uuid');
      const afterDelete = await getSession('test-upload-uuid');
      expect(afterDelete).toBeNull();
    } finally {
      delete (globalThis as any).window;
    }
  });
});

describe('Upload Engine Execution (worker.ts)', () => {
  const originalFetch = globalThis.fetch;

  beforeEach(() => {
    uploadStore.clearAll();
    vi.restoreAllMocks();
  });

  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  it('performs single PUT upload for small file (< 10MB)', async () => {
    const refreshSpy = vi.spyOn(bucketStore, 'refresh').mockResolvedValue(undefined);

    globalThis.fetch = vi.fn().mockImplementation((url: string) => {
      // 1. API call to /upload/init
      if (url.includes('/upload/init')) {
        return Promise.resolve(
          new Response(
            JSON.stringify({
              mode: 'single',
              upload_url: 'https://r2.presigned.put/small.txt',
            }),
            { status: 200, headers: { 'Content-Type': 'application/json' } }
          )
        );
      }
      // 2. Direct single PUT to presigned URL
      if (url === 'https://r2.presigned.put/small.txt') {
        return Promise.resolve(new Response(null, { status: 200 }));
      }
      return Promise.reject(new Error(`Unexpected URL: ${url}`));
    });

    const file = new File(['hello world'], 'small.txt', { type: 'text/plain' });
    const item = await uploadFile(file, 'primary', 'documents/');

    expect(item.status).toBe('completed');
    expect(item.progress).toBe(100);
    expect(item.key).toBe('documents/small.txt');
    expect(refreshSpy).toHaveBeenCalled();
  });

  it('performs multipart upload for large file (>= 10MB) with concurrency', async () => {
    const refreshSpy = vi.spyOn(bucketStore, 'refresh').mockResolvedValue(undefined);

    const fileSize = 25 * 1024 * 1024; // 25MB -> 3 parts
    // Create dummy blob with specified size
    const dummyBlob = new Blob([new Uint8Array(fileSize)]);
    const file = new File([dummyBlob], 'large.bin', { type: 'application/octet-stream' });

    const partsReceived: number[] = [];

    globalThis.fetch = vi.fn().mockImplementation((url: string) => {
      // Init multipart
      if (url.includes('/upload/init')) {
        return Promise.resolve(
          new Response(
            JSON.stringify({
              mode: 'multipart',
              upload_id: 'up-multipart-123',
              part_size: PART_SIZE,
              parts: [
                { part_number: 1, url: 'https://r2.presigned/part-1' },
                { part_number: 2, url: 'https://r2.presigned/part-2' },
                { part_number: 3, url: 'https://r2.presigned/part-3' },
              ],
            }),
            { status: 200, headers: { 'Content-Type': 'application/json' } }
          )
        );
      }

      // Part PUTs
      if (url.includes('/part-')) {
        const num = parseInt(url.split('/part-')[1], 10);
        partsReceived.push(num);
        return Promise.resolve(
          new Response(null, {
            status: 200,
            headers: { ETag: `"etag-${num}"` },
          })
        );
      }

      // Complete multipart
      if (url.includes('/upload/complete')) {
        return Promise.resolve(
          new Response(
            JSON.stringify({
              status: 'completed',
              etag: '"final-etag-789"',
            }),
            { status: 200, headers: { 'Content-Type': 'application/json' } }
          )
        );
      }

      return Promise.reject(new Error(`Unexpected URL: ${url}`));
    });

    const item = await uploadFile(file, 'primary', '');

    expect(item.status).toBe('completed');
    expect(item.progress).toBe(100);
    expect(partsReceived.sort()).toEqual([1, 2, 3]);
    expect(refreshSpy).toHaveBeenCalled();
  });

  it('supports cancellation and cleans up state', async () => {
    let abortResolver: (() => void) | null = null;
    const abortCalled = new Promise<void>((resolve) => {
      abortResolver = resolve;
    });

    globalThis.fetch = vi.fn().mockImplementation((url: string) => {
      if (url.includes('/upload/init')) {
        return Promise.resolve(
          new Response(
            JSON.stringify({
              mode: 'multipart',
              upload_id: 'up-abort-test',
              part_size: PART_SIZE,
              parts: [{ part_number: 1, url: 'https://r2.presigned/part-1' }],
            }),
            { status: 200, headers: { 'Content-Type': 'application/json' } }
          )
        );
      }

      if (url.includes('/part-1')) {
        // Pending part upload that will get aborted
        return new Promise((_resolve, reject) => {
          setTimeout(() => reject(new DOMException('Aborted', 'AbortError')), 50);
        });
      }

      if (url.includes('/upload/abort')) {
        abortResolver?.();
        return Promise.resolve(
          new Response(JSON.stringify({ status: 'aborted' }), {
            status: 200,
            headers: { 'Content-Type': 'application/json' },
          })
        );
      }

      return Promise.reject(new Error(`Unexpected URL: ${url}`));
    });

    const file = new File([new Blob([new Uint8Array(15 * 1024 * 1024)])], 'cancel-me.bin');
    const uploadPromise = uploadFile(file, 'primary', 'test/');

    // Wait a tick for init to finish and part upload to start
    await new Promise((r) => setTimeout(r, 10));

    const item = uploadStore.items[0];
    expect(item).toBeDefined();

    // Cancel the transfer
    await cancelUpload(item.id);
    await abortCalled;

    await expect(uploadPromise).rejects.toThrow();
    expect(item.status).toBe('aborted');
  });

  it('resumes interrupted upload with remaining parts', async () => {
    const refreshSpy = vi.spyOn(bucketStore, 'refresh').mockResolvedValue(undefined);

    const fileSize = 30 * 1024 * 1024; // 30MB -> 3 parts
    const file = new File([new Blob([new Uint8Array(fileSize)])], 'resume.bin');

    const manifest: UploadManifest = {
      uploadId: 'resumed-session-id',
      key: 'archives/resume.bin',
      profile: 'primary',
      fileSize,
      partSize: PART_SIZE,
      completedParts: [{ part_number: 1, etag: '"etag-part-1"' }],
      createdAt: Date.now() - 60000,
    };

    let completePayload: any = null;

    globalThis.fetch = vi.fn().mockImplementation((url: string, init?: RequestInit) => {
      // Resume endpoint returns completed part 1 and remaining parts 2 & 3
      if (url.includes('/upload/resume')) {
        return Promise.resolve(
          new Response(
            JSON.stringify({
              upload_id: 'resumed-session-id',
              completed_parts: [{ part_number: 1, etag: '"etag-part-1"' }],
              remaining_parts: [
                { part_number: 2, url: 'https://r2.presigned/part-2' },
                { part_number: 3, url: 'https://r2.presigned/part-3' },
              ],
            }),
            { status: 200, headers: { 'Content-Type': 'application/json' } }
          )
        );
      }

      if (url.includes('/part-2')) {
        return Promise.resolve(new Response(null, { status: 200, headers: { ETag: '"etag-part-2"' } }));
      }

      if (url.includes('/part-3')) {
        return Promise.resolve(new Response(null, { status: 200, headers: { ETag: '"etag-part-3"' } }));
      }

      if (url.includes('/upload/complete')) {
        completePayload = JSON.parse(init?.body as string);
        return Promise.resolve(
          new Response(JSON.stringify({ status: 'completed', etag: '"final-resume-etag"' }), {
            status: 200,
            headers: { 'Content-Type': 'application/json' },
          })
        );
      }

      return Promise.reject(new Error(`Unexpected URL: ${url}`));
    });

    const item = await resumeInterruptedUpload(file, manifest);

    expect(item.status).toBe('completed');
    expect(item.progress).toBe(100);
    expect(completePayload.upload_id).toBe('resumed-session-id');
    expect(completePayload.parts).toEqual([
      { part_number: 1, etag: '"etag-part-1"' },
      { part_number: 2, etag: '"etag-part-2"' },
      { part_number: 3, etag: '"etag-part-3"' },
    ]);
    expect(refreshSpy).toHaveBeenCalled();
  });
});
