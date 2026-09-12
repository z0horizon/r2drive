# Research: Multi-Database Architecture with SQLx (SQLite & PostgreSQL)

**Date**: 2026-09-12  
**Target**: `r2drive` Persistence Architecture (Phase 1 through Phase 3)  
**Author**: Antigravity Research Subagent (`browser`)

---

## 1. Executive Summary & Production Decision

To allow users to configure either:
```yaml
database:
  url: "sqlite:///data/r2drive.db?mode=rwc"
```
OR
```yaml
database:
  url: "postgres://user:password@localhost:5432/r2drive"
```
`r2drive` will implement the **Trait-Based Repository Pattern** (`MetadataRepo` trait) with separate native backends rather than `sqlx::Any`.

### Why `sqlx::Any` is Rejected:
1. **Placeholder & Query Syntax Clashes**: PostgreSQL requires `$1, $2`, while SQLite uses `?`. `sqlx::Any` rewrites queries at runtime, which breaks on operators like Postgres JSONB `?` or regexes.
2. **Lowest-Common-Denominator Types**: `sqlx::Any` forces dynamic type coercion, sacrificing native types like `chrono::DateTime<Utc>`, `bool`, and `i64`.
3. **DDL Incompatibilities in Migrations**: SQLite and PostgreSQL differ significantly in DDL (`AUTOINCREMENT` vs `GENERATED ALWAYS AS IDENTITY`, `TEXT` ISO8601 vs `TIMESTAMPTZ`, triggers). Unified migration files fail.
4. **Engine Tuning**: SQLite requires single-writer WAL mode and busy timeouts (`busy_timeout = 5000ms`), while PostgreSQL uses multi-connection pooling.

---

## 2. Recommended Repository Architecture

```
src/db/
├── mod.rs                  # init_database factory, URL detection (sqlite vs postgres)
├── repo.rs                 # MetadataRepo trait (async_trait)
├── models.rs               # Pure domain models (DbBucket, DbObject, DbSession, DbMultipartSession)
├── sqlite/
│   ├── mod.rs              # SqliteMetadataRepo implementing MetadataRepo
│   └── dto.rs              # SQLite-specific row mapping (TEXT timestamps, INTEGER booleans)
└── postgres/
    ├── mod.rs              # PostgresMetadataRepo implementing MetadataRepo
    └── dto.rs              # Postgres-specific row mapping (TIMESTAMPTZ, native bool, BIGINT)
```

### 2.1 The Repository Trait (`src/db/repo.rs`)
```rust
use async_trait::async_trait;
use crate::error::AppError;
use super::models::{DbBucket, DbObject, DbSession, DbMultipartSession};

#[async_trait]
pub trait MetadataRepo: Send + Sync {
    // Buckets
    async fn get_bucket_by_profile(&self, profile: &str) -> Result<Option<DbBucket>, AppError>;
    async fn upsert_bucket(&self, bucket: &DbBucket) -> Result<(), AppError>;

    // Objects & Prefix Sync
    async fn list_objects_by_prefix(&self, profile: &str, prefix: &str) -> Result<Vec<DbObject>, AppError>;
    async fn upsert_objects(&self, profile: &str, objects: &[DbObject]) -> Result<(), AppError>;
    async fn delete_object(&self, profile: &str, key: &str) -> Result<(), AppError>;
    async fn get_prefix_sync_status(&self, profile: &str, prefix: &str) -> Result<Option<chrono::DateTime<chrono::Utc>>, AppError>;
    async fn update_prefix_sync_status(&self, profile: &str, prefix: &str, synced_at: chrono::DateTime<chrono::Utc>) -> Result<(), AppError>;

    // Sessions & Multipart
    async fn get_session(&self, token_hash: &str) -> Result<Option<DbSession>, AppError>;
    async fn save_session(&self, session: &DbSession) -> Result<(), AppError>;
    async fn delete_session(&self, token_hash: &str) -> Result<(), AppError>;

    async fn get_multipart_session(&self, upload_id: &str) -> Result<Option<DbMultipartSession>, AppError>;
    async fn save_multipart_session(&self, session: &DbMultipartSession) -> Result<(), AppError>;
    async fn delete_multipart_session(&self, upload_id: &str) -> Result<(), AppError>;
    async fn list_stale_multipart_sessions(&self, threshold: chrono::DateTime<chrono::Utc>) -> Result<Vec<DbMultipartSession>, AppError>;
}
```

---

## 3. Database URL Detection & Factory (`src/db/mod.rs`)

```rust
use std::sync::Arc;
use url::Url;
use crate::error::AppError;
use self::repo::MetadataRepo;

pub enum DatabaseEngine {
    Sqlite,
    Postgres,
}

pub async fn create_metadata_store(db_url: &str) -> Result<Arc<dyn MetadataRepo>, AppError> {
    let parsed = Url::parse(db_url).map_err(|e| AppError::Config(format!("Invalid database URL: {e}")))?;
    
    match parsed.scheme() {
        "sqlite" => {
            let repo = sqlite::SqliteMetadataRepo::connect(db_url).await?;
            Ok(Arc::new(repo))
        }
        "postgres" | "postgresql" => {
            let repo = postgres::PostgresMetadataRepo::connect(db_url).await?;
            Ok(Arc::new(repo))
        }
        other => Err(AppError::Config(format!("Unsupported database scheme '{other}'. Supported: sqlite, postgres"))),
    }
}
```

---

## 4. Dual Migrations Strategy

```text
migrations/
├── sqlite/
│   └── 20260912000001_init.sql
└── postgres/
    └── 20260912000001_init.sql
```

Migrations are compiled into the binary with `sqlx::migrate!`:
- `sqlx::migrate!("./migrations/sqlite").run(&pool).await?;`
- `sqlx::migrate!("./migrations/postgres").run(&pool).await?;`
