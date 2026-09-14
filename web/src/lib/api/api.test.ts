import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { apiRequest, ApiError } from './client';
import { login, logout, checkAuth } from './auth';
import { listBuckets, listObjects, deleteObject, getDownloadUrl } from './objects';
import { initUpload, resumeUpload, completeUpload, abortUpload, getCorsProbeUrl } from './transfers';

describe('API Client (client.ts)', () => {
  const originalFetch = globalThis.fetch;

  beforeEach(() => {
    vi.restoreAllMocks();
  });

  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  it('performs successful GET request with same-origin credentials and JSON parse', async () => {
    const mockData = { message: 'hello' };
    globalThis.fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify(mockData), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      })
    );

    const res = await apiRequest<{ message: string }>('/api/test');
    expect(res).toEqual(mockData);
    expect(globalThis.fetch).toHaveBeenCalledWith('/api/test', expect.objectContaining({
      credentials: 'same-origin',
    }));
  });

  it('automatically adds Content-Type application/json for JSON payload string', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ ok: true }), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      })
    );

    await apiRequest('/api/post-test', {
      method: 'POST',
      body: JSON.stringify({ key: 'val' }),
    });

    const calledHeaders = (globalThis.fetch as any).mock.calls[0][1].headers;
    expect(calledHeaders.get('Content-Type')).toBe('application/json');
  });

  it('handles 204 No Content response gracefully', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(
      new Response(null, {
        status: 204,
      })
    );

    const res = await apiRequest<void>('/api/empty');
    expect(res).toBeUndefined();
  });

  it('throws typed ApiError with backend error message on failure', async () => {
    globalThis.fetch = vi.fn().mockImplementation(() =>
      Promise.resolve(
        new Response(JSON.stringify({ error: 'Unauthorized access' }), {
          status: 401,
          statusText: 'Unauthorized',
          headers: { 'Content-Type': 'application/json' },
        })
      )
    );

    await expect(apiRequest('/api/secret')).rejects.toThrow('Unauthorized access');

    try {
      await apiRequest('/api/secret');
    } catch (e: any) {
      expect(e).toBeInstanceOf(ApiError);
      expect(e.status).toBe(401);
      expect(e.statusText).toBe('Unauthorized');
      expect(e.data).toEqual({ error: 'Unauthorized access' });
    }
  });

  it('throws ApiError with text content when response is not JSON', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(
      new Response('Bad Gateway', {
        status: 502,
        statusText: 'Bad Gateway',
        headers: { 'Content-Type': 'text/plain' },
      })
    );

    await expect(apiRequest('/api/proxy')).rejects.toThrow('Bad Gateway');
  });
});

describe('Auth API (auth.ts)', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('login sends admin password and returns LoginResponse', async () => {
    const mockRes = { status: 'authenticated', expires_at: '2026-09-13T00:00:00Z' };
    globalThis.fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify(mockRes), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      })
    );

    const result = await login('secretpass');
    expect(result).toEqual(mockRes);
    expect(globalThis.fetch).toHaveBeenCalledWith(
      '/api/auth/login',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ password: 'secretpass' }),
      })
    );
  });

  it('logout sends POST to /api/auth/logout', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ status: 'logged_out' }), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      })
    );

    await logout();
    expect(globalThis.fetch).toHaveBeenCalledWith(
      '/api/auth/logout',
      expect.objectContaining({ method: 'POST' })
    );
  });

  it('checkAuth returns true when /api/auth/me returns authenticated', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ authenticated: true }), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      })
    );

    const isAuthed = await checkAuth();
    expect(isAuthed).toBe(true);
  });

  it('checkAuth returns false when /api/auth/me returns 401 or errors', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ error: 'Unauthorized' }), {
        status: 401,
        headers: { 'Content-Type': 'application/json' },
      })
    );

    const isAuthed = await checkAuth();
    expect(isAuthed).toBe(false);
  });
});

