# Roadmap: r2drive

A self-hosted Cloudflare R2 drive service packaged as a Docker container, providing an intuitive WebConsole and powerful CLI built on top of `r2kit`.

---

## Vision & Architecture

- **Core Engine**: Rust backend service (`StorageNode`) leveraging `r2kit` for high-throughput, safe R2 operations.
- **Web Interface**: Modern Single-Page Application (`WebConsole`) built with Svelte 5, Vite, and Tailwind CSS, embedded directly into the Rust binary.
- **Delivery**: Single Docker image (`r2drive:latest`) with optional `--headless` mode for API-only workloads.
- **Persistence**: Hybrid YAML configuration (`config.yaml`) + embedded SQLite `MetadataStore` (architected for future PostgreSQL via SQLx).
- **Data Plane**: `PresignedTransfer` enabling browser-direct multipart uploads to Cloudflare R2, bypassing server bandwidth bottlenecks.
- **Reliability & Security**: Application-level `TrashBin` and `FileVersion` history; capability-token public sharing gateway with instant revocation.

---

## Milestones

### Phase 1: MVP — Core Drive & Container

**Goal**: Deliver a fully functional, self-hosted Docker service and CLI for browsing, uploading, and managing Cloudflare R2 buckets.

- [x] **1.1 Core CLI Engine**
  - [x] Initialize Cargo workspace and crate setup linking `r2kit`
  - [x] CLI command suite: `r2drive ls`, `r2drive upload`, `r2drive download`, `r2drive rm`, `r2drive cat`
  - [x] Resumable CLI upload engine: auto-persists `MultipartSessionSnapshot` to `~/.cache/r2drive/uploads/` on interruption and resumes via `r2kit`
  - [x] YAML configuration loader (`config.yaml`) supporting multiple `BucketProfile` entries
- [x] **1.2 StorageNode & Persistence Layer**
  - [x] Embedded SQLite `MetadataStore` with migrations (buckets, cached objects, folders, active multipart sessions)
  - [x] On-demand `CacheFreshness` sync engine with configurable TTL and manual force-refresh
  - [x] REST API: Authentication (`AdminSession`), Bucket management, Object browsing, Presigned URLs
  - [x] Resumable upload coordinator: `upload/init`, `upload/resume`, `upload/complete`, and `upload/abort` endpoints
  - [x] Automated 24-hour `StaleUploadCleanup` background worker aborting abandoned multipart sessions
- [x] **1.3 WebConsole (SPA with Svelte 5)**
  - [x] Responsive file explorer layout (folder tree, grid/list view, breadcrumbs)
  - [x] Admin login screen
  - [x] Multi-bucket switcher selector
  - [x] Drag-and-drop direct-to-R2 presigned uploader with chunked progress tracking
  - [x] Resumable upload UX: transient auto-retry (exponential backoff) and reload resume prompt via `IndexedDB`
  - [x] File download and deletion actions
- [x] **1.4 Packaging & Release**
  - [x] Embed WebConsole assets into Rust binary via `rust-embed`
  - [x] Multi-stage `Dockerfile` producing slim all-in-one container (~45MB)
  - [x] Headless mode toggle (`--headless` flag / `R2DRIVE_HEADLESS=true`)
- [ ] **1.5 CORS Diagnostic & Resilient Upload Fallback** (Follow-up)
  - [ ] Proactive preflight health check probe on bucket selection with warning banner
  - [ ] Smart CORS configuration guide modal with 1-click JSON copy and Cloudflare deep link
  - [ ] Automatic server-proxy streaming upload fallback (`POST /api/buckets/{profile}/upload/proxy`) when browser direct PUT is blocked by CORS

---

### Phase 2: Enhanced Drive Experience, Sharing & Versioning

**Goal**: Elevate `r2drive` from a simple object browser into a complete cloud drive with versioning, trash recovery, secure sharing, and media streaming.

- [ ] **2.1 TrashBin & Soft-Delete Engine**
  - [ ] Application-level soft delete marking `deleted_at` timestamp in `MetadataStore`
  - [ ] Dedicated "Trash" view in WebConsole and `r2drive trash ls/restore/empty` CLI commands
  - [ ] Instant restore without data transfer overhead
  - [ ] Configurable retention policy (default: 30 days) with automated background purge task
- [ ] **2.2 FileVersion History**
  - [ ] Server-side `CopyObject` to `.versions/<key>.<hash>` prior to in-place overwrite
  - [ ] Version timeline inspector in WebConsole with timestamp, author, and size
  - [ ] One-click rollback (`Restore version`) promoting historical snapshot to live object
