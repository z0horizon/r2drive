/**
 * Generic HTTP client wrapper for r2drive WebConsole API.
 */

export class ApiError extends Error {
  readonly status: number;
  readonly statusText: string;
  readonly data?: unknown;

  constructor(message: string, status: number, statusText: string, data?: unknown) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
    this.statusText = statusText;
    this.data = data;
  }
}

/**
 * Sends an HTTP request to the r2drive backend API.
 * Automatically injects `credentials: 'same-origin'` and standard JSON headers.
 * Throws a typed `ApiError` with HTTP status and backend message on failures.
 *
 * @param path The API endpoint path (e.g. '/api/buckets').
 * @param options Standard fetch RequestInit options.
 * @returns Resolves with parsed JSON response data.
 */
export async function apiRequest<T>(path: string, options: RequestInit = {}): Promise<T> {
  const headers = new Headers(options.headers);

  if (!headers.has('Accept')) {
    headers.set('Accept', 'application/json');
  }

  // If a string body looks like JSON, ensure Content-Type is set
  if (options.body && typeof options.body === 'string' && !headers.has('Content-Type')) {
    headers.set('Content-Type', 'application/json');
  }

  const response = await fetch(path, {
    ...options,
    headers,
    credentials: options.credentials ?? 'same-origin',
  });

  if (!response.ok) {
    let errorMessage = `API request failed: ${response.status} ${response.statusText}`;
    let errorData: unknown = undefined;

    const contentType = response.headers.get('content-type');
    if (contentType && contentType.includes('application/json')) {
      try {
        errorData = await response.json();
        if (
          errorData &&
          typeof errorData === 'object' &&
          'error' in errorData &&
          typeof (errorData as { error: unknown }).error === 'string'
        ) {
          errorMessage = (errorData as { error: string }).error;
        } else if (
          errorData &&
          typeof errorData === 'object' &&
          'message' in errorData &&
          typeof (errorData as { message: unknown }).message === 'string'
        ) {
          errorMessage = (errorData as { message: string }).message;
        }
      } catch {
        // Fallback to default message
      }
    } else {
      try {
        const text = await response.text();
        if (text.trim()) {
          errorMessage = text.trim();
        }
      } catch {
        // Fallback to default message
      }
    }

    throw new ApiError(errorMessage, response.status, response.statusText, errorData);
  }

  if (response.status === 204 || response.headers.get('content-length') === '0') {
    return undefined as unknown as T;
  }

  const contentType = response.headers.get('content-type');
  if (contentType && contentType.includes('application/json')) {
    return (await response.json()) as T;
  }

  return (await response.text()) as unknown as T;
}