describe('Objects API (objects.ts)', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('listBuckets fetches bucket profile list', async () => {
    const profiles = [{ name: 'primary', bucket: 'my-bucket', default: true }];
    globalThis.fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify(profiles), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      })
    );

    const res = await listBuckets();
    expect(res).toEqual(profiles);
    expect(globalThis.fetch).toHaveBeenCalledWith(
      '/api/buckets',
      expect.anything()
    );
  });

  it('listObjects formats prefix and refresh query parameters correctly', async () => {
    const mockListing = {
      prefix: 'docs/',
      directories: ['docs/arch/'],
      objects: [],
      synced_at: '2026-09-12T00:00:00Z',
    };
    globalThis.fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify(mockListing), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      })
    );

    const res = await listObjects('primary', 'docs/', true);
    expect(res).toEqual(mockListing);
    expect(globalThis.fetch).toHaveBeenCalledWith(
      '/api/buckets/primary/objects?prefix=docs%2F&refresh=true',
      expect.anything()
    );
  });

  it('deleteObject sends DELETE with key query param', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ status: 'deleted', key: 'file.txt' }), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      })
    );

    await deleteObject('primary', 'file.txt');
    expect(globalThis.fetch).toHaveBeenCalledWith(
      '/api/buckets/primary/objects?key=file.txt',
      expect.objectContaining({ method: 'DELETE' })
    );
  });

  it('getDownloadUrl requests download URL and unpacks download_url', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ download_url: 'https://r2.example.com/file.txt' }), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      })
    );

    const url = await getDownloadUrl('primary', 'file.txt');
    expect(url).toBe('https://r2.example.com/file.txt');
  });
});

describe('Transfers API (transfers.ts)', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('initUpload initializes single or multipart upload', async () => {
    const mockSingle = { mode: 'single', upload_url: 'https://presigned.put/url' };
    globalThis.fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify(mockSingle), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      })
    );

    const res = await initUpload('primary', 'test.png', 1024, 'image/png');
    expect(res).toEqual(mockSingle);
    expect(globalThis.fetch).toHaveBeenCalledWith(
      '/api/buckets/primary/upload/init',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({
          key: 'test.png',
          size_bytes: 1024,
          content_type: 'image/png',
        }),
      })
    );
  });

  it('resumeUpload calls /upload/resume with upload_id', async () => {
    const mockResume = {
      upload_id: 'up-123',
      completed_parts: [{ part_number: 1, etag: '"etag-1"' }],
      remaining_parts: [{ part_number: 2, url: 'https://part2.url' }],
    };
    globalThis.fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify(mockResume), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      })
    );

    const res = await resumeUpload('primary', 'up-123');
    expect(res).toEqual(mockResume);
    expect(globalThis.fetch).toHaveBeenCalledWith(
      '/api/buckets/primary/upload/resume',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ upload_id: 'up-123' }),
      })
    );
  });

  it('completeUpload calls /upload/complete with parts list', async () => {
    const mockComplete = { status: 'completed', etag: '"finaletag"' };
    globalThis.fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify(mockComplete), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      })
    );

    const parts = [{ part_number: 1, etag: '"part1"' }, { part_number: 2, etag: '"part2"' }];
    const res = await completeUpload('primary', 'up-123', parts);
    expect(res).toEqual(mockComplete);
    expect(globalThis.fetch).toHaveBeenCalledWith(
      '/api/buckets/primary/upload/complete',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ upload_id: 'up-123', parts }),
      })
    );
  });

  it('abortUpload calls /upload/abort with upload_id', async () => {
    const mockAbort = { status: 'aborted' };
    globalThis.fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify(mockAbort), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      })
    );

    const res = await abortUpload('primary', 'up-123');
    expect(res).toEqual(mockAbort);
    expect(globalThis.fetch).toHaveBeenCalledWith(
      '/api/buckets/primary/upload/abort',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ upload_id: 'up-123' }),
      })
    );
  });

  it('getCorsProbeUrl fetches presigned probe URL', async () => {
    const mockProbe = { probe_url: 'https://r2.example.com/.r2drive-probe?token=xyz' };
    globalThis.fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify(mockProbe), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      })
    );

    const probeUrl = await getCorsProbeUrl('primary');
    expect(probeUrl).toBe('https://r2.example.com/.r2drive-probe?token=xyz');
    expect(globalThis.fetch).toHaveBeenCalledWith(
      '/api/buckets/primary/cors-probe',
      expect.anything()
    );
  });
});
