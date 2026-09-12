# Phase 1: Core Drive & Container Design Specification

- **Feature**: Phase 1 MVP — Core Drive & Container
- **Date**: 2026-09-12
- **Status**: Approved Design
- **Target Repository**: `r2drive`
- **Underlying Engine**: `r2kit` (Cloudflare R2 Rust Toolkit)

---

## 1. Overview & Objectives

Phase 1 establishes `r2drive` as a self-hosted Cloudflare R2 drive service packaged as a single all-in-one Docker container and a dual-mode terminal CLI. It provides:
1. **Core CLI Engine**: Unix-style terminal commands (`ls`, `upload`, `download`, `cat`, `rm`, `serve`) communicating with Cloudflare R2 via `r2kit`.
2. **StorageNode Backend**: High-performance asynchronous HTTP service built with Axum, handling authentication, bucket profiles, metadata caching, and presigned transfer coordination.
3. **MetadataStore**: Embedded SQLite database managed through `sqlx`, enabling sub-millisecond directory and object browsing with on-demand cache freshness invalidation.
4. **WebConsole SPA**: Modern browser file manager built with Svelte 5, Vite, and Tailwind CSS, embedded directly into the Rust executable via `rust-embed`.
5. **Direct-to-R2 Data Plane**: Multipart upload coordination using presigned URLs generated via `r2kit`, streaming file chunks directly from browser to Cloudflare R2 with zero server bandwidth overhead.
6. **Container Distribution**: Multi-stage Docker image (~45MB) with optional `--headless` mode.

---

## 2. Architecture & Component Layout

```
r2drive/
├── Cargo.toml
├── config.yaml.example
├── Dockerfile
├── migrations/
│   ├── sqlite/
│   │   └── 20260912000001_init.sql
│   └── postgres/               # Ready for Phase 2
├── src/
│   ├── main.rs                 # CLI entry point, logging/tracing setup, subcommand dispatcher
│   ├── error.rs                # Unified AppError enum converting to HTTP responses and CLI exits
│   ├── config/
│   │   ├── mod.rs              # Configuration loader (config.yaml + environment variables)
│   │   └── model.rs            # Config, BucketProfile, ServerConfig struct definitions
│   ├── r2/
│   │   ├── mod.rs
│   │   ├── client.rs           # Multi-bucket client pool managing r2kit::R2Client instances
│   │   └── transfer.rs         # Presigned upload/download coordination using r2kit
│   ├── db/
│   │   ├── mod.rs              # Factory: detect sqlite:// vs postgres:// and initialize repo
│   │   ├── repo.rs             # MetadataRepo trait defining all storage operations
│   │   ├── models.rs           # Pure domain entities: DbBucket, DbObject, DbSession, DbMultipartSession
│   │   ├── sqlite.rs           # SqliteMetadataRepo implementing MetadataRepo using sqlx::SqlitePool
│   │   └── postgres.rs         # PostgresMetadataRepo scaffolding for Phase 2
│   ├── sync/
│   │   └── engine.rs           # CacheFreshness TTL checks and R2 prefix re-sync engine
│   ├── server/
│   │   ├── mod.rs              # Axum router setup, TCP binding, and graceful shutdown
│   │   ├── state.rs            # AppState holding Arc<Config>, Arc<SqlitePool>, Arc<R2Manager>
│   │   ├── middleware.rs       # Auth middleware validating HTTP-only cookie or Bearer token
│   │   ├── assets.rs           # rust-embed handler for embedded Svelte SPA with index.html fallback
│   │   └── routes/
│   │       ├── auth.rs         # POST /api/auth/login, POST /api/auth/logout, GET /api/auth/me
│   │       ├── buckets.rs      # GET /api/buckets, GET /api/buckets/:profile
│   │       ├── objects.rs      # GET /api/buckets/:profile/objects, DELETE, POST .../refresh
│   │       └── transfers.rs    # POST .../upload/init, POST .../upload/complete, GET .../download
│   └── cli/
│       ├── mod.rs              # Subcommand routing
│       ├── ls.rs               # r2drive ls
│       ├── upload.rs           # r2drive upload (streaming chunked upload via r2kit)
│       ├── download.rs         # r2drive download
│       ├── rm.rs               # r2drive rm
│       └── cat.rs              # r2drive cat
└── web/
    ├── package.json
    ├── vite.config.ts          # API proxy (/api -> http://localhost:8080) for development
    ├── tailwind.config.ts
    ├── index.html
    └── src/
        ├── app.css
        ├── App.svelte          # Root layout and authentication view switcher
        ├── main.ts
        └── lib/
            ├── api/            # Typed REST API client wrappers
            ├── stores/         # Svelte 5 Runes stores ($state) for auth, navigation, upload queue
            ├── upload/         # Browser multipart chunker and direct-to-R2 uploader
            └── components/     # Header, Breadcrumbs, FileList, DropZone, UploadModal, LoginModal
```

