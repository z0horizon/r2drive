/**
 * Authentication API endpoints for r2drive WebConsole.
 */

import { apiRequest } from './client';

export interface LoginResponse {
  status: string;
  expires_at: string;
}

export interface AuthMeResponse {
  authenticated: boolean;
}

/**
 * Authenticates with the backend admin password.
 * On success, the backend sets an HttpOnly session cookie.
 */
export async function login(password: string): Promise<LoginResponse> {
  return apiRequest<LoginResponse>('/api/auth/login', {
    method: 'POST',
    body: JSON.stringify({ password }),
  });
}

/**
 * Logs out the current session and clears the session cookie.
 */
export async function logout(): Promise<void> {
  await apiRequest<{ status: string }>('/api/auth/logout', {
    method: 'POST',
  });
}

/**
 * Checks whether the current session is authenticated.
 * Returns true if authenticated, false on 401 or network error.
 */
export async function checkAuth(): Promise<boolean> {
  try {
    const res = await apiRequest<AuthMeResponse>('/api/auth/me', {
      method: 'GET',
    });
    return res.authenticated === true;
  } catch {
    return false;
  }
}
