import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { AuthStore, authStore } from './auth.svelte';
import { BucketStore, bucketStore } from './bucket.svelte';
import { UploadStore, uploadStore } from './upload.svelte';
import * as authApi from '../api/auth';
import * as objectsApi from '../api/objects';
import * as transfersApi from '../api/transfers';

describe('AuthStore (auth.svelte.ts)', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('initializes with default unauthenticated state', () => {
    const store = new AuthStore();
    expect(store.isAuthenticated).toBe(false);
    expect(store.loading).toBe(false);
    expect(store.error).toBeNull();
    expect(authStore).toBeInstanceOf(AuthStore);
    expect(bucketStore).toBeInstanceOf(BucketStore);
    expect(uploadStore).toBeInstanceOf(UploadStore);
  });

  it('check updates isAuthenticated state on success', async () => {
    const store = new AuthStore();
    vi.spyOn(authApi, 'checkAuth').mockResolvedValue(true);

    const result = await store.check();
    expect(result).toBe(true);
    expect(store.isAuthenticated).toBe(true);
    expect(store.loading).toBe(false);
  });

  it('check sets false on unauthenticated or failure', async () => {
    const store = new AuthStore();
    vi.spyOn(authApi, 'checkAuth').mockResolvedValue(false);

    const result = await store.check();
    expect(result).toBe(false);
    expect(store.isAuthenticated).toBe(false);
  });

  it('login authenticates and updates state', async () => {
    const store = new AuthStore();
    vi.spyOn(authApi, 'login').mockResolvedValue({ status: 'authenticated', expires_at: '2026-09-13T00:00:00Z' });

    const result = await store.login('password123');
    expect(result).toBe(true);
    expect(store.isAuthenticated).toBe(true);
  });

  it('login records error and throws on rejection', async () => {
    const store = new AuthStore();
    vi.spyOn(authApi, 'login').mockRejectedValue(new Error('Invalid password'));

    await expect(store.login('badpass')).rejects.toThrow('Invalid password');
    expect(store.isAuthenticated).toBe(false);
    expect(store.error).toBe('Invalid password');
    expect(store.loading).toBe(false);
  });

  it('logout resets isAuthenticated to false', async () => {
    const store = new AuthStore();
    store.isAuthenticated = true;
    vi.spyOn(authApi, 'logout').mockResolvedValue();

    await store.logout();
    expect(store.isAuthenticated).toBe(false);
    expect(store.loading).toBe(false);
  });
});

