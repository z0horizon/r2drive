# CORS Diagnostic & Resilient Upload Fallback

## Problem Statement

When users connect a fresh or existing Cloudflare R2 bucket to `r2drive` and access the WebConsole from `http://localhost:8080` (or any custom domain), browser direct-to-R2 uploads (`PresignedTransfer`) fail if the bucket does not have a CORS policy configured.

Cloudflare R2 defaults to rejecting all cross-origin requests:
```http
OPTIONS /<bucket>/... HTTP/1.1
Origin: http://localhost:8080
Access-Control-Request-Method: PUT

HTTP/1.1 403 Forbidden
<Error><Code>Unauthorized</Code><Message>CORS not configured for this bucket</Message></Error>
```

Browsers block the upload request with `TypeError: Failed to fetch`. Users unfamiliar with Cloudflare R2 bucket CORS settings are left confused by the upload failure.

## Agreed Requirements for Follow-Up Task

1. **Proactive Health Check Banner (Preflight Probe)**:
   - When switching to or opening a bucket in WebConsole, run a lightweight, non-blocking `OPTIONS` preflight probe to the bucket.
   - If Cloudflare returns 403 (CORS not configured), display an amber warning banner under the Header:
     *"⚠️ Bucket chưa được cấu hình CORS. Trình duyệt không thể upload trực tiếp lên Cloudflare R2. [Xem hướng dẫn & Copy JSON cấu hình]"*

2. **Smart CORS Diagnostic & Setup Modal**:
   - Triggered either by clicking the warning banner or when an upload encounters a CORS/fetch error.
   - Dynamic JSON snippet pre-filled with the current origin (`window.location.origin`):
     ```json
     [
       {
         "AllowedOrigins": ["http://localhost:8080", "*"],
         "AllowedMethods": ["GET", "PUT", "POST", "DELETE", "HEAD"],
         "AllowedHeaders": ["*"],
         "ExposeHeaders": ["ETag"],
         "MaxAgeSeconds": 3600
       }
     ]
     ```
   - 1-Click **"Copy CORS JSON"** button with visual feedback (copied tooltip/toast).
   - Direct link to Cloudflare Dashboard (`https://dash.cloudflare.com/?to=/:account/r2/overview`).
   - 3-step concise guide (Open Settings > CORS Policy > Paste & Save).
   - "Retry / Re-check" button to dismiss banner once saved.

3. **Optional Server-Proxy Upload Fallback (Zero-Config Upload)**:
   - Backend endpoint: `POST /api/buckets/{profile}/upload/proxy?key=...`
   - Streams the file body directly to Cloudflare R2 using StorageNode backend credentials (server-to-server, unaffected by browser CORS).
   - If direct browser-to-R2 upload fails due to CORS, automatically offer or execute fallback upload via server proxy so user's upload never gets blocked.
   - Displays a notice: *"Uploaded via server proxy (CORS not enabled on R2 bucket. Configure CORS for direct maximum-speed transfers)."*

## References

- Cloudflare R2 CORS Documentation: `https://developers.cloudflare.com/r2/buckets/cors/`
- Architecture: `docs/adr/0004-direct-to-r2-presigned-transfer.md`
- Tracking: Scheduled as follow-up task after Phase 1 MVP.
