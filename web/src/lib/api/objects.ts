/**
 * Objects and Buckets API client for r2drive WebConsole.
 */

import { apiRequest } from './client';

export interface BucketProfile {
  name: string;
  bucket: string;
  default: boolean;
}

export interface ObjectItem {
  key: string;
  name: string;
  size_bytes: number;
  etag?: string;
  content_type?: string;
  last_modified: string;
}

export interface PrefixListing {
  prefix: string;
  directories: string[];
  objects: ObjectItem[];
  synced_at: string;
}

export interface DeleteObjectResponse {
  status: string;
  key: string;
}

export interface DownloadUrlResponse {
  download_url: string;
}

/**
 * Lists all configured bucket profiles and identifies the default profile.
 */
export async function listBuckets(): Promise<BucketProfile[]> {
  return apiRequest<BucketProfile[]>('/api/buckets');
}

/**
 * Lists objects and virtual directories under a specific prefix in a bucket profile.
 *
 * @param profile The bucket profile name (e.g. 'primary').
 * @param prefix Virtual directory prefix to list under.
 * @param refresh Whether to bypass the local cache and force-sync from Cloudflare R2.
 */
export async function listObjects(
  profile: string,
  prefix: string = '',
  refresh: boolean = false
): Promise<PrefixListing> {
  const params = new URLSearchParams();
  if (prefix) {
    params.set('prefix', prefix);
  }
  if (refresh) {
    params.set('refresh', 'true');
  }

  const query = params.toString() ? `?${params.toString()}` : '';
  return apiRequest<PrefixListing>(`/api/buckets/${encodeURIComponent(profile)}/objects${query}`);
}

/**
 * Deletes an object by key from the specified bucket profile.
 */
export async function deleteObject(profile: string, key: string): Promise<void> {
  const params = new URLSearchParams({ key });
  await apiRequest<DeleteObjectResponse>(
    `/api/buckets/${encodeURIComponent(profile)}/objects?${params.toString()}`,
    {
      method: 'DELETE',
    }
  );
}

/**
 * Obtains a presigned or custom domain download URL for a file in a bucket profile.
 */
export async function getDownloadUrl(profile: string, key: string): Promise<string> {
  const params = new URLSearchParams({ key });
  const res = await apiRequest<DownloadUrlResponse>(
    `/api/buckets/${encodeURIComponent(profile)}/download?${params.toString()}`
  );
  return res.download_url;
}