---

## 3. Configuration Specification (`config.yaml`)

Configuration is loaded from the path specified by `--config <PATH>`, or `$R2DRIVE_CONFIG`, or defaulting to `~/.config/r2drive/config.yaml` on host machines and `/etc/r2drive/config.yaml` in Docker.

```yaml
server:
  host: "0.0.0.0"
  port: 8080
  admin_password: "change-me-in-production"
  jwt_secret: "auto-generated-random-string-or-custom"
  session_ttl_hours: 72
  headless: false

database:
  url: "sqlite:///data/r2drive.db?mode=rwc"

sync:
  default_ttl_seconds: 60

default_profile: "primary"

profiles:
  primary:
    account_id: "${R2_ACCOUNT_ID}"
    access_key_id: "${R2_ACCESS_KEY_ID}"
    secret_access_key: "${R2_SECRET_ACCESS_KEY}"
    bucket_name: "${R2_BUCKET_NAME}"
    public_url: "https://pub.example.com" # optional custom domain
```

---

## 4. Multi-Database Architecture & Schema

To support both `sqlite:///...` and `postgres://...` in `config.yaml` without lowest-common-denominator compromises, `r2drive` establishes a trait-based repository architecture:
- `MetadataRepo` trait (`src/db/repo.rs`) provides the abstract interface for all storage queries.
- Scheme detection in `src/db/mod.rs` instantiates either `SqliteMetadataRepo` or `PostgresMetadataRepo`.
- Migrations are separated into engine-specific directories: `migrations/sqlite/` and `migrations/postgres/`.

### 4.1 SQLite Schema (`migrations/sqlite/20260912000001_init.sql`)

```sql
-- Managed bucket profiles
CREATE TABLE IF NOT EXISTS buckets (
    id TEXT PRIMARY KEY,
    profile_name TEXT NOT NULL UNIQUE,
    bucket_name TEXT NOT NULL,
    account_id TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Cached objects & prefixes
CREATE TABLE IF NOT EXISTS objects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    bucket_profile TEXT NOT NULL,
    object_key TEXT NOT NULL,
    parent_prefix TEXT NOT NULL,
    is_directory INTEGER NOT NULL DEFAULT 0 CHECK (is_directory IN (0, 1)),
    size_bytes INTEGER NOT NULL DEFAULT 0,
    etag TEXT,
    content_type TEXT,
    last_modified TEXT NOT NULL,
    synced_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(bucket_profile, object_key)
);

CREATE INDEX IF NOT EXISTS idx_objects_lookup 
ON objects(bucket_profile, parent_prefix, is_directory);

-- Prefix sync timestamp for CacheFreshness TTL
CREATE TABLE IF NOT EXISTS prefix_sync_status (
    bucket_profile TEXT NOT NULL,
    prefix TEXT NOT NULL,
    last_synced_at TEXT NOT NULL,
    PRIMARY KEY (bucket_profile, prefix)
);

-- Admin authentication sessions
CREATE TABLE IF NOT EXISTS sessions (
    token_hash TEXT PRIMARY KEY,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    expires_at TEXT NOT NULL
);

-- Active and resumable multipart upload sessions
CREATE TABLE IF NOT EXISTS multipart_sessions (
    upload_id TEXT PRIMARY KEY,
    bucket_profile TEXT NOT NULL,
    object_key TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    part_size INTEGER NOT NULL,
    total_parts INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_activity_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
```

