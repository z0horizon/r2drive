/**
 * Upload and transfer API client for r2drive WebConsole.
 */

import { apiRequest } from './client';

export interface PresignedPart {
  part_number: number;
  url: string;
}

export interface SingleUploadInitResponse {
  mode: 'single';
  upload_url: string;
}

export interface MultipartUploadInitResponse {
  mode: 'multipart';
  upload_id: string;
  part_size: number;
  parts: PresignedPart[];
}

export type InitUploadResponse = SingleUploadInitResponse | MultipartUploadInitResponse;

export interface CompletedPart {
  part_number: number;
  etag: string;
}

export interface ResumeUploadResponse {
  upload_id: string;
  completed_parts: CompletedPart[];
  remaining_parts: PresignedPart[];
}

export interface CompleteUploadResponse {
  status: string;
  etag: string;
}

export interface AbortUploadResponse {
  status: string;
}

/**
 * Initializes a presigned single or multipart upload session.
 * Files under 10MB receive a direct single PUT URL; files 10MB and above
 * initiate a multipart upload with presigned URLs for each chunk.
 *
 * @param profile The bucket profile name.
 * @param key The destination object key in the bucket.
 * @param size_bytes The total file size in bytes.
 * @param content_type Optional MIME type for the object.
 */
export async function initUpload(
  profile: string,
  key: string,
  size_bytes: number,
  content_type?: string
): Promise<InitUploadResponse> {
  return apiRequest<InitUploadResponse>(
    `/api/buckets/${encodeURIComponent(profile)}/upload/init`,
    {
      method: 'POST',
      body: JSON.stringify({
        key,
        size_bytes,
        ...(content_type ? { content_type } : {}),
      }),
    }
  );
}

/**
 * Resumes an interrupted multipart upload session, querying completed parts
 * and returning presigned URLs for remaining parts.
 *
 * @param profile The bucket profile name.
 * @param upload_id The multipart session upload ID.
 */
export async function resumeUpload(
  profile: string,
  upload_id: string
): Promise<ResumeUploadResponse> {
  return apiRequest<ResumeUploadResponse>(
    `/api/buckets/${encodeURIComponent(profile)}/upload/resume`,
    {
      method: 'POST',
      body: JSON.stringify({ upload_id }),
    }
  );
}

/**
 * Completes a multipart upload by assembling all uploaded part receipts with ETags.
 *
 * @param profile The bucket profile name.
 * @param upload_id The multipart session upload ID.
 * @param parts Array of completed parts with part_number and etag.
 * @param key Optional object key if overriding.
 */
export async function completeUpload(
  profile: string,
  upload_id: string,
  parts: CompletedPart[],
  key?: string
): Promise<CompleteUploadResponse> {
  return apiRequest<CompleteUploadResponse>(
    `/api/buckets/${encodeURIComponent(profile)}/upload/complete`,
    {
      method: 'POST',
      body: JSON.stringify({
        upload_id,
        parts,
        ...(key ? { key } : {}),
      }),
    }
  );
}

/**
 * Aborts an active multipart upload session and cleans up server state.
 *
 * @param profile The bucket profile name.
 * @param upload_id The multipart session upload ID.
 */
export async function abortUpload(
  profile: string,
  upload_id: string
): Promise<AbortUploadResponse> {
  return apiRequest<AbortUploadResponse>(
    `/api/buckets/${encodeURIComponent(profile)}/upload/abort`,
    {
      method: 'POST',
      body: JSON.stringify({ upload_id }),
    }
  );
}

export interface ServerFallbackPolicy {
  enabled: boolean;
  max_payload_bytes: number;
}

export interface CorsProbeResponse {
  probe_url: string;
  fallback_policy: ServerFallbackPolicy;
}

/**
 * Fetches a presigned probe URL and server FallbackPolicy used to perform an
 * OPTIONS preflight check for CORS and detect proxy upload availability.
 *
 * @param profile The bucket profile name.
 */
export async function getCorsProbe(profile: string): Promise<CorsProbeResponse> {
  return apiRequest<CorsProbeResponse>(
    `/api/buckets/${encodeURIComponent(profile)}/cors-probe`
  );
}

/**
 * Fetches a presigned probe URL used to perform an OPTIONS preflight check for CORS.
 *
 * @param profile The bucket profile name.
 */
export async function getCorsProbeUrl(profile: string): Promise<string> {
  return (await getCorsProbe(profile)).probe_url;
}

export interface ProxyUploadResponse {
  status: string;
  key: string;
  size_bytes: number;
  etag?: string;
}

/**
 * Uploads a file or blob through the server streaming proxy fallback endpoint
 * using XMLHttpRequest for granular upload progress tracking and abort support.
 *
 * @param profile The bucket profile name.
 * @param key The destination object key in the bucket.
 * @param file File or Blob to upload.
 * @param onProgress Optional callback reporting (loadedBytes, totalBytes).
 * @param signal Optional AbortSignal for user cancellation.
 */
export function uploadViaProxy(
  profile: string,
  key: string,
  file: File | Blob,
  onProgress?: (loaded: number, total: number) => void,
  signal?: AbortSignal
): Promise<ProxyUploadResponse> {
  return new Promise((resolve, reject) => {
    if (signal?.aborted) {
      reject(new DOMException('Upload aborted by user', 'AbortError'));
      return;
    }

    const xhr = new XMLHttpRequest();
    const url = `/api/buckets/${encodeURIComponent(profile)}/upload/proxy?key=${encodeURIComponent(key)}`;
    xhr.open('POST', url);
    xhr.withCredentials = true;

    if (file.type) {
      xhr.setRequestHeader('Content-Type', file.type);
    }

    if (xhr.upload && onProgress) {
      xhr.upload.onprogress = (e: ProgressEvent) => {
        if (e.lengthComputable) {
          onProgress(e.loaded, e.total);
        }
      };
    }

    let abortHandler: (() => void) | null = null;
    const cleanup = () => {
      if (signal && abortHandler) {
        signal.removeEventListener('abort', abortHandler);
        abortHandler = null;
      }
    };

    xhr.onload = () => {
      cleanup();
      let data: any;
      try {
        data = JSON.parse(xhr.responseText);
      } catch {
        data = xhr.responseText;
      }

      if (xhr.status >= 200 && xhr.status < 300) {
        resolve(data);
      } else {
        const errorMsg = data?.error ?? `Proxy upload failed with status ${xhr.status}`;
        reject(new Error(errorMsg));
      }
    };

    xhr.onerror = () => {
      cleanup();
      reject(new Error('Network error during proxy upload'));
    };

    xhr.onabort = () => {
      cleanup();
      reject(new DOMException('Upload aborted by user', 'AbortError'));
    };

    if (signal) {
      abortHandler = () => {
        xhr.abort();
        cleanup();
        reject(new DOMException('Upload aborted by user', 'AbortError'));
      };
      signal.addEventListener('abort', abortHandler, { once: true });
    }

    xhr.send(file);
  });
}

