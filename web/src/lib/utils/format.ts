/**
 * Utilities for formatting file sizes and timestamps in the WebConsole.
 */

const BYTE_UNITS = ['B', 'KB', 'MB', 'GB', 'TB', 'PB', 'EB'] as const;

/**
 * Formats a number of bytes into a human-readable string (e.g. '0 B', '1.5 KB', '10 MB').
 * Handles zero, negative numbers, non-finite values, and custom decimal precision.
 *
 * @param bytes The number of bytes to format.
 * @param decimals Optional decimal places (default: 1 for fractional values, 0 for integers).
 * @returns A formatted string such as '0 B', '1 KB', '1.5 KB', '10 MB'.
 */
export function formatBytes(bytes: number, decimals?: number): string {
  if (!Number.isFinite(bytes) || bytes === 0) {
    return '0 B';
  }

  const isNegative = bytes < 0;
  const absBytes = Math.abs(bytes);

  if (absBytes < 1024) {
    return `${isNegative ? '-' : ''}${Math.round(absBytes)} B`;
  }

  const k = 1024;
  const i = Math.min(
    Math.floor(Math.log(absBytes) / Math.log(k)),
    BYTE_UNITS.length - 1
  );

  const value = absBytes / Math.pow(k, i);
  const dm = decimals !== undefined ? Math.max(0, decimals) : 1;

  // Format with specified decimals and strip redundant trailing zeroes (e.g. 1.0 -> 1)
  const formatted = parseFloat(value.toFixed(dm)).toString();

  return `${isNegative ? '-' : ''}${formatted} ${BYTE_UNITS[i]}`;
}

/**
 * Formats a date/timestamp into a human-readable format ('YYYY-MM-DD HH:mm:ss').
 * Accepts ISO string, Date object, Unix timestamp in milliseconds, or null/undefined.
 *
 * @param date The date string or Date object.
 * @returns Formatted date string or '-' if input is invalid/empty.
 */
export function formatDate(date: string | Date | number | null | undefined): string {
  if (!date) {
    return '-';
  }

  const d = date instanceof Date ? date : new Date(date);
  if (isNaN(d.getTime())) {
    return '-';
  }

  const pad = (n: number) => n.toString().padStart(2, '0');
  const year = d.getFullYear();
  const month = pad(d.getMonth() + 1);
  const day = pad(d.getDate());
  const hours = pad(d.getHours());
  const minutes = pad(d.getMinutes());
  const seconds = pad(d.getSeconds());

  return `${year}-${month}-${day} ${hours}:${minutes}:${seconds}`;
}