- [ ] **2.3 Secure Public Sharing Gateway**
  - [ ] Capability URL generation (`/s/<share_token>`) managed by `StorageNode`
  - [ ] Optional password protection with Argon2id hash verification
  - [ ] Expiration date enforcement and max download count quotas
  - [ ] Instant link revocation toggle in WebConsole
  - [ ] Ephemeral 5-minute presigned redirect (HTTP 302) to R2 preserving zero-egress server costs
- [ ] **2.4 Media Streaming & Background Thumbnailing**
  - [ ] Direct video and audio streaming via HTTP Range requests (206 Partial Content) straight from R2
  - [ ] Background thumbnail generation daemon for images/videos cached locally in `.thumbnails/`
  - [ ] Built-in viewer modal for images, markdown, syntax-highlighted code, and PDF
- [ ] **2.5 Batch Operations & Search**
  - [ ] Multi-selection bar: batch delete, batch move, and streaming ZIP archive download
  - [ ] Server-side prefix rename and directory restructuring via parallel `CopyObject` + `DeleteObject`
  - [ ] Sub-second full-text prefix and keyword search querying `MetadataStore`
- [ ] **2.6 Dual-Database Support: PostgreSQL Integration**
  - [ ] Dynamic connection scheme detection in `config.yaml` (`sqlite://...` vs `postgres://...` / `postgresql://...`)
  - [ ] Native PostgreSQL repository implementation (`PostgresMetadataRepo`) via `sqlx`
  - [ ] Separate PostgreSQL migration suite (`migrations/postgres/`) with native `TIMESTAMPTZ`, `BIGINT GENERATED ALWAYS AS IDENTITY`, and `BOOLEAN`
  - [ ] Clustered and multi-container deployment capability using external Postgres

---

### Phase 3: Multi-Tenancy, Team Collaboration & Scale

**Goal**: Transform into a multi-tenant collaboration platform with enterprise identity and clustered database support.

- [ ] **3.1 Multi-Tenant Isolation & Quotas**
  - [ ] Tenant data segregation via isolated `UserPrefix` (`users/<user_id>/...`)
  - [ ] Shared team spaces (`teams/<team_id>/...`) with member permission levels (Admin, Editor, Viewer)
  - [ ] Storage quota enforcement (hard/soft limits) per user and team with usage visualizer
- [ ] **3.2 Authentication & Identity Federation**
  - [ ] User management with local credentials and secure password reset workflows
  - [ ] OAuth2 / OpenID Connect (OIDC) integration (Google, GitHub, Authentik, Keycloak)
  - [ ] Role-Based Access Control (RBAC) governing bucket access and administrative privileges
- [ ] **3.3 Enterprise Clustering & High Availability**
  - [ ] Connection pooling optimization and read replicas for PostgreSQL
  - [ ] Stateless `StorageNode` clustering behind reverse proxies (Traefik, Nginx, Caddy)
- [ ] **3.4 Audit Trail & Compliance**
  - [ ] Structured event logging for all file operations (upload, download, share, delete, permission changes)
  - [ ] Exportable audit logs in JSON/CSV for security compliance
  - [ ] Real-time activity feed in WebConsole
- [ ] **3.5 High-Throughput gRPC Interface (`tonic`)**
  - [ ] Protobuf service contracts (`r2drive.v1`) for bucket management, object operations, and metadata streaming
  - [ ] Dual-protocol runtime: `tonic` gRPC server running multiplexed alongside Axum HTTP (or dedicated port)
  - [ ] High-efficiency server-streaming for large-scale object listings (millions of objects without HTTP timeouts)
  - [ ] Foundation for low-latency Inter-Process Communication (IPC over Unix Domain Sockets) with Phase 4 desktop daemons

---

### Phase 4: Virtual Drive, WebDAV & Selective Sync

**Goal**: Seamless operating system integration with local desktop file systems without opening a browser.

- [ ] **4.1 Conflict-Branching Sync Daemon (`r2drive watch`)**
  - [ ] Cross-platform file watcher monitoring local directories (`notify` crate)
  - [ ] Bi-directional sync loop reconciling local changes against R2
  - [ ] Non-destructive conflict resolution producing `(Conflicted Copy YYYY-MM-DD)` files
  - [ ] Pattern-based ignore engine (`.r2ignore`)
- [ ] **4.2 Embedded WebDAV Server**
  - [ ] Native WebDAV protocol server built into `StorageNode`
  - [ ] Direct mounting in macOS Finder, Windows File Explorer, and Linux file managers
  - [ ] Zero kernel-extension requirement (macFUSE-free)
- [ ] **4.3 Smart Selective Sync & On-Demand Hydration**
  - [ ] Lightweight 0-byte local placeholder stubs for remote files
  - [ ] Transparent on-demand download (hydration) when a file is opened by any application
  - [ ] Local LRU disk cache eviction automatically purging hydrated files to free space