### 4.2 PostgreSQL Schema Reference (`migrations/postgres/20260912000001_init.sql` - Phase 2)
In PostgreSQL, `objects.id` uses `BIGINT GENERATED ALWAYS AS IDENTITY`, timestamps use native `TIMESTAMPTZ`, and booleans use native `BOOLEAN`. The `MetadataRepo` trait maps both engines into pure domain models.

---

## 5. REST API Contracts

### 5.1 Authentication
- **`POST /api/auth/login`**:
  - Request: `{ "password": "..." }`
  - Response: `{ "status": "authenticated", "expires_at": "..." }`
  - Sets Header: `Set-Cookie: r2drive_session=<TOKEN>; HttpOnly; SameSite=Lax; Path=/`
- **`POST /api/auth/logout`**:
  - Clears `r2drive_session` cookie and purges database session record.
- **`GET /api/auth/me`**:
  - Checks current cookie or `Authorization: Bearer <TOKEN>`. Returns `{ "authenticated": true }`.

### 5.2 Bucket Management
- **`GET /api/buckets`**:
  - Returns list of configured `BucketProfile`s: `[{ "name": "primary", "bucket": "my-r2-bucket", "default": true }]`.

### 5.3 Object Browsing & Management
- **`GET /api/buckets/:profile/objects?prefix=path/to/&refresh=false`**:
  - Evaluates `CacheFreshness` in `MetadataStore`.
  - If TTL expired or `refresh=true`: dispatches `r2kit` bucket listing, updates `objects` table, and updates `prefix_sync_status`.
  - Returns structured listing separating virtual directories and files:
    ```json
    {
      "prefix": "photos/",
      "directories": ["photos/2026/", "photos/vacation/"],
      "objects": [
        {
          "key": "photos/dog.jpg",
          "name": "dog.jpg",
          "size_bytes": 1048576,
          "etag": "\"abc123etag\"",
          "content_type": "image/jpeg",
          "last_modified": "2026-09-12T10:00:00Z"
        }
      ],
      "synced_at": "2026-09-12T17:30:00Z"
    }
    ```
- **`DELETE /api/buckets/:profile/objects?key=path/to/file.ext`**:
  - Calls `r2kit` bucket object deletion and purges record from `MetadataStore`.

### 5.4 Transfer Coordination
- **`POST /api/buckets/:profile/upload/init`**:
  - Request: `{ "key": "video.mp4", "size_bytes": 104857600, "content_type": "video/mp4" }`
  - For small files (<10MB):
    - Generates a single Presigned PUT URL.
    - Response: `{ "mode": "single", "upload_url": "https://..." }`
  - For large files (>=10MB):
    - Initializes an `r2kit::MultipartSession`.
    - Generates 10MB chunk presigned part URLs.
    - Response:
      ```json
      {
        "mode": "multipart",
        "upload_id": "r2-upload-session-id",
        "part_size": 10485760,
        "parts": [
          { "part_number": 1, "url": "https://..." },
          { "part_number": 2, "url": "https://..." }
        ]
      }
      ```
- **`POST /api/buckets/:profile/upload/complete`**:
  - Request:
    ```json
    {
      "key": "video.mp4",
      "upload_id": "r2-upload-session-id",
      "parts": [
        { "part_number": 1, "etag": "\"etag1\"" },
        { "part_number": 2, "etag": "\"etag2\"" }
      ]
    }
    ```
- **`POST /api/buckets/:profile/upload/resume`**:
  - Request: `{ "upload_id": "r2-upload-session-id" }`
  - Calls `r2kit`'s `resume_presigned_multipart` using the stored snapshot.
  - Queries Cloudflare R2 for completed parts (`ListParts`), identifies missing part numbers, and generates presigned URLs only for the remaining chunks.
  - Response:
    ```json
    {
      "upload_id": "r2-upload-session-id",
      "completed_parts": [{ "part_number": 1, "etag": "\"etag1\"" }],
      "remaining_parts": [{ "part_number": 2, "url": "https://..." }]
    }
    ```