describe('BucketStore (bucket.svelte.ts)', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('computes breadcrumbs correctly for root and nested paths', () => {
    const store = new BucketStore();
    store.currentPrefix = '';
    expect(store.breadcrumbs).toEqual([{ label: 'Root', prefix: '' }]);

    store.currentPrefix = 'documents/reports/2026/';
    expect(store.breadcrumbs).toEqual([
      { label: 'Root', prefix: '' },
      { label: 'documents', prefix: 'documents/' },
      { label: 'reports', prefix: 'documents/reports/' },
      { label: '2026', prefix: 'documents/reports/2026/' },
    ]);
  });

  it('loadProfiles selects default profile and fetches contents', async () => {
    const store = new BucketStore();
    const profiles = [
      { name: 'backup', bucket: 'backup-bucket', default: false },
      { name: 'primary', bucket: 'main-bucket', default: true },
    ];
    vi.spyOn(objectsApi, 'listBuckets').mockResolvedValue(profiles);
    const listSpy = vi.spyOn(objectsApi, 'listObjects').mockResolvedValue({
      prefix: '',
      directories: ['photos/'],
      objects: [{ key: 'file.txt', name: 'file.txt', size_bytes: 123, last_modified: '2026-09-12T00:00:00Z' }],
      synced_at: '2026-09-12T00:00:00Z',
    });

    await store.loadProfiles();
    expect(store.profiles).toEqual(profiles);
    expect(store.selectedProfile).toBe('primary');
    expect(listSpy).toHaveBeenCalledWith('primary', '', false);
    expect(store.directories).toEqual(['photos/']);
    expect(store.objects.length).toBe(1);
  });

  it('setProfile updates profile and resets prefix', async () => {
    const store = new BucketStore();
    store.currentPrefix = 'old/prefix/';
    const listSpy = vi.spyOn(objectsApi, 'listObjects').mockResolvedValue({
      prefix: '',
      directories: [],
      objects: [],
      synced_at: '2026-09-12T00:00:00Z',
    });

    await store.setProfile('secondary');
    expect(store.selectedProfile).toBe('secondary');
    expect(store.currentPrefix).toBe('');
    expect(listSpy).toHaveBeenCalledWith('secondary', '', false);
  });

  it('setPrefix normalizes leading slashes and appends trailing slash', async () => {
    const store = new BucketStore();
    store.selectedProfile = 'primary';
    const listSpy = vi.spyOn(objectsApi, 'listObjects').mockResolvedValue({
      prefix: 'nested/dir/',
      directories: [],
      objects: [],
      synced_at: '2026-09-12T00:00:00Z',
    });

    await store.setPrefix('/nested/dir');
    expect(store.currentPrefix).toBe('nested/dir/');
    expect(listSpy).toHaveBeenCalledWith('primary', 'nested/dir/', false);
  });

  it('refresh forces backend cache sync', async () => {
    const store = new BucketStore();
    store.selectedProfile = 'primary';
    store.currentPrefix = 'data/';
    const listSpy = vi.spyOn(objectsApi, 'listObjects').mockResolvedValue({
      prefix: 'data/',
      directories: [],
      objects: [],
      synced_at: '2026-09-12T00:00:00Z',
    });

    await store.refresh();
    expect(listSpy).toHaveBeenCalledWith('primary', 'data/', true);
  });

  it('deleteItem removes object from local state', async () => {
    const store = new BucketStore();
    store.selectedProfile = 'primary';
    store.objects = [
      { key: 'keep.txt', name: 'keep.txt', size_bytes: 10, last_modified: '2026-09-12T00:00:00Z' },
      { key: 'remove.txt', name: 'remove.txt', size_bytes: 20, last_modified: '2026-09-12T00:00:00Z' },
    ];
    vi.spyOn(objectsApi, 'deleteObject').mockResolvedValue();

    await store.deleteItem('remove.txt');
    expect(store.objects.map((o) => o.key)).toEqual(['keep.txt']);
  });

  it('reset clears all bucket store state', () => {
    const store = new BucketStore();
    store.profiles = [{ name: 'primary', bucket: 'b', default: true }];
    store.selectedProfile = 'primary';
    store.currentPrefix = 'photos/';
    store.directories = ['photos/vacation/'];
    store.objects = [
      { key: 'photos/img.jpg', name: 'img.jpg', size_bytes: 100, last_modified: '2026-09-12T00:00:00Z' },
    ];
    store.loading = true;
    store.corsStatus = 'healthy';
    store.serverFallbackPolicy = { enabled: false, max_payload_bytes: 1048576 };

    store.reset();
    expect(store.profiles).toEqual([]);
    expect(store.selectedProfile).toBe('');
    expect(store.currentPrefix).toBe('');
    expect(store.directories).toEqual([]);
    expect(store.objects).toEqual([]);
    expect(store.loading).toBe(false);
    expect(store.error).toBeNull();
    expect(store.corsStatus).toBe('unknown');
    expect(store.serverFallbackPolicy).toEqual({
      enabled: true,
      max_payload_bytes: 5 * 1024 * 1024 * 1024,
    });
  });

  it('initializes corsStatus to unknown', () => {
    const store = new BucketStore();
    expect(store.corsStatus).toBe('unknown');
    expect(bucketStore.corsStatus).toBe('unknown');
  });

  it('checkCors sets corsStatus to checking then transitions to healthy on 200/204 OPTIONS response', async () => {
    const store = new BucketStore();
    store.selectedProfile = 'primary';
    vi.spyOn(transfersApi, 'getCorsProbe').mockResolvedValue({
      probe_url: 'https://probe.r2.test/bucket/.r2drive-probe',
      fallback_policy: { enabled: true, max_payload_bytes: 5368709120 },
    });

    let fetchResolve: (value: Response) => void;
    const fetchPromise = new Promise<Response>((resolve) => {
      fetchResolve = resolve;
    });
    const fetchSpy = vi.spyOn(globalThis, 'fetch').mockImplementation(() => fetchPromise);

    const checkPromise = store.checkCors('primary');
    expect(store.corsStatus).toBe('checking');

    fetchResolve!(new Response(null, { status: 200 }));
    await checkPromise;
    expect(store.corsStatus).toBe('healthy');
    expect(fetchSpy).toHaveBeenCalledWith('https://probe.r2.test/bucket/.r2drive-probe', {
      method: 'OPTIONS',
      headers: {
        'Access-Control-Request-Method': 'PUT',
      },
    });

    // Also test 204 No Content response
    store.corsStatus = 'unknown';
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(new Response(null, { status: 204 }));
    await store.checkCors('primary');
    expect(store.corsStatus).toBe('healthy');
  });

  it('checkCors updates serverFallbackPolicy when probe returns fallback_policy', async () => {
    const store = new BucketStore();
    store.selectedProfile = 'primary';
    vi.spyOn(transfersApi, 'getCorsProbe').mockResolvedValue({
      probe_url: 'https://probe.r2.test/bucket/.r2drive-probe',
      fallback_policy: {
        enabled: false,
        max_payload_bytes: 104857600,
      },
    });
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(new Response(null, { status: 200 }));

    await store.checkCors('primary');
    expect(store.serverFallbackPolicy).toEqual({
      enabled: false,
      max_payload_bytes: 104857600,
    });
  });

  it('checkCors transitions to blocked on 403 or fetch network failure', async () => {
    const store = new BucketStore();
    store.selectedProfile = 'primary';
    vi.spyOn(transfersApi, 'getCorsProbe').mockResolvedValue({
      probe_url: 'https://probe.r2.test/bucket/.r2drive-probe',
      fallback_policy: { enabled: true, max_payload_bytes: 5368709120 },
    });

    // 403 Forbidden
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(new Response(null, { status: 403 }));
    await store.checkCors('primary');
    expect(store.corsStatus).toBe('blocked');

    // Network error / fetch rejection
    store.corsStatus = 'unknown';
    vi.spyOn(globalThis, 'fetch').mockRejectedValue(new TypeError('Failed to fetch'));
    await store.checkCors('primary');
    expect(store.corsStatus).toBe('blocked');
  });

  it('backend probe API error leaves corsStatus as unknown rather than false-positive blocked', async () => {
    const store = new BucketStore();
    store.selectedProfile = 'primary';
    vi.spyOn(transfersApi, 'getCorsProbe').mockRejectedValue(new Error('Probe URL API failed'));
    await store.checkCors('primary');
    expect(store.corsStatus).toBe('unknown');
  });

  it('checkCors respects caching when force = false', async () => {
    const store = new BucketStore();
    store.selectedProfile = 'primary';
    const probeSpy = vi.spyOn(transfersApi, 'getCorsProbe').mockResolvedValue({
      probe_url: 'https://probe.r2.test',
      fallback_policy: { enabled: true, max_payload_bytes: 5368709120 },
    });
    const fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValue(new Response(null, { status: 200 }));

    store.corsStatus = 'healthy';
    await store.checkCors('primary', false);
    expect(probeSpy).not.toHaveBeenCalled();
    expect(fetchSpy).not.toHaveBeenCalled();

    store.corsStatus = 'blocked';
    await store.checkCors('primary', false);
    expect(probeSpy).not.toHaveBeenCalled();
    expect(fetchSpy).not.toHaveBeenCalled();

    // When force = true, it should bypass cache
    await store.checkCors('primary', true);
    expect(probeSpy).toHaveBeenCalledWith('primary');
    expect(fetchSpy).toHaveBeenCalledTimes(1);
    expect(store.corsStatus).toBe('healthy');
  });

  it('changing bucket profile re-checks or resets corsStatus', async () => {
    const store = new BucketStore();
    vi.spyOn(objectsApi, 'listObjects').mockResolvedValue({
      prefix: '',
      directories: [],
      objects: [],
      synced_at: '2026-09-12T00:00:00Z',
    });
    const checkSpy = vi.spyOn(store, 'checkCors').mockResolvedValue();

    store.corsStatus = 'healthy';
    await store.setProfile('secondary');

    expect(store.selectedProfile).toBe('secondary');
    expect(checkSpy).toHaveBeenCalledWith('secondary');
  });

  it('checkCors defaults to selectedProfile if profile argument is omitted', async () => {
    const store = new BucketStore();
    store.selectedProfile = 'current-profile';
    const probeSpy = vi.spyOn(transfersApi, 'getCorsProbe').mockResolvedValue({
      probe_url: 'https://probe.r2.test',
      fallback_policy: { enabled: true, max_payload_bytes: 5368709120 },
    });
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(new Response(null, { status: 200 }));

    await store.checkCors();
    expect(probeSpy).toHaveBeenCalledWith('current-profile');
    expect(store.corsStatus).toBe('healthy');
  });

  it('does not overwrite corsStatus if selectedProfile changed while probe was in-flight', async () => {
    const store = new BucketStore();
    store.selectedProfile = 'bucket-a';

    let fetchResolve: (value: Response) => void;
    const fetchPromise = new Promise<Response>((resolve) => {
      fetchResolve = resolve;
    });
    vi.spyOn(transfersApi, 'getCorsProbe').mockResolvedValue({
      probe_url: 'https://probe.r2.test/bucket-a/.r2drive-probe',
      fallback_policy: { enabled: true, max_payload_bytes: 5368709120 },
    });
    vi.spyOn(globalThis, 'fetch').mockImplementation(() => fetchPromise);

    // Start probe for bucket-a
    const probePromise = store.checkCors('bucket-a');
    expect(store.corsStatus).toBe('checking');

    // While probe for bucket-a is in flight, user switches to bucket-b
    store.selectedProfile = 'bucket-b';
    store.corsStatus = 'unknown';

    // Now bucket-a probe resolves with healthy OPTIONS response
    fetchResolve!(new Response(null, { status: 200 }));
    await probePromise;

    // store.corsStatus should NOT have been overwritten by bucket-a probe!
    expect(store.corsStatus).toBe('unknown');
  });

  it('does not overwrite corsStatus if selectedProfile changed while getCorsProbe was pending', async () => {
    const store = new BucketStore();
    store.selectedProfile = 'bucket-a';

    let probeResolve: (value: transfersApi.CorsProbeResponse) => void;
    const probePromise = new Promise<transfersApi.CorsProbeResponse>((resolve) => {
      probeResolve = resolve;
    });
    vi.spyOn(transfersApi, 'getCorsProbe').mockReturnValue(probePromise);
    const fetchSpy = vi.spyOn(globalThis, 'fetch');

    const checkPromise = store.checkCors('bucket-a');
    expect(store.corsStatus).toBe('checking');

    // Switch profile before probe URL resolves
    store.selectedProfile = 'bucket-b';
    store.corsStatus = 'unknown';

    probeResolve!({
      probe_url: 'https://probe.r2.test/bucket-a/.r2drive-probe',
      fallback_policy: { enabled: false, max_payload_bytes: 12345 },
    });
    await checkPromise;

    expect(fetchSpy).not.toHaveBeenCalled();
    expect(store.corsStatus).toBe('unknown');
    expect(store.serverFallbackPolicy).toEqual({
      enabled: true,
      max_payload_bytes: 5 * 1024 * 1024 * 1024,
    });
  });
});

