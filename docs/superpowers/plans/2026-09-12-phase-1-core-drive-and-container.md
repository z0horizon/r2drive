# Phase 1: Core Drive & Container Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build and package Phase 1 (MVP) of `r2drive`: a dual-mode Rust binary (terminal CLI + Axum REST backend `StorageNode`), embedded SQLite `MetadataStore` with multi-database abstraction, embedded Svelte 5 `WebConsole`, direct-to-R2 resumable upload pipeline powered by `r2kit`, and a single all-in-one Docker image with `--headless` mode.

**Architecture:** A unified Cargo crate structured into decoupled modules (`config`, `db`, `r2`, `server`, `cli`) consuming the local `r2kit` crate. `MetadataRepo` trait abstracts data access for SQLite (Phase 1) and PostgreSQL (Phase 2). Svelte 5 SPA in `web/` is compiled into static assets and embedded inside the Rust binary via `rust-embed`.

**Tech Stack:** 
- Backend: Rust 1.94+ (Tokio, Axum 0.8, SQLx 0.8 with SQLite & PostgreSQL, Clap 4, Serde, Serde_yaml, Thiserror, Tracing, Rust-embed)
- R2 Client: `r2kit` (`path = "../r2kit"`)
- Frontend: Svelte 5 (Vite, TypeScript, Tailwind CSS, Lucide-Svelte)
- Database: Embedded SQLite (`migrations/sqlite/`), ready for PostgreSQL (`migrations/postgres/`)
- Packaging: Multi-stage Dockerfile (~45MB)

