/**
 * Svelte 5 Runes store managing authentication state and session lifecycle.
 */

import { checkAuth, login as apiLogin, logout as apiLogout } from '../api/auth';

export class AuthStore {
  isAuthenticated = $state(false);
  loading = $state(false);
  error = $state<string | null>(null);

  /**
   * Verifies current session authentication status with /api/auth/me.
   */
  async check(): Promise<boolean> {
    this.loading = true;
    this.error = null;
    try {
      this.isAuthenticated = await checkAuth();
      return this.isAuthenticated;
    } catch (err) {
      this.isAuthenticated = false;
      this.error = err instanceof Error ? err.message : String(err);
      return false;
    } finally {
      this.loading = false;
    }
  }

  /**
   * Submits admin password to /api/auth/login and updates authentication state.
   */
  async login(password: string): Promise<boolean> {
    this.loading = true;
    this.error = null;
    try {
      await apiLogin(password);
      this.isAuthenticated = true;
      return true;
    } catch (err) {
      this.isAuthenticated = false;
      this.error = err instanceof Error ? err.message : String(err);
      throw err;
    } finally {
      this.loading = false;
    }
  }

  /**
   * Ends current session and resets authentication state.
   */
  async logout(): Promise<void> {
    this.loading = true;
    this.error = null;
    try {
      await apiLogout();
    } finally {
      this.isAuthenticated = false;
      this.loading = false;
    }
  }
}

export const authStore = new AuthStore();
