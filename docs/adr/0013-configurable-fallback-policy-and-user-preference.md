# 0013: Configurable FallbackPolicy and User FallbackPreference

## Context & Problem
While `ProxyTransferFallback` (ADR-0012) guarantees reliable uploads when Cloudflare R2 CORS is unconfigured, unconstrained streaming through `StorageNode` risks saturating host VPS bandwidth, CPU, and egress quotas on large files. Furthermore, server administrators require control over proxy upload availability and payload limits, while `WebConsole` users need autonomy over whether transfers should automatically divert through `StorageNode` or halt with diagnostic guidance.

## Decision
1. **Administrative `FallbackPolicy` (`config.yaml`)**:
   `StorageNode` introduces an optional `transfers:` configuration block:
   - `proxy_fallback: bool` (default: `true`).
   - `max_proxy_file_size: Option<String | u64>` (e.g. `"500MB"`, `"1GB"`, or raw bytes; default: 5GB, conforming to S3 single PUT bounds).
   If disabled, `StorageNode` rejects proxy upload requests at the door with `403 Forbidden`.
2. **Proactive Discovery via `CorsProbe`**:
   The `GET /api/buckets/{profile}/cors-probe` endpoint returns the server's active `FallbackPolicy` alongside the probe URL (`{ probe_url, fallback_policy: { enabled, max_payload_bytes } }`). This avoids extra round-trips and informs `WebConsole` of server limits before transfers begin.
3. **Dual `PayloadLimitGate` Enforcement**:
   - *Client-side*: `WebConsole` validates file sizes prior to dispatching proxy uploads, instantly notifying users if a file exceeds the server's limit without transmitting useless bytes.
   - *Server-side*: `StorageNode` validates the incoming `Content-Length` header in `upload_proxy`, immediately returning `413 Payload Too Large` if the threshold is exceeded.
4. **User `FallbackPreference` (`WebConsole`)**:
   `WebConsole` introduces a user preference persisted in browser `localStorage` (`r2drive_proxy_fallback_enabled`, default: `true`).
   - If disabled by user: transfers failing CORS will not proxy; they fail with actionable CORS setup notifications.
   - If disabled by server: the user toggle is rendered disabled with explanatory guidance ("Disabled by server administrator").
