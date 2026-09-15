# 0012: Proactive CorsProbe and Streaming ProxyTransferFallback

## Context & Problem
Cloudflare R2 defaults to rejecting cross-origin requests unless explicit CORS rules are defined on the bucket. In self-hosted `r2drive` deployments (e.g. running on `localhost:8080` or a custom internal domain), direct browser-to-R2 uploads (`PresignedTransfer`) fail with opaque network errors (`TypeError: Failed to fetch`), confusing first-time users.

## Decision
1. **Proactive `CorsProbe`**: WebConsole performs a lightweight, non-blocking HTTP `OPTIONS` preflight request against a short-lived presigned URL (`GET /api/buckets/{profile}/cors-probe`) when opening or switching buckets. If rejected by R2 CORS rules, WebConsole displays an amber warning banner and provides a guided setup modal with 1-click JSON configuration pre-populated with the user's current origin.
2. **Streaming `ProxyTransferFallback`**: StorageNode provides an authenticated fallback endpoint (`POST /api/buckets/{profile}/upload/proxy?key=...`) that pipes incoming HTTP request bodies straight to S3 `PutObject` as a chunked byte stream with O(1) memory overhead.
3. **Resilient Failover**: If direct browser-to-R2 `PresignedTransfer` encounters a CORS failure, WebConsole transparently falls back to `ProxyTransferFallback`, ensuring uploads never fail silently, while tagging the transfer with an informative badge.
