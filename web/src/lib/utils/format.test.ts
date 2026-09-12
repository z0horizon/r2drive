import { describe, it, expect } from 'vitest';
import { formatBytes, formatDate } from './format';

describe('formatBytes', () => {
  it('formats zero bytes correctly', () => {
    expect(formatBytes(0)).toBe('0 B');
    expect(formatBytes(-0)).toBe('0 B');
  });

  it('handles invalid or non-finite inputs gracefully', () => {
    expect(formatBytes(NaN)).toBe('0 B');
    expect(formatBytes(Infinity)).toBe('0 B');
    expect(formatBytes(-Infinity)).toBe('0 B');
  });

  it('formats small byte values under 1 KB', () => {
    expect(formatBytes(1)).toBe('1 B');
    expect(formatBytes(500)).toBe('500 B');
    expect(formatBytes(1023)).toBe('1023 B');
  });

  it('formats exact power-of-two boundaries without redundant decimals', () => {
    expect(formatBytes(1024)).toBe('1 KB');
    expect(formatBytes(1048576)).toBe('1 MB');
    expect(formatBytes(1073741824)).toBe('1 GB');
    expect(formatBytes(1099511627776)).toBe('1 TB');
  });

  it('formats fractional values with default 1 decimal place', () => {
    expect(formatBytes(1536)).toBe('1.5 KB');
    expect(formatBytes(1048576 * 2.5)).toBe('2.5 MB');
  });

  it('formats negative byte counts correctly', () => {
    expect(formatBytes(-500)).toBe('-500 B');
    expect(formatBytes(-1024)).toBe('-1 KB');
    expect(formatBytes(-1536)).toBe('-1.5 KB');
    expect(formatBytes(-1048576)).toBe('-1 MB');
  });

  it('respects custom decimal parameter', () => {
    // 1500 / 1024 = 1.46484375
    expect(formatBytes(1500, 2)).toBe('1.46 KB');
    expect(formatBytes(1500, 3)).toBe('1.465 KB');
    expect(formatBytes(1500, 0)).toBe('1 KB');
  });
});

describe('formatDate', () => {
  it('returns "-" for null, undefined, or empty string', () => {
    expect(formatDate(null)).toBe('-');
    expect(formatDate(undefined)).toBe('-');
    expect(formatDate('')).toBe('-');
  });

  it('returns "-" for invalid date inputs', () => {
    expect(formatDate('invalid-date')).toBe('-');
    expect(formatDate('2026-99-99T99:99:99Z')).toBe('-');
  });

  it('formats a Date object with zero-padded components', () => {
    // Month is 0-indexed: 8 = September, 4 = 04th day, 9 = 09 hours
    const d = new Date(2026, 8, 4, 9, 5, 7);
    expect(formatDate(d)).toBe('2026-09-04 09:05:07');
  });

  it('formats an ISO date string or timestamp correctly', () => {
    const timestamp = new Date(2026, 11, 25, 18, 30, 0).getTime();
    expect(formatDate(timestamp)).toBe('2026-12-25 18:30:00');

    const d = new Date(2026, 0, 1, 0, 0, 0);
    expect(formatDate(d.toISOString())).toBe(formatDate(d));
  });
});
