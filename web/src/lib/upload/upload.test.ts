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
        return Promise.reject(new Error('connection reset by peer'));
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

  it('throws immediately without retrying when ETag header is missing or empty', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(
      new Response(null, {
        status: 200,
        headers: {}, // No ETag header
      })
    );

    const blob = new Blob(['data']);
    await expect(
      uploadPartWithRetry('https://r2.example.com/part1', blob, undefined, 5, 10)
    ).rejects.toThrow(/Upload response missing ETag header/);

    expect(globalThis.fetch).toHaveBeenCalledTimes(1);
  });

  it('throws immediately on permanent 400 or 403 error without retrying', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(
      new Response('Access Denied', { status: 403, statusText: 'Forbidden' })
    );

    const blob = new Blob(['data']);
    await expect(
      uploadPartWithRetry('https://r2.example.com/part1', blob, undefined, 5, 10)
    ).rejects.toThrow(/HTTP 403: Forbidden/);

    expect(globalThis.fetch).toHaveBeenCalledTimes(1);
  });

  it('throws immediately on CORS / fetch failure error without retrying', async () => {
    globalThis.fetch = vi.fn().mockRejectedValue(new TypeError('Failed to fetch'));

    const blob = new Blob(['data']);
    await expect(
      uploadPartWithRetry('https://r2.example.com/part1', blob, undefined, 5, 10)
    ).rejects.toThrow(/Failed to fetch/);

    expect(globalThis.fetch).toHaveBeenCalledTimes(1);
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

      // Add completed part concurrently
      await Promise.all([
        addCompletedPart('test-upload-uuid', { part_number: 3, etag: '"part3"' }),
        addCompletedPart('test-upload-uuid', { part_number: 4, etag: '"part4"' }),
        addCompletedPart('test-upload-uuid', { part_number: 2, etag: '"part2"' }),
      ]);
      const updated = await getSession('test-upload-uuid');
      expect(updated?.completedParts).toHaveLength(4);
      expect(updated?.completedParts.map((p) => p.part_number)).toEqual([1, 2, 3, 4]);

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

  it('aborts sibling workers when a chunk upload encounters fatal failure', async () => {
    const fileSize = 25 * 1024 * 1024; // 25MB -> 3 parts
    const file = new File([new Blob([new Uint8Array(fileSize)])], 'fail-fast.bin');

    let part2Started = false;
    let part2Aborted = false;

    globalThis.fetch = vi.fn().mockImplementation((url: string, init?: RequestInit) => {
      if (url.includes('/upload/init')) {
        return Promise.resolve(
          new Response(
            JSON.stringify({
              mode: 'multipart',
              upload_id: 'up-fail-fast',
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

      if (url.includes('/part-1')) {
        // Immediate fatal 403 Forbidden
        return Promise.resolve(
          new Response('Forbidden', { status: 403, statusText: 'Forbidden' })
        );
      }

      if (url.includes('/part-2')) {
        part2Started = true;
        return new Promise((_resolve, reject) => {
          init?.signal?.addEventListener('abort', () => {
            part2Aborted = true;
            reject(new DOMException('Aborted', 'AbortError'));
          });
        });
      }

      return Promise.reject(new Error(`Unexpected URL: ${url}`));
    });

    await expect(uploadFile(file, 'primary', '')).rejects.toThrow(/HTTP 403: Forbidden/);
    expect(part2Started).toBe(true);
    expect(part2Aborted).toBe(true);

    const item = uploadStore.items[0];
    expect(item).toBeDefined();
    expect(item.status).toBe('failed');
    expect(item.error).toContain('HTTP 403: Forbidden');
  });

  describe('WebConsole Resilient Upload Fallback & Seamless Retry', () => {
    class MockXHR {
      static instances: MockXHR[] = [];
      static onSend: ((xhr: MockXHR, body?: any) => void) | null = null;

      open = vi.fn();
      setRequestHeader = vi.fn();
      send = vi.fn((body?: any) => {
        if (MockXHR.onSend) {
          MockXHR.onSend(this, body);
        }
      });
      abort = vi.fn(() => {
        if (this.onabort) this.onabort();
      });
      withCredentials = false;
      status = 200;
      responseText = '';
      upload = {
        onprogress: null as ((e: any) => void) | null,
      };
      onload: (() => void) | null = null;
      onerror: (() => void) | null = null;
      onabort: (() => void) | null = null;

      constructor() {
        MockXHR.instances.push(this);
      }
    }

    beforeEach(() => {
      MockXHR.instances = [];
      MockXHR.onSend = null;
      (globalThis as any).XMLHttpRequest = MockXHR;
      uploadStore.clearAll();
      bucketStore.corsStatus = 'unknown';
    });

    afterEach(() => {
      delete (globalThis as any).XMLHttpRequest;
    });

    it('falls back to uploadViaProxy when single upload direct PUT throws CORS TypeError', async () => {
      const fileSize = 1024; // 1KB < 10MB -> single PUT
      const file = new File([new Uint8Array(fileSize)], 'cors-file.txt', { type: 'text/plain' });
      const refreshSpy = vi.spyOn(bucketStore, 'refresh').mockResolvedValue();

      globalThis.fetch = vi.fn().mockImplementation((url: string) => {
        if (url.includes('/upload/init')) {
          return Promise.resolve(
            new Response(
              JSON.stringify({
                mode: 'single',
                upload_url: 'https://r2.direct/cors-file.txt',
              }),
              { status: 200, headers: { 'Content-Type': 'application/json' } }
            )
          );
        }

        if (url.includes('https://r2.direct/cors-file.txt')) {
          // Browser throws TypeError: Failed to fetch on CORS block
          return Promise.reject(new TypeError('Failed to fetch'));
        }

        return Promise.reject(new Error(`Unexpected URL: ${url}`));
      });

      MockXHR.onSend = (xhr) => {
        xhr.upload?.onprogress?.({ lengthComputable: true, loaded: fileSize, total: fileSize });
        xhr.status = 200;
        xhr.responseText = JSON.stringify({
          status: 'uploaded',
          key: 'cors-file.txt',
          size_bytes: fileSize,
          etag: '"proxy-etag-123"',
        });
        xhr.onload?.();
      };

      const item = await uploadFile(file, 'primary', '');

      expect(MockXHR.instances).toHaveLength(1);
      const xhr = MockXHR.instances[0];
      expect(xhr.open).toHaveBeenCalledWith(
        'POST',
        '/api/buckets/primary/upload/proxy?key=cors-file.txt'
      );
      expect(bucketStore.corsStatus).toBe('blocked');
      expect(item.fallback).toBe(true);
      expect(item.status).toBe('completed');
      expect(item.progress).toBe(100);
      expect(refreshSpy).toHaveBeenCalled();
    });

    it('falls back to uploadViaProxy when multipart upload encounters CORS TypeError', async () => {
      const fileSize = 20 * 1024 * 1024; // 20MB -> multipart
      const file = new File([new Uint8Array(fileSize)], 'large-cors.bin', { type: 'application/octet-stream' });
      const refreshSpy = vi.spyOn(bucketStore, 'refresh').mockResolvedValue();
      let abortedMultipartId = '';

      globalThis.fetch = vi.fn().mockImplementation((url: string, init?: RequestInit) => {
        if (url.includes('/upload/init')) {
          return Promise.resolve(
            new Response(
              JSON.stringify({
                mode: 'multipart',
                upload_id: 'mp-cors-session',
                part_size: PART_SIZE,
                parts: [
                  { part_number: 1, url: 'https://r2.presigned/part-1' },
                  { part_number: 2, url: 'https://r2.presigned/part-2' },
                ],
              }),
              { status: 200, headers: { 'Content-Type': 'application/json' } }
            )
          );
        }

        if (url.includes('/part-1') || url.includes('/part-2')) {
          // Direct part upload blocked by CORS
          return Promise.reject(new TypeError('Failed to fetch'));
        }

        if (url.includes('/upload/abort')) {
          const body = JSON.parse(init?.body as string);
          abortedMultipartId = body.upload_id;
          return Promise.resolve(
            new Response(JSON.stringify({ status: 'aborted' }), {
              status: 200,
              headers: { 'Content-Type': 'application/json' },
            })
          );
        }

        return Promise.reject(new Error(`Unexpected URL: ${url}`));
      });

      MockXHR.onSend = (xhr) => {
        xhr.upload?.onprogress?.({ lengthComputable: true, loaded: fileSize, total: fileSize });
        xhr.status = 200;
        xhr.responseText = JSON.stringify({
          status: 'uploaded',
          key: 'large-cors.bin',
          size_bytes: fileSize,
          etag: '"proxy-etag-large"',
        });
        xhr.onload?.();
      };

      const item = await uploadFile(file, 'primary', '');

      expect(MockXHR.instances).toHaveLength(1);
      const xhr = MockXHR.instances[0];
      expect(xhr.open).toHaveBeenCalledWith(
        'POST',
        '/api/buckets/primary/upload/proxy?key=large-cors.bin'
      );
      expect(bucketStore.corsStatus).toBe('blocked');
      expect(item.fallback).toBe(true);
      expect(item.status).toBe('completed');
      expect(item.progress).toBe(100);
      expect(abortedMultipartId).toBe('mp-cors-session');
      expect(refreshSpy).toHaveBeenCalled();
    });

    it('skips direct presigned upload and fast-paths directly to proxy fallback when corsStatus is blocked', async () => {
      bucketStore.corsStatus = 'blocked';
      const fileSize = 2048;
      const file = new File([new Uint8Array(fileSize)], 'already-blocked.txt', { type: 'text/plain' });
      const fetchSpy = vi.fn();
      globalThis.fetch = fetchSpy;

      MockXHR.onSend = (xhr) => {
        xhr.upload?.onprogress?.({ lengthComputable: true, loaded: fileSize, total: fileSize });
        xhr.status = 200;
        xhr.responseText = JSON.stringify({
          status: 'uploaded',
          key: 'already-blocked.txt',
          size_bytes: fileSize,
          etag: '"proxy-etag-fastpath"',
        });
        xhr.onload?.();
      };

      const item = await uploadFile(file, 'primary', '');

      // Direct presigned init / PUT was never called because status was already blocked
      expect(fetchSpy).not.toHaveBeenCalled();
      expect(MockXHR.instances).toHaveLength(1);
      expect(item.fallback).toBe(true);
      expect(item.status).toBe('completed');
      expect(item.progress).toBe(100);
    });

    it('uploadStore.setFallback flags fallback and clears uploadId', () => {
      const item = uploadStore.add({
        file: new Blob(['hello']),
        key: 'doc.txt',
        profile: 'primary',
      });
      uploadStore.setUploadId(item.id, 'mp-session-to-clear');
      expect(item.uploadId).toBe('mp-session-to-clear');
      expect(item.fallback).toBeFalsy();

      uploadStore.setFallback(item.id);
      expect(item.fallback).toBe(true);
      expect(item.uploadId).toBeUndefined();
    });
  });
});
