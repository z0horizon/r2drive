/**
 * Svelte 5 Runes store managing upload transfer queue, progress, and status.
 */

export type UploadStatus = 'queued' | 'uploading' | 'completed' | 'failed' | 'aborted';

export interface UploadItem {
  id: string;
  file: File | Blob;
  name: string;
  key: string;
  profile: string;
  progress: number; // 0 to 100
  speed: number;    // bytes per second
  status: UploadStatus;
  error?: string;
  uploadId?: string; // Multipart upload session ID when applicable
  totalBytes: number;
  uploadedBytes: number;
}

export interface AddUploadOptions {
  file: File | Blob;
  key: string;
  profile: string;
  id?: string;
  name?: string;
}

export class UploadStore {
  items = $state<UploadItem[]>([]);

  /**
   * Returns list of currently active or queued uploads.
   */
  get activeUploads(): UploadItem[] {
    return this.items.filter((i) => i.status === 'uploading' || i.status === 'queued');
  }

  /**
   * Returns list of successfully completed uploads.
   */
  get completedUploads(): UploadItem[] {
    return this.items.filter((i) => i.status === 'completed');
  }

  /**
   * Returns list of failed uploads.
   */
  get failedUploads(): UploadItem[] {
    return this.items.filter((i) => i.status === 'failed');
  }

  /**
   * Returns true if any upload is currently in progress or queued.
   */
  get isUploading(): boolean {
    return this.activeUploads.length > 0;
  }

  /**
   * Computes aggregate upload progress (0 to 100) across all items.
   */
  get overallProgress(): number {
    if (this.items.length === 0) return 0;
    const total = this.items.reduce((acc, i) => acc + i.totalBytes, 0);
    if (total === 0) return 100;
    const uploaded = this.items.reduce((acc, i) => acc + i.uploadedBytes, 0);
    return Math.min(100, Math.round((uploaded / total) * 100));
  }

  /**
   * Adds a new file to the upload transfer queue.
   */
  add(options: AddUploadOptions): UploadItem {
    const id =
      options.id ??
      (typeof crypto !== 'undefined' && crypto.randomUUID
        ? crypto.randomUUID()
        : `upload-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`);

    const name =
      options.name ??
      (options.file instanceof File
        ? options.file.name
        : options.key.split('/').pop() || options.key);

    const totalBytes = options.file.size;

    const item: UploadItem = {
      id,
      file: options.file,
      name,
      key: options.key,
      profile: options.profile,
      progress: 0,
      speed: 0,
      status: 'queued',
      totalBytes,
      uploadedBytes: 0,
    };

    this.items.push(item);
    return item;
  }

  /**
   * Updates upload progress, transfer speed, and uploaded bytes for a transfer.
   */
  updateProgress(
    id: string,
    progress: number,
    speed: number = 0,
    uploadedBytes?: number
  ): void {
    const item = this.items.find((i) => i.id === id);
    if (!item) return;

    if (item.status === 'failed' || item.status === 'aborted' || item.status === 'completed') {
      return;
    }

    item.progress = Math.min(100, Math.max(0, progress));
    item.speed = speed;
    if (uploadedBytes !== undefined) {
      item.uploadedBytes = uploadedBytes;
    } else {
      item.uploadedBytes = Math.round((item.progress / 100) * item.totalBytes);
    }

    if (item.status === 'queued') {
      item.status = 'uploading';
    }
  }

  /**
   * Marks a transfer as completed with 100% progress.
   */
  markComplete(id: string): void {
    const item = this.items.find((i) => i.id === id);
    if (!item) return;

    item.status = 'completed';
    item.progress = 100;
    item.uploadedBytes = item.totalBytes;
    item.speed = 0;
    item.error = undefined;
  }

  /**
   * Marks a transfer as failed with an error message.
   */
  markFailed(id: string, error: string): void {
    const item = this.items.find((i) => i.id === id);
    if (!item) return;

    item.status = 'failed';
    item.error = error;
    item.speed = 0;
  }

  /**
   * Marks a transfer as aborted.
   */
  markAborted(id: string): void {
    const item = this.items.find((i) => i.id === id);
    if (!item) return;

    item.status = 'aborted';
    item.speed = 0;
  }

  /**
   * Associates a multipart upload session ID with a transfer.
   */
  setUploadId(id: string, uploadId: string): void {
    const item = this.items.find((i) => i.id === id);
    if (item) {
      item.uploadId = uploadId;
    }
  }

  /**
   * Removes a transfer item by ID.
   */
  removeItem(id: string): void {
    this.items = this.items.filter((i) => i.id !== id);
  }

  /**
   * Removes all completed transfers from the list.
   */
  clearCompleted(): void {
    this.items = this.items.filter((i) => i.status !== 'completed');
  }

  /**
   * Clears all transfers from the list.
   */
  clearAll(): void {
    this.items = [];
  }
}

export const uploadStore = new UploadStore();