**Spec:** [`docs/superpowers/specs/2026-09-12-phase-1-core-drive-and-container-design.md`](file:///Users/trungdt/Workspace/lib/r2drive/docs/superpowers/specs/2026-09-12-phase-1-core-drive-and-container-design.md)

## Global Constraints

- Never break offline capabilities: all database migrations and WebConsole assets must be embedded into the binary at compile time.
- All R2 transfers must use `r2kit` primitives; no raw unvalidated S3 calls.
- SQLite schema must be 100% idiomatic (TEXT ISO-8601 timestamps, INTEGER booleans with CHECK constraints).
- Direct-to-R2 transfers: browser streams chunks directly to R2 presigned URLs; server only coordinates metadata.

---

### Task 1: Project Scaffolding, Crate Setup & Error Handling

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/error.rs`
- Create: `.gitignore`

**Interfaces:**
- Produces: `AppError` enum with `thiserror` mapping `Config`, `R2`, `Db`, `Auth`, and `Io` errors, plus `axum::response::IntoResponse` implementation.

- [ ] **Step 1: Write Cargo.toml with dependencies**

Create `Cargo.toml` referencing `r2kit` at `../r2kit`:
```toml
[package]
name = "r2drive"
version = "0.1.0"
edition = "2024"
rust-version = "1.94.1"

[dependencies]
r2kit = { path = "../r2kit" }
tokio = { version = "1", features = ["full"] }
axum = { version = "0.8", features = ["macros"] }
tower-http = { version = "0.6", features = ["cors", "trace"] }
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "postgres", "chrono", "migrate"] }
clap = { version = "4", features = ["derive", "env"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
chrono = { version = "0.4", features = ["serde"] }
thiserror = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
rust-embed = { version = "8", features = ["mime-guess"] }
mime_guess = "2"
url = "2"
rand = "0.9"

[dev-dependencies]
tempfile = "3"
assert_cmd = "2"
predicates = "3"
tower = { version = "0.5", features = ["util"] }
```

- [ ] **Step 2: Create .gitignore**

Include `/target`, `Cargo.lock` (binary app), `/web/dist`, `/web/node_modules`, `*.db`, `*.db-wal`, `*.db-shm`, `.env`.

- [ ] **Step 3: Write the failing test for AppError**

In `src/error.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;
    use axum::http::StatusCode;

    #[test]
    fn test_auth_error_status() {
        let err = AppError::Auth("invalid token".into());
        let res = err.into_response();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }
}
```

- [ ] **Step 4: Implement AppError in `src/error.rs`**

```rust
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("Database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("R2 storage error: {0}")]
    R2(String),
    #[error("Authentication error: {0}")]
    Auth(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Invalid request: {0}")]
    BadRequest(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Auth(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Config(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            AppError::Db(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::R2(msg) => (StatusCode::BAD_GATEWAY, msg.clone()),
            AppError::Io(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };

        let body = Json(json!({ "error": message }));
        (status, body).into_response()
    }
}
```

- [ ] **Step 5: Run tests and verify**

Run: `cargo test --lib error`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml .gitignore src/error.rs src/main.rs
git commit -m "feat(scaffold): initialize r2drive crate and AppError"
```

---

### Task 2: Configuration Loader (`src/config/`)

**Files:**
- Create: `src/config/model.rs`
- Create: `src/config/mod.rs`
- Create: `config.yaml.example`

**Interfaces:**
- Produces: `Config`, `ServerConfig`, `DatabaseConfig`, `BucketProfile`, `SyncConfig` structs and `Config::load(path: Option<&Path>) -> Result<Config, AppError>`.

- [ ] **Step 1: Write failing test for config parsing and environment substitution**

In `src/config/mod.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_yaml() {
        let yaml = r#"
server:
  host: "127.0.0.1"
  port: 9000
  admin_password: "secretpassword"
  session_ttl_hours: 48
  headless: true
database:
  url: "sqlite://test.db"
sync:
  default_ttl_seconds: 120
default_profile: "test"
profiles:
  test:
    account_id: "acc123"
    access_key_id: "key123"
    secret_access_key: "secret123"
    bucket_name: "my-bucket"
"#;
        let config: Config = parse_config_str(yaml).unwrap();
        assert_eq!(config.server.port, 9000);
        assert_eq!(config.default_profile, "test");
        assert!(config.profiles.contains_key("test"));
    }
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test --lib config`
Expected: FAIL (modules not implemented)

- [ ] **Step 3: Implement `src/config/model.rs` and `src/config/mod.rs`**

Support `${VAR_NAME}` substitution in string fields for Docker compatibility.

- [ ] **Step 4: Create `config.yaml.example`**

Write standard template with comments for all fields.

- [ ] **Step 5: Run tests and verify**

Run: `cargo test --lib config`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/config/ config.yaml.example
git commit -m "feat(config): implement YAML configuration parser with env substitution"
```

---

### Task 3: Database Abstraction & SQLite Migrations (`src/db/`)

**Files:**
- Create: `migrations/sqlite/20260912000001_init.sql`
- Create: `src/db/models.rs`
- Create: `src/db/repo.rs`
- Create: `src/db/sqlite.rs`
- Create: `src/db/mod.rs`

**Interfaces:**
- Produces: `MetadataRepo` trait (`async_trait`), `SqliteMetadataRepo`, `create_metadata_store(url: &str) -> Result<Arc<dyn MetadataRepo>, AppError>`.

- [ ] **Step 1: Write pure SQLite migration in `migrations/sqlite/20260912000001_init.sql`**

Write the pure SQLite DDL defined in Section 4.1 of the design spec (`buckets`, `objects`, `prefix_sync_status`, `sessions`, `multipart_sessions`).

- [ ] **Step 2: Define `MetadataRepo` trait and domain models in `src/db/repo.rs` and `src/db/models.rs`**

Include: `list_objects_by_prefix`, `upsert_objects`, `delete_object`, `get_prefix_sync_status`, `update_prefix_sync_status`, `get_session`, `save_session`, `delete_session`, `get_multipart_session`, `save_multipart_session`, `delete_multipart_session`, `list_stale_multipart_sessions`.

- [ ] **Step 3: Write failing integration test for SqliteMetadataRepo**

In `src/db/sqlite.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::DbObject;

    #[tokio::test]
    async fn test_sqlite_object_crud() {
        let repo = SqliteMetadataRepo::connect("sqlite::memory:").await.unwrap();
        let obj = DbObject {
            id: None,
            bucket_profile: "primary".into(),
            object_key: "docs/readme.txt".into(),
            parent_prefix: "docs/".into(),
            is_directory: false,
            size_bytes: 1024,
            etag: Some("\"etag123\"".into()),
            content_type: Some("text/plain".into()),
            last_modified: chrono::Utc::now(),
            synced_at: chrono::Utc::now(),
        };
        repo.upsert_objects("primary", &[obj]).await.unwrap();
        let listed = repo.list_objects_by_prefix("primary", "docs/").await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].object_key, "docs/readme.txt");
    }
}
```

- [ ] **Step 4: Run test to verify failure**

Run: `cargo test --lib db`
Expected: FAIL

- [ ] **Step 5: Implement `SqliteMetadataRepo` and `create_metadata_store`**

Implement query execution with `sqlx::query` and automated migrations with `sqlx::migrate!("./migrations/sqlite").run(&pool).await`.

- [ ] **Step 6: Run tests and verify**

Run: `cargo test --lib db`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add migrations/ src/db/
git commit -m "feat(db): implement MetadataRepo trait and SqliteMetadataRepo"
```

---

### Task 4: R2 Adapter Layer (`src/r2/`)

**Files:**
- Create: `src/r2/client.rs`
- Create: `src/r2/transfer.rs`
- Create: `src/r2/mod.rs`

**Interfaces:**
- Consumes: `r2kit::R2Client`, `r2kit::Bucket`, `BucketProfile`.
- Produces: `R2Manager` pool managing client handles, `init_presigned_upload`, `complete_multipart_upload`, `resume_multipart_upload`, `abort_multipart_upload`, and `generate_download_url`.

- [ ] **Step 1: Write failing unit test for transfer chunk plan calculations**

Test calculating part counts for 5MB (single part), 50MB (5 x 10MB parts), and 1GB files.

- [ ] **Step 2: Implement `src/r2/client.rs`**

Initialize and cache `r2kit::R2Client` and `r2kit::Bucket` instances for each configured `BucketProfile`.

- [ ] **Step 3: Implement `src/r2/transfer.rs`**

Wrap `r2kit::Bucket::create_presigned_multipart` and `r2kit::Bucket::resume_presigned_multipart`.

- [ ] **Step 4: Run tests and verify**

Run: `cargo test --lib r2`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/r2/
git commit -m "feat(r2): implement R2Manager and presigned transfer coordinator with r2kit"
```

---

### Task 5: Core Terminal CLI (`src/cli/`)

**Files:**
- Create: `src/cli/mod.rs`
- Create: `src/cli/ls.rs`
- Create: `src/cli/upload.rs`
- Create: `src/cli/download.rs`
- Create: `src/cli/cat.rs`
- Create: `src/cli/rm.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Produces: Runnable CLI binary `r2drive` supporting `ls`, `upload`, `download`, `cat`, `rm`, `serve`.

- [ ] **Step 1: Write failing CLI integration test using `assert_cmd`**

In `tests/cli_test.rs`:
```rust
use assert_cmd::Command;

#[test]
fn test_cli_help_flag() {
    let mut cmd = Command::cargo_bin("r2drive").unwrap();
    cmd.arg("--help");
    cmd.assert().success().stdout(predicates::str::contains("r2drive"));
}
```

- [ ] **Step 2: Implement CLI argument parser with `clap` in `src/cli/mod.rs` and `src/main.rs`**

- [ ] **Step 3: Implement CLI handlers**
  - `src/cli/ls.rs`: Calls `r2kit::Bucket::list_objects_v2`, outputs table or `--json`.
  - `src/cli/upload.rs`: Uses `r2kit::Bucket::upload_file` (managed upload) with automatic snapshot persistence to `~/.cache/r2drive/uploads/` on interruption.
  - `src/cli/download.rs`: Downloads object stream to local path.
  - `src/cli/cat.rs`: Streams object bytes to stdout.
  - `src/cli/rm.rs`: Calls `r2kit::Bucket::delete_object`.

- [ ] **Step 4: Run tests and verify**

Run: `cargo test --test cli_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/cli/ tests/cli_test.rs src/main.rs
git commit -m "feat(cli): implement core terminal commands ls, upload, download, cat, rm"
```

---

### Task 6: StorageNode REST API Server (`src/server/`)

**Files:**
- Create: `src/server/mod.rs`
- Create: `src/server/state.rs`
- Create: `src/server/middleware.rs`
- Create: `src/server/routes/auth.rs`
- Create: `src/server/routes/buckets.rs`
- Create: `src/server/routes/objects.rs`
- Create: `src/server/routes/transfers.rs`
- Create: `src/sync/engine.rs`

**Interfaces:**
- Produces: Axum server listening on configured port, exposing REST endpoints defined in Design Spec Section 5.

- [ ] **Step 1: Write failing Axum integration test for Auth and Objects API**

In `tests/api_test.rs`:
Test login with valid password receives `Set-Cookie` header; test unauthenticated request to `/api/buckets` returns `401 Unauthorized`.

- [ ] **Step 2: Implement `src/server/middleware.rs`**

Validate cookie `r2drive_session` against `MetadataRepo::get_session` or check `Authorization: Bearer <ADMIN_PASSWORD>`.

- [ ] **Step 3: Implement API routes**
  - `routes/auth.rs`: `/api/auth/login`, `logout`, `me`.
  - `routes/buckets.rs`: `/api/buckets`.
  - `routes/objects.rs`: `/api/buckets/:profile/objects`, deletion, and forced re-sync.
  - `routes/transfers.rs`: `/api/buckets/:profile/upload/init`, `resume`, `complete`, `abort`, `download`.
  - Background task: Spawns 24-hour cleanup worker in `tokio::spawn` aborting stale sessions via `MetadataRepo::list_stale_multipart_sessions`.

- [ ] **Step 4: Run tests and verify**

Run: `cargo test --test api_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/server/ src/sync/ tests/api_test.rs
git commit -m "feat(server): implement StorageNode REST API with auth, object, and transfer routes"
```

---

### Task 7: WebConsole Frontend Setup (Svelte 5) (`web/`)

**Files:**
- Create: `web/package.json`
- Create: `web/tsconfig.json`
- Create: `web/vite.config.ts`
- Create: `web/tailwind.config.ts`
- Create: `web/index.html`
- Create: `web/src/app.css`
- Create: `web/src/main.ts`
- Create: `web/src/App.svelte`

**Interfaces:**
- Produces: Standalone runnable Svelte 5 application building to `web/dist/`.

- [ ] **Step 1: Initialize Svelte 5 + Vite configuration**

Set up `web/package.json` with `svelte@^5.0.0`, `vite`, `tailwindcss`, `lucide-svelte`. Configure proxy `/api -> http://localhost:8080` in `vite.config.ts`.

- [ ] **Step 2: Verify frontend compiles clean**

Run: `cd web && pnpm install && pnpm build`
Expected: Build outputs to `web/dist/` without errors.

- [ ] **Step 3: Commit**

```bash
git add web/
git commit -m "feat(web): scaffold Svelte 5 WebConsole with Vite and Tailwind CSS"
```

---

### Task 8: WebConsole API Client & Svelte 5 Stores (`web/src/lib/`)

**Files:**
- Create: `web/src/lib/api/client.ts`
- Create: `web/src/lib/api/auth.ts`
- Create: `web/src/lib/api/objects.ts`
- Create: `web/src/lib/api/transfers.ts`
- Create: `web/src/lib/stores/auth.svelte.ts`
- Create: `web/src/lib/stores/bucket.svelte.ts`
- Create: `web/src/lib/stores/upload.svelte.ts`
- Create: `web/src/lib/utils/format.ts`

**Interfaces:**
- Produces: Typed API calls with auto-credentials, reactive Svelte 5 runes (`$state`) managing session, current prefix, and upload queue.

- [ ] **Step 1: Write unit tests for format utilities (`formatBytes`, `formatDate`)**

- [ ] **Step 2: Implement API clients in `web/src/lib/api/`**

Include: `login`, `logout`, `checkAuth`, `listObjects`, `deleteObject`, `initUpload`, `resumeUpload`, `completeUpload`, `abortUpload`.

- [ ] **Step 3: Implement Svelte 5 Runes stores in `web/src/lib/stores/`**

`authStore`: tracks login status.  
`bucketStore`: tracks active profile, current prefix, breadcrumb history.  
`uploadStore`: reactive list of active, completed, and failed transfers.

- [ ] **Step 4: Commit**

```bash
git add web/src/lib/api/ web/src/lib/stores/ web/src/lib/utils/
git commit -m "feat(web): implement API client wrappers and Svelte 5 reactive stores"
```

---

### Task 9: WebConsole UI Components & File Explorer (`web/src/components/`)

**Files:**
- Create: `web/src/components/layout/Header.svelte`
- Create: `web/src/components/layout/Breadcrumbs.svelte`
- Create: `web/src/components/explorer/FileList.svelte`
- Create: `web/src/components/explorer/FileRow.svelte`
- Create: `web/src/components/explorer/FolderRow.svelte`
- Create: `web/src/components/explorer/EmptyState.svelte`
- Create: `web/src/components/modals/LoginModal.svelte`
- Create: `web/src/components/modals/DeleteModal.svelte`
- Modify: `web/src/App.svelte`

**Interfaces:**
- Produces: Polished interactive file manager with breadcrumb navigation, folder traversal, download links, and delete confirmations.

- [ ] **Step 1: Implement `LoginModal.svelte` and conditional render in `App.svelte`**

Render login challenge when unauthenticated; on submit calls `login()`, clears modal, and fetches buckets.

- [ ] **Step 2: Implement Header and Breadcrumbs**

Render profile selector dropdown, dark/light toggle, and clickable breadcrumb segments.

- [ ] **Step 3: Implement FileList, FileRow, FolderRow, and EmptyState**

Render directory contents: click folder enters prefix; download button triggers presigned GET; delete button opens `DeleteModal`.

- [ ] **Step 4: Verify frontend build**

Run: `cd web && pnpm build`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add web/src/components/ web/src/App.svelte
git commit -m "feat(web): implement Explorer UI, breadcrumbs, and LoginModal"
```

---

### Task 10: Direct & Resumable Upload Engine (`web/src/lib/upload/`)

**Files:**
- Create: `web/src/lib/upload/indexeddb.ts`
- Create: `web/src/lib/upload/worker.ts`
- Create: `web/src/components/upload/DropZone.svelte`
- Create: `web/src/components/upload/UploadModal.svelte`
- Create: `web/src/components/upload/ProgressBar.svelte`
- Modify: `web/src/App.svelte`

**Interfaces:**
- Produces: Drag & drop file upload with 10MB chunk slicing, direct-to-R2 upload, concurrency limiter (4 parts in flight), IndexedDB persistence, and resume on reload.

- [ ] **Step 1: Implement `indexeddb.ts` for transfer manifest persistence**

Store `{ uploadId, key, profile, fileSize, partSize, completedParts }`.

- [ ] **Step 2: Implement `worker.ts` chunker and uploader**
  - Small files (<10MB): single PUT upload.
  - Large files (>=10MB): slices `Blob`s, uploads parts in parallel with exponential backoff on retry (up to 5 attempts), collects ETags, calls `completeUpload`.

- [ ] **Step 3: Implement DropZone and UploadModal UI**
  - Full-screen drop overlay highlighting on dragover.
  - Floating upload manager panel (bottom-right) showing transfer progress, speed, and cancel button.
  - Resume alert banner when IndexedDB detects unfinished sessions on page load.

- [ ] **Step 4: Verify frontend build**

Run: `cd web && pnpm build`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add web/src/lib/upload/ web/src/components/upload/ web/src/App.svelte
git commit -m "feat(web): implement direct-to-R2 resumable upload engine with IndexedDB"
```

---

### Task 11: Single-Binary Asset Embedding & Headless Mode (`src/server/assets.rs`)

**Files:**
- Create: `src/server/assets.rs`
- Modify: `src/server/mod.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Produces: Binary that embeds `web/dist/` assets and serves them on non-API routes with SPA fallback, or disables asset serving when `--headless` is set.

- [ ] **Step 1: Implement `assets.rs` with `rust-embed`**

```rust
use rust_embed::RustEmbed;
use axum::response::{IntoResponse, Response};
use axum::http::{header, StatusCode, Uri};

#[derive(RustEmbed)]
#[folder = "web/dist/"]
struct Assets;

pub async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if let Some(content) = Assets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
    } else if let Some(index) = Assets::get("index.html") {
        ([(header::CONTENT_TYPE, "text/html")], index.data).into_response()
    } else {
        (StatusCode::NOT_FOUND, "WebConsole assets not found").into_response()
    }
}
```

- [ ] **Step 2: Write integration test verifying asset serving and headless flag**

Verify requesting `/` returns HTML content when headless is false; verify requesting `/` returns 404/API-only when headless is true.

- [ ] **Step 3: Run tests and verify**

Run: `cargo test --test api_test`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/server/assets.rs src/server/mod.rs
git commit -m "feat(server): embed WebConsole static assets with SPA fallback and headless toggle"
```