describe('UploadStore (upload.svelte.ts)', () => {
  const storageMap = new Map<string, string>();
  const mockLocalStorage = {
    getItem: vi.fn((key: string) => storageMap.get(key) ?? null),
    setItem: vi.fn((key: string, val: string) => {
      storageMap.set(key, val);
    }),
    removeItem: vi.fn((key: string) => {
      storageMap.delete(key);
    }),
    clear: vi.fn(() => {
      storageMap.clear();
    }),
  };

  beforeEach(() => {
    vi.restoreAllMocks();
    storageMap.clear();
    vi.stubGlobal('localStorage', mockLocalStorage);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('adds items and initializes queued state', () => {
    const store = new UploadStore();
    const fakeFile = new File(['hello world'], 'hello.txt', { type: 'text/plain' });

    const item = store.add({
      file: fakeFile,
      key: 'uploads/hello.txt',
      profile: 'primary',
    });

    expect(item.name).toBe('hello.txt');
    expect(item.key).toBe('uploads/hello.txt');
    expect(item.totalBytes).toBe(11);
    expect(item.status).toBe('queued');
    expect(item.progress).toBe(0);
    expect(store.items.length).toBe(1);
  });

  it('updates progress and transitions queued to uploading', () => {
    const store = new UploadStore();
    const fakeFile = new File([new ArrayBuffer(1000)], 'data.bin');
    const item = store.add({ file: fakeFile, key: 'data.bin', profile: 'primary' });

    store.updateProgress(item.id, 50, 102400, 500);

    const updated = store.items[0];
    expect(updated.status).toBe('uploading');
    expect(updated.progress).toBe(50);
    expect(updated.speed).toBe(102400);
    expect(updated.uploadedBytes).toBe(500);
    expect(store.isUploading).toBe(true);
  });

  it('updateProgress ignores items in terminal states', () => {
    const store = new UploadStore();
    const fakeFile = new File(['abc'], 'test.txt');
    const item = store.add({ file: fakeFile, key: 'test.txt', profile: 'primary' });

    store.markComplete(item.id);
    store.updateProgress(item.id, 50, 100, 50);
    expect(item.status).toBe('completed');
    expect(item.progress).toBe(100);

    store.markFailed(item.id, 'Failure');
    store.updateProgress(item.id, 20, 100, 20);
    expect(item.status).toBe('failed');
    expect(item.error).toBe('Failure');

    store.markAborted(item.id);
    store.updateProgress(item.id, 10, 100, 10);
    expect(item.status).toBe('aborted');
  });

  it('marks upload complete, failed, and aborted', () => {
    const store = new UploadStore();
    const f1 = new File([new ArrayBuffer(100)], 'f1.txt');
    const f2 = new File([new ArrayBuffer(200)], 'f2.txt');
    const f3 = new File([new ArrayBuffer(300)], 'f3.txt');

    const i1 = store.add({ file: f1, key: 'f1.txt', profile: 'primary' });
    const i2 = store.add({ file: f2, key: 'f2.txt', profile: 'primary' });
    const i3 = store.add({ file: f3, key: 'f3.txt', profile: 'primary' });

    store.markComplete(i1.id);
    expect(store.items[0].status).toBe('completed');
    expect(store.items[0].progress).toBe(100);
    expect(store.completedUploads.length).toBe(1);

    store.markFailed(i2.id, 'Network timeout');
    expect(store.items[1].status).toBe('failed');
    expect(store.items[1].error).toBe('Network timeout');
    expect(store.failedUploads.length).toBe(1);

    store.markAborted(i3.id);
    expect(store.items[2].status).toBe('aborted');
  });

  it('computes overall progress correctly', () => {
    const store = new UploadStore();
    const f1 = new File([new ArrayBuffer(100)], 'f1.txt');
    const f2 = new File([new ArrayBuffer(300)], 'f2.txt');

    const i1 = store.add({ file: f1, key: 'f1.txt', profile: 'primary' });
    const i2 = store.add({ file: f2, key: 'f2.txt', profile: 'primary' });

    // Total 400 bytes. i1: 100 bytes (100%), i2: 100 bytes (33.3%) -> total 200/400 = 50%
    store.updateProgress(i1.id, 100, 0, 100);
    store.updateProgress(i2.id, 33, 0, 100);

    expect(store.overallProgress).toBe(50);
  });

  it('clearCompleted removes completed items and preserves active ones', () => {
    const store = new UploadStore();
    const f1 = new File(['a'], 'f1.txt');
    const f2 = new File(['b'], 'f2.txt');
    const i1 = store.add({ file: f1, key: 'f1.txt', profile: 'primary' });
    const i2 = store.add({ file: f2, key: 'f2.txt', profile: 'primary' });

    store.markComplete(i1.id);
    expect(store.items.length).toBe(2);

    store.clearCompleted();
    expect(store.items.length).toBe(1);
    expect(store.items[0].id).toBe(i2.id);
  });

  it('initializes proxyFallbackPreference from localStorage or defaults to true', () => {
    localStorage.removeItem('r2drive_proxy_fallback_enabled');
    const store1 = new UploadStore();
    expect(store1.proxyFallbackPreference).toBe(true);

    localStorage.setItem('r2drive_proxy_fallback_enabled', 'false');
    const store2 = new UploadStore();
    expect(store2.proxyFallbackPreference).toBe(false);

    store2.setProxyFallbackPreference(true);
    expect(store2.proxyFallbackPreference).toBe(true);
    expect(localStorage.getItem('r2drive_proxy_fallback_enabled')).toBe('true');
    localStorage.removeItem('r2drive_proxy_fallback_enabled');
  });

  it('handles localStorage SecurityError gracefully and defaults to true', () => {
    vi.stubGlobal('localStorage', {
      getItem: vi.fn(() => {
        throw new DOMException('The operation is insecure', 'SecurityError');
      }),
      setItem: vi.fn(() => {
        throw new DOMException('The operation is insecure', 'SecurityError');
      }),
      removeItem: vi.fn(),
      clear: vi.fn(),
    });

    const store = new UploadStore();
    expect(store.proxyFallbackPreference).toBe(true);

    // setProxyFallbackPreference should also not throw when setItem throws
    expect(() => store.setProxyFallbackPreference(false)).not.toThrow();
    expect(store.proxyFallbackPreference).toBe(false);
  });
});
