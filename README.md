# r2drive

[![CI](https://img.shields.io/badge/build-passing-brightgreen.svg)]()
[![Rust](https://img.shields.io/badge/rust-1.94%2B-orange.svg)]()
[![Svelte](https://img.shields.io/badge/svelte-5.0-red.svg)]()
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)]()

A modern, self-hosted Cloudflare R2 object storage drive service packaged as an ultra-slim, all-in-one Docker container and single binary. Built on top of [`r2kit`](https://crates.io/crates/r2kit), `r2drive` combines a high-throughput **StorageNode** REST service, an embedded **WebConsole** single-page application (SPA), and a full-featured terminal **CLI**.

---

## Table of Contents

- [Overview](#overview)
- [Key Features](#key-features)
- [Architecture](#architecture)
- [Quickstart with Docker](#quickstart-with-docker)
- [Configuration Guide](#configuration-guide)
  - [Config Precedence](#config-precedence)
  - [Configuration File (`config.yaml`)](#configuration-file-configyaml)
  - [Environment Variable Substitution](#environment-variable-substitution)
- [CLI Usage Guide](#cli-usage-guide)
  - [Global Options](#global-options)
  - [Commands & Examples](#commands--examples)
- [WebConsole Guide](#webconsole-guide)
- [REST API Reference](#rest-api-reference)
- [Development & Testing](#development--testing)
- [License](#license)

---

## Overview

Managing Cloudflare R2 buckets often requires juggling heavy third-party S3 clients, web dashboards, or custom scripts. `r2drive` provides a unified solution:

1. **StorageNode**: An asynchronous, high-performance Axum REST server managing bucket profiles, caching object metadata, orchestrating multipart uploads, and running automated background maintenance tasks.
2. **WebConsole**: A modern Svelte 5 browser-based file management application embedded directly into the binary with zero external asset dependencies.
3. **CLI**: A standalone command-line client for direct bucket interactions, file transfers, and streaming.

Everything compiles down to a **single, standalone binary** containing all database migrations and frontend assets.

---

## Key Features

- 🚀 **Direct-to-R2 Resumable Multipart Uploads (`PresignedTransfer`)**: Browser and client uploads stream chunks directly to Cloudflare R2 using presigned URLs generated via `r2kit`. Large uploads never proxy through or bottleneck the StorageNode server.
- 🔄 **Resumable Upload Sessions**: Uploads can be paused, resumed, or recovered after network interruptions or browser reloads. Chunks and ETags are tracked in browser `IndexedDB` and verified against the backend `MetadataStore`.
- 📦 **Zero-Dependency Single Binary**: Svelte 5 WebConsole assets are embedded at compile time with `rust-embed`. SQLite migrations are embedded and run automatically on startup via `sqlx::migrate!`.
- ⚡ **Local Metadata Caching (`MetadataStore`)**: Fast, sub-millisecond directory and prefix listings backed by a local SQLite database (designed with PostgreSQL expansion readiness) with configurable cache TTL (`CacheFreshness`).
- 🧹 **Automated Stale Upload Cleanup**: A background worker periodically inspects incomplete multipart uploads older than 24 hours and issues abort commands to Cloudflare R2, preventing orphaned part charges.
- 🗂️ **Multi-Bucket Profile Support (`BucketProfile`)**: Manage multiple Cloudflare accounts and buckets within a single instance or CLI session.
- 🔒 **Secure Admin Authentication**: Protected endpoints and WebConsole sessions authenticated with secure JWT tokens via HttpOnly cookies and bearer authorization headers.
- 🖥️ **Headless Server Mode**: Toggle off WebConsole asset hosting (`--headless`) for lightweight API-only microservice or edge node deployments.

---

## Architecture

```
                                  +-----------------------------+
                                  |     Cloudflare R2 Bucket    |
                                  +-----------------------------+
                                     ^                       ^
                    Direct Presigned |                       | Direct S3 API
                    Chunk PUT / GET  |                       | (metadata & presign)
                                     v                       v
+-------------------+      +-------------------+   +--------------------+
|    WebConsole     | <--> |    StorageNode    |   |     r2drive CLI    |
| (Svelte 5 / SPA)  | HTTP |  (Axum REST API)  |   | (Terminal Client)  |
+-------------------+      +-------------------+   +--------------------+
         ^                           |                        |
         |                           v                        v
         |                 +--------------------+   +--------------------+
         +---------------- |   MetadataStore    |   |   Local Snapshot   |
          Embedded Assets  | (SQLite / SQLx DB) |   |   Upload Cache     |
                           +--------------------+   +--------------------+
```

---

## Quickstart with Docker

The fastest way to deploy `r2drive` is via Docker. The multi-stage container image is based on Debian Bookworm Slim, packaging the compiled Rust binary, embedded WebConsole, and SQLite runtime.

### 1. Prepare Configuration

Create a local configuration directory with your `config.yaml`:

```bash
mkdir -p ./config ./data
cp config.yaml.example ./config/config.yaml
```

Set your Cloudflare R2 credentials in `./config/config.yaml` or pass them as environment variables.

### 2. Run with `docker run`

```bash
docker run -d \
  --name r2drive \
  -p 8080:8080 \
  -v "$(pwd)/config:/etc/r2drive:ro" \
  -v "$(pwd)/data:/data" \
  -e R2_ACCOUNT_ID="your-cloudflare-account-id" \
  -e R2_ACCESS_KEY_ID="your-r2-access-key-id" \
  -e R2_SECRET_ACCESS_KEY="your-r2-secret-access-key" \
  -e R2_BUCKET_NAME="my-bucket" \
  -e R2DRIVE_ADMIN_PASSWORD="super-secret-password" \
  r2drive:latest
```

The WebConsole is now accessible at `http://localhost:8080`.

### 3. Run with `docker-compose`

```yaml
version: "3.8"

services:
  r2drive:
    image: r2drive:latest
    build: .
    restart: unless-stopped
    ports:
      - "8080:8080"
    volumes:
      - ./config:/etc/r2drive:ro
      - ./data:/data
    environment:
      - R2DRIVE_CONFIG=/etc/r2drive/config.yaml
      - R2_ACCOUNT_ID=${R2_ACCOUNT_ID}
      - R2_ACCESS_KEY_ID=${R2_ACCESS_KEY_ID}
      - R2_SECRET_ACCESS_KEY=${R2_SECRET_ACCESS_KEY}
      - R2_BUCKET_NAME=${R2_BUCKET_NAME}
      - R2DRIVE_ADMIN_PASSWORD=${R2DRIVE_ADMIN_PASSWORD:-admin123}
```

Start the service:

```bash
docker compose up -d
```

---

## Configuration Guide

### Config Precedence

`r2drive` resolves configuration files in the following order:

1. Explicit `--config <PATH>` CLI argument.
2. `R2DRIVE_CONFIG` environment variable.
3. User directory: `~/.config/r2drive/config.yaml`.
4. System directory: `/etc/r2drive/config.yaml`.
5. Local current working directory: `./config.yaml`.

### Configuration File (`config.yaml`)

```yaml
# ==============================================================================
# r2drive Configuration
# ==============================================================================

# Server configuration
server:
  # Host address to bind to (0.0.0.0 for containerized / public interfaces)
  host: "0.0.0.0"
  # Port number for HTTP / REST API and WebConsole
  port: 8080
  # Admin password for WebConsole login and protected endpoints
  admin_password: "${R2DRIVE_ADMIN_PASSWORD:-change-me-in-production}"
  # JWT signing secret (leave unset to generate an ephemeral key on startup)
  jwt_secret: "${JWT_SECRET:-custom-random-signing-secret}"
  # Session validity in hours
  session_ttl_hours: 72
  # Headless mode: disable static WebConsole hosting when running API-only
  headless: false

# Metadata storage database
database:
  # SQLite connection URI (PostgreSQL schema supported for expansion)
  url: "sqlite:///data/r2drive.db?mode=rwc"

# Metadata cache synchronization
sync:
  # Default cache time-to-live in seconds before re-validating against R2
  default_ttl_seconds: 60

# Default bucket profile to use when omitted from CLI commands
default_profile: "primary"

# Bucket profiles
profiles:
  primary:
    account_id: "${R2_ACCOUNT_ID}"
    access_key_id: "${R2_ACCESS_KEY_ID}"
    secret_access_key: "${R2_SECRET_ACCESS_KEY}"
    bucket_name: "${R2_BUCKET_NAME}"
    # Optional public CDN / custom domain URL for downloads
    public_url: "https://pub.example.com"

  backup:
    account_id: "${R2_BACKUP_ACCOUNT_ID:-${R2_ACCOUNT_ID}}"
    access_key_id: "${R2_BACKUP_ACCESS_KEY_ID:-${R2_ACCESS_KEY_ID}}"
    secret_access_key: "${R2_BACKUP_SECRET_ACCESS_KEY:-${R2_SECRET_ACCESS_KEY}}"
    bucket_name: "my-cold-backups"
```

### Environment Variable Substitution

`r2drive` natively parses and interpolates environment variables within `config.yaml`:

- **Required variable**: `${VAR_NAME}` — errors on launch if `VAR_NAME` is unset.
- **Default fallback**: `${VAR_NAME:-default_value}` — uses `default_value` if `VAR_NAME` is unset.
- **Escaped literal**: `$${LITERAL_NAME}` — renders as `${LITERAL_NAME}` without substitution.
- **Comments**: Comment lines (`#`) containing variable expressions are left untouched.

---

## CLI Usage Guide

`r2drive` functions as an expressive command-line tool for managing Cloudflare R2 buckets directly.

### Global Options

```
Options:
  -c, --config <PATH>    Path to config.yaml file
  -p, --profile <NAME>   Bucket profile name (falls back to default_profile)
  -h, --help             Print help
  -V, --version          Print version
```

### Commands & Examples

#### 1. List Objects (`r2drive ls`)

List objects and directory prefixes within the active bucket:

```bash
# List top-level objects and folders in default profile
r2drive ls

# List inside a specific folder prefix
r2drive ls documents/2026/

# List all objects recursively
r2drive ls -r

# List objects as structured JSON (for script pipelines)
r2drive ls documents/ --json

# Target a specific bucket profile
r2drive -p backup ls
```

#### 2. Upload Files (`r2drive upload`)

Upload a local file directly to Cloudflare R2:

```bash
# Upload a file preserving its local filename
r2drive upload ./release-v1.0.tar.gz

# Upload a file to a custom remote key path
r2drive upload ./photo.jpg images/profile.jpg

# Upload using a non-default profile
r2drive -p backup upload ./backup.tar.gz archives/daily/backup.tar.gz
```

*Note: For files larger than 5 MB, `r2drive upload` automatically utilizes multipart chunking with progress reporting and local snapshot caching for resume capability.*

#### 3. Download Files (`r2drive download`)

Download an object from Cloudflare R2 to a local path:

```bash
# Download remote object to local destination
r2drive download images/profile.jpg ./downloaded_profile.jpg
```

#### 4. Stream Object to Stdout (`r2drive cat`)

Stream the remote object's contents directly to standard output:

```bash
# Output text file to terminal
r2drive cat config/app.json

# Pipe binary content to another tool
r2drive cat data/records.csv.gz | gunzip | head -n 20
```

#### 5. Remove Object (`r2drive rm`)

Delete an object from Cloudflare R2:

```bash
# Delete object
r2drive rm images/old-banner.png
```

#### 6. Start StorageNode & WebConsole (`r2drive serve`)

Start the Axum HTTP REST API server and embedded WebConsole:

```bash
# Start server on configured port (default 8080)
r2drive serve

# Override port at runtime
r2drive serve -P 9090

# Run in headless mode (API only, WebConsole disabled)
r2drive serve --headless
```

---

## WebConsole Guide

The embedded WebConsole is a reactive single-page application built with **Svelte 5 Runes**, **Tailwind CSS**, and **Lucide-Svelte**.

### Core Capabilities

- **Admin Authentication**: Secure login gate using the configured `admin_password`. Issues an HttpOnly, SameSite session cookie with automated session verification.
- **Bucket Switching**: Instant navigation across all configured `BucketProfile` instances via a profile dropdown.
- **File Explorer**:
  - Breadcrumb navigation for directory exploration.
  - Search filtering by object key and filename.
  - Formatted file sizes, MIME type identification, and last modified timestamps.
  - Copy public URL action for buckets configured with `public_url`.
  - Delete action with confirmation modal.
- **Drag & Drop Upload Zone**:
  - Drag files anywhere over the explorer or click to browse.
  - Dynamic chunk sizing: Automatically calculates part sizes from 5 MB up to 100 MB, staying safely within R2's 10,000 part limit for files up to multi-terabyte scale.
  - Concurrency control: Parallel worker pool uploading up to 4 parts concurrently.
  - Direct Presigned Transfers: Browser streams chunks directly to Cloudflare R2 without routing payload data through StorageNode.
- **Resumable Session Engine**:
  - Transfer states and completed ETags persist in browser `IndexedDB`.
  - Pause, cancel, and resume active uploads.
  - Interrupted or dropped uploads prompt for instant resumption upon re-selecting or dropping the file.

---

## REST API Reference

The StorageNode exposes an Axum-powered REST API:

| Method | Endpoint | Description | Auth Required |
|---|---|---|---|
| `POST` | `/api/auth/login` | Authenticate with admin password; sets session cookie | No |
| `POST` | `/api/auth/logout` | Invalidate current session and clear cookie | Yes |
| `GET` | `/api/auth/me` | Inspect current session identity and validity | Yes |
| `GET` | `/api/buckets` | List all configured bucket profiles | Yes |
| `GET` | `/api/buckets/{profile}/objects` | List cached objects with prefix and delimiter filtering | Yes |
| `DELETE` | `/api/buckets/{profile}/objects` | Delete an object from the bucket and purge cache | Yes |
| `POST` | `/api/buckets/{profile}/upload/init` | Initiate single or multipart upload session | Yes |
| `POST` | `/api/buckets/{profile}/upload/resume` | Retrieve completed part ETags for active multipart session | Yes |
| `POST` | `/api/buckets/{profile}/upload/complete` | Finalize multipart upload on Cloudflare R2 | Yes |
| `POST` | `/api/buckets/{profile}/upload/abort` | Abort multipart session and clean up remote parts | Yes |
| `GET` | `/api/buckets/{profile}/download` | Generate download URL (public domain or presigned R2 URL) | Yes |

---

## Development & Testing

### Prerequisites

- **Rust**: 1.94+ (`cargo`, `rustc`)
- **Node.js**: 22+
- **pnpm**: 9+ (`corepack enable`)
- **SQLite**: 3.x (embedded)

### Build & Run Locally

1. **Build WebConsole**:
   ```bash
   cd web
   pnpm install
   pnpm build
   cd ..
   ```

2. **Run StorageNode**:
   ```bash
   cargo run -- serve
   ```

3. **Frontend Development Server** (with Vite hot-reloading):
   ```bash
   cd web
   pnpm dev
   ```
   Vite proxies `/api` requests to `http://localhost:8080`.

### Running Tests

- **Backend Unit & Integration Tests**:
  ```bash
  cargo test
  ```
  Runs 104 comprehensive tests spanning CLI subcommands, SQLite metadata store operations, multipart chunk math, authentication, and HTTP endpoints.

- **Frontend Tests & Type Checking**:
  ```bash
  pnpm --dir web test    # Vitest unit & store tests (71 tests)
  pnpm --dir web check   # Svelte-check TypeScript diagnostics
  ```

- **Clippy & Code Style**:
  ```bash
  cargo clippy --all-targets -- -D warnings
  ```

---

## License

Dual-licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