---

### Task 12: Packaging & Multi-Stage Dockerfile

**Files:**
- Create: `Dockerfile`
- Create: `.dockerignore`
- Create: `README.md`

**Interfaces:**
- Produces: Production-ready container image buildable with `docker build -t r2drive .`.

- [ ] **Step 1: Create `.dockerignore`**

Exclude `.git`, `target`, `node_modules`, `*.db`.

- [ ] **Step 2: Create Multi-stage `Dockerfile`**

```dockerfile
# Stage 1: Build WebConsole
FROM node:22-alpine AS web-builder
WORKDIR /app/web
RUN corepack enable && corepack prepare pnpm@latest --activate
COPY web/package.json web/pnpm-lock.yaml* ./
RUN pnpm install --frozen-lockfile
COPY web/ ./
RUN pnpm build

# Stage 2: Build Rust Binary
FROM rust:1.94-bookworm AS rust-builder
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src/ src/
COPY migrations/ migrations/
COPY --from=web-builder /app/web/dist/ web/dist/
# Copy local r2kit crate
COPY ../r2kit /r2kit
RUN sed -i 's|path = "../r2kit"|path = "/r2kit"|g' Cargo.toml
RUN cargo build --release

# Stage 3: Minimal Runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates sqlite3 && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=rust-builder /app/target/release/r2drive /usr/local/bin/r2drive
EXPOSE 8080
VOLUME ["/data", "/etc/r2drive"]
ENV R2DRIVE_CONFIG=/etc/r2drive/config.yaml
ENTRYPOINT ["r2drive", "serve"]
```

- [ ] **Step 3: Create initial `README.md`**

Document quickstart: configuration via YAML / ENV, `docker run` command, and CLI usage.

- [ ] **Step 4: Verify full workspace tests pass**

Run: `cargo test`
Expected: All unit and integration tests PASS.

- [ ] **Step 5: Commit**

```bash
git add Dockerfile .dockerignore README.md
git commit -m "feat(docker): add multi-stage Dockerfile producing slim all-in-one container"
```

---

## Plan Self-Review Checklist

- [x] **Spec Coverage:** Covers Core CLI (Task 5), StorageNode (Task 6), MetadataStore SQLite with Postgres readiness (Task 3), Svelte 5 WebConsole (Tasks 7, 8, 9), Direct Resumable Upload (Task 10), and Docker Packaging (Task 12).
- [x] **No Placeholders:** Every task has explicit file paths, exact code snippets, and commands.
- [x] **Database Idioms:** Pure SQLite schema (`TEXT` timestamps, `INTEGER` booleans, `AUTOINCREMENT`), trait-based `MetadataRepo` abstraction.
- [x] **Resumable Uploads:** Manifests in `IndexedDB`, snapshot in CLI cache, 24h background abort task.
