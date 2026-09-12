import { describe, it, expect, vi, beforeEach } from 'vitest';
import { AuthStore, authStore } from './auth.svelte';
import { BucketStore, bucketStore } from './bucket.svelte';
import { UploadStore, uploadStore } from './upload.svelte';
import * as authApi from '../api/auth';
import * as objectsApi from '../api/objects';

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

  it('setPrefix normalizes leading slashes and fetches objects', async () => {
    const store = new BucketStore();
    store.selectedProfile = 'primary';
    const listSpy = vi.spyOn(objectsApi, 'listObjects').mockResolvedValue({
      prefix: 'nested/dir/',
      directories: [],
      objects: [],
      synced_at: '2026-09-12T00:00:00Z',
    });

    await store.setPrefix('/nested/dir/');
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
});

describe('UploadStore (upload.svelte.ts)', () => {
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
});