- **`POST /api/buckets/:profile/upload/abort`**:
  - Request: `{ "upload_id": "r2-upload-session-id" }`
  - Calls `r2kit`'s `abort_multipart` to purge in-flight chunks on R2, and removes session from `multipart_sessions` table.
- **`GET /api/buckets/:profile/download?key=path/to/file.ext`**:
  - Returns presigned GET URL with 1-hour expiration.

---

## 6. WebConsole Architecture (Svelte 5)

1. **Reactive State via Runes**:
   - `authStore`: Tracks authentication status (`$state({ isAuthenticated: false, loading: true })`).
   - `navStore`: Tracks selected bucket profile, current prefix, breadcrumbs (`$state({ profile: 'primary', prefix: '' })`).
   - `uploadQueueStore`: Manages array of in-flight and completed transfers with reactivity on transfer progress, speed calculation, and retry counters.
2. **Chunked Direct & Resumable Upload Engine**:
   - Web browser reads selected files via HTML5 Drag & Drop or file picker.
   - Slices files into 10MB `Blob` parts using `File.prototype.slice()`.
   - Executes parallel `fetch(partUrl, { method: 'PUT', body: chunk })` constrained to a maximum concurrency of 4 simultaneous chunk uploads per file.
   - Automatically retries transient part failures using exponential backoff (up to 5 attempts).
   - Persists transfer state (`uploadId`, `fileSize`, `partSize`, `completedParts`) in browser `IndexedDB`.
   - Upon page refresh, prompts users to resume interrupted uploads by re-selecting the source file and calling `/api/buckets/:profile/upload/resume`.
   - Collects `ETag` response headers and dispatches completion payload to `/api/buckets/:profile/upload/complete`.
3. **Styling & Assets**:
   - Modern Tailwind CSS design supporting automatic dark/light theme switching.
   - Minimalist file icons rendered using `lucide-svelte`.

---

## 7. Packaging & Distribution

### 7.1 Multi-Stage Dockerfile
- **Stage 1 (Frontend Build)**: Node.js 22 alpine, installs dependencies with `pnpm`, compiles Svelte 5 into `web/dist/`.
- **Stage 2 (Rust Backend Build)**: Rust official bookworm/alpine builder, compiles `r2drive` binary with `rust-embed` bundling `web/dist/`.
- **Stage 3 (Runtime)**: Distroless or Debian slim image with `ca-certificates` and `sqlite3` runtime libraries. Total image size ~45MB.

### 7.2 CLI Commands
- `r2drive serve [--port <PORT>] [--headless]`: Launches HTTP daemon and WebConsole. Runs automated background 24-hour cleanup worker for orphaned multipart sessions.
- `r2drive ls [PROFILE] [PREFIX] [--recursive] [--json]`: Lists directory contents.
- `r2drive upload [PROFILE] <LOCAL_PATH> [REMOTE_PATH]`: Performs managed chunked transfer directly via `r2kit`. Automatically writes `MultipartSessionSnapshot` to `~/.cache/r2drive/uploads/` and resumes if re-run on an interrupted file.
- `r2drive download [PROFILE] <REMOTE_PATH> [LOCAL_PATH]`: Downloads file to local disk.
- `r2drive cat [PROFILE] <REMOTE_PATH>`: Streams object directly to stdout.
- `r2drive rm [PROFILE] <REMOTE_PATH> [--yes]`: Deletes remote object.

---

## 8. Verification & Testing Strategy

1. **Unit Tests**:
   - Configuration parser unit tests verifying valid YAML, missing fields, and environment variable substitution.
   - Error conversion tests ensuring `AppError` maps to proper HTTP status codes.
2. **Integration Tests (SQLite & API)**:
   - In-memory SQLite tests (`sqlite::memory:`) running migrations and validating `repo.rs` queries.
   - Axum test server using `tower::ServiceExt` testing `/api/auth`, `/api/buckets`, and session cookie validation.
3. **R2 Integration Tests**:
   - Live/mock multipart transfer tests against `r2kit` verifying chunk calculation and part completion manifests.
4. **End-to-End CLI Tests**:
   - `assert_cmd` test suite asserting exit codes and formatted stdout for `r2drive ls --json`, `r2drive upload`, and `r2drive rm`.
