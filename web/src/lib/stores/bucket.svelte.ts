/**
 * Svelte 5 Runes store managing bucket profiles, directory navigation, and object listings.
 */

import {
  listBuckets,
  listObjects,
  deleteObject,
  type BucketProfile,
  type ObjectItem,
  type PrefixListing,
} from '../api/objects';

export interface Breadcrumb {
  label: string;
  prefix: string;
}

export class BucketStore {
  profiles = $state<BucketProfile[]>([]);
  selectedProfile = $state<string>('');
  currentPrefix = $state<string>('');
  directories = $state<string[]>([]);
  objects = $state<ObjectItem[]>([]);
  loading = $state<boolean>(false);
  error = $state<string | null>(null);

  /**
   * Reactive breadcrumb hierarchy derived from current prefix.
   * Clicking any segment navigates directly to that prefix.
   */
  get breadcrumbs(): Breadcrumb[] {
    const crumbs: Breadcrumb[] = [{ label: 'Root', prefix: '' }];
    if (!this.currentPrefix) {
      return crumbs;
    }

    const segments = this.currentPrefix.split('/').filter(Boolean);
    let accumulated = '';
    for (const segment of segments) {
      accumulated += `${segment}/`;
      crumbs.push({
        label: segment,
        prefix: accumulated,
      });
    }
    return crumbs;
  }

  /**
   * Loads all available bucket profiles from the backend and selects default.
   */
  async loadProfiles(): Promise<void> {
    this.loading = true;
    this.error = null;
    try {
      const profiles = await listBuckets();
      this.profiles = profiles;

      if (profiles.length > 0 && !this.selectedProfile) {
        const defaultProfile = profiles.find((p) => p.default) ?? profiles[0];
        this.selectedProfile = defaultProfile.name;
      }

      if (this.selectedProfile) {
        await this.fetchObjects(false);
      }
    } catch (err) {
      this.error = err instanceof Error ? err.message : String(err);
    } finally {
      this.loading = false;
    }
  }

  /**
   * Selects a bucket profile, resets prefix to root, and fetches contents.
   */
  async setProfile(profile: string): Promise<void> {
    this.selectedProfile = profile;
    this.currentPrefix = '';
    await this.fetchObjects(false);
  }

  /**
   * Navigates into a virtual directory prefix and fetches its contents.
   */
  async setPrefix(prefix: string): Promise<void> {
    let normalized = prefix.trim();
    if (normalized.startsWith('/')) {
      normalized = normalized.slice(1);
    }
    if (normalized && !normalized.endsWith('/')) {
      normalized += '/';
    }
    this.currentPrefix = normalized;
    await this.fetchObjects(false);
  }

  /**
   * Forces a refresh from Cloudflare R2 bypassing local SQLite cache.
   */
  async refresh(): Promise<void> {
    await this.fetchObjects(true);
  }

  /**
   * Fetches objects and directories for current profile and prefix.
   */
  async fetchObjects(forceRefresh: boolean = false): Promise<PrefixListing | null> {
    if (!this.selectedProfile) {
      return null;
    }

    this.loading = true;
    this.error = null;
    try {
      const listing = await listObjects(
        this.selectedProfile,
        this.currentPrefix,
        forceRefresh
      );
      this.directories = listing.directories;
      this.objects = listing.objects;
      return listing;
    } catch (err) {
      this.error = err instanceof Error ? err.message : String(err);
      return null;
    } finally {
      this.loading = false;
    }
  }

  /**
   * Deletes an object by key and immediately removes it from local listing.
   */
  async deleteItem(key: string): Promise<void> {
    if (!this.selectedProfile) {
      return;
    }

    this.loading = true;
    this.error = null;
    try {
      await deleteObject(this.selectedProfile, key);
      this.objects = this.objects.filter((obj) => obj.key !== key);
    } catch (err) {
      this.error = err instanceof Error ? err.message : String(err);
      throw err;
    } finally {
      this.loading = false;
    }
  }

  /**
   * Resets all bucket store state on session end or logout.
   */
  reset(): void {
    this.profiles = [];
    this.selectedProfile = '';
    this.currentPrefix = '';
    this.directories = [];
    this.objects = [];
    this.loading = false;
    this.error = null;
  }
}

export const bucketStore = new BucketStore();
