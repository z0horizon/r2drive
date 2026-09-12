use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::Row;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use std::str::FromStr;

use super::models::{BucketRecord, DbObject, MultipartSessionRecord, PrefixSyncStatus, Session};
use super::repo::MetadataRepo;
use crate::error::AppError;

fn parse_datetime(s: &str) -> Result<DateTime<Utc>, AppError> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.fZ") {
        return Ok(naive.and_utc());
    }
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Ok(naive.and_utc());
    }
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f") {
        return Ok(naive.and_utc());
    }
    Err(AppError::Config(format!(
        "Failed to parse datetime '{s}' as RFC3339 or SQLite timestamp"
    )))
}

#[derive(Clone, Debug)]
pub struct SqliteMetadataRepo {
    pool: sqlx::SqlitePool,
}

impl SqliteMetadataRepo {
    pub async fn connect(url: &str) -> Result<Self, AppError> {
        let is_memory = url.contains(":memory:");
        let options = SqliteConnectOptions::from_str(url)?.create_if_missing(true);

        let options = if is_memory {
            options
        } else {
            options
                .journal_mode(SqliteJournalMode::Wal)
                .busy_timeout(std::time::Duration::from_secs(5))
        };

        let pool = SqlitePoolOptions::new()
            .max_connections(if is_memory { 1 } else { 16 })
            .connect_with(options)
            .await?;

        sqlx::migrate!("./migrations/sqlite").run(&pool).await?;

        Ok(Self { pool })
    }

    pub fn pool(&self) -> &sqlx::SqlitePool {
        &self.pool
    }
}

#[async_trait]
impl MetadataRepo for SqliteMetadataRepo {
    async fn list_objects_by_prefix(
        &self,
        bucket_profile: &str,
        prefix: &str,
    ) -> Result<Vec<DbObject>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT id, bucket_profile, object_key, parent_prefix, is_directory, size_bytes, etag, content_type, last_modified, synced_at
            FROM objects
            WHERE bucket_profile = ? AND parent_prefix = ?
            ORDER BY is_directory DESC, object_key ASC
            "#,
        )
        .bind(bucket_profile)
        .bind(prefix)
        .fetch_all(&self.pool)
        .await?;

        let mut result = Vec::with_capacity(rows.len());
        for row in rows {
            let is_dir: i64 = row.try_get("is_directory").or_else(|_| {
                row.try_get::<bool, _>("is_directory")
                    .map(|b| if b { 1 } else { 0 })
            })?;
            let last_modified_raw: String = row.try_get("last_modified")?;
            let synced_at_raw: String = row.try_get("synced_at")?;

            result.push(DbObject {
                id: row.try_get("id").ok(),
                bucket_profile: row.try_get("bucket_profile")?,
                object_key: row.try_get("object_key")?,
                parent_prefix: row.try_get("parent_prefix")?,
                is_directory: is_dir != 0,
                size_bytes: row.try_get("size_bytes")?,
                etag: row.try_get("etag").ok(),
                content_type: row.try_get("content_type").ok(),
                last_modified: parse_datetime(&last_modified_raw)?,
                synced_at: parse_datetime(&synced_at_raw)?,
            });
        }

        Ok(result)
    }

    async fn upsert_objects(
        &self,
        bucket_profile: &str,
        objects: &[DbObject],
    ) -> Result<(), AppError> {
        if objects.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await?;
        for obj in objects {
            sqlx::query(
                r#"
                INSERT INTO objects (
                    bucket_profile, object_key, parent_prefix, is_directory, size_bytes, etag, content_type, last_modified, synced_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(bucket_profile, object_key) DO UPDATE SET
                    parent_prefix = excluded.parent_prefix,
                    is_directory = excluded.is_directory,
                    size_bytes = excluded.size_bytes,
                    etag = excluded.etag,
                    content_type = excluded.content_type,
                    last_modified = excluded.last_modified,
                    synced_at = excluded.synced_at
                "#,
            )
            .bind(bucket_profile)
            .bind(&obj.object_key)
            .bind(&obj.parent_prefix)
            .bind(if obj.is_directory { 1i64 } else { 0i64 })
            .bind(obj.size_bytes)
            .bind(&obj.etag)
            .bind(&obj.content_type)
            .bind(obj.last_modified.to_rfc3339())
            .bind(obj.synced_at.to_rfc3339())
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;

        Ok(())
    }

    async fn delete_object(&self, bucket_profile: &str, object_key: &str) -> Result<(), AppError> {
        sqlx::query(
            r#"
            DELETE FROM objects
            WHERE bucket_profile = ? AND object_key = ?
            "#,
        )
        .bind(bucket_profile)
        .bind(object_key)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_prefix_sync_status(
        &self,
        bucket_profile: &str,
        prefix: &str,
    ) -> Result<Option<PrefixSyncStatus>, AppError> {
        let row = sqlx::query(
            r#"
            SELECT bucket_profile, prefix, last_synced_at
            FROM prefix_sync_status
            WHERE bucket_profile = ? AND prefix = ?
            "#,
        )
        .bind(bucket_profile)
        .bind(prefix)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let last_synced_at_raw: String = row.try_get("last_synced_at")?;
                Ok(Some(PrefixSyncStatus {
                    bucket_profile: row.try_get("bucket_profile")?,
                    prefix: row.try_get("prefix")?,
                    last_synced_at: parse_datetime(&last_synced_at_raw)?,
                }))
            }
            None => Ok(None),
        }
    }

    async fn update_prefix_sync_status(&self, status: &PrefixSyncStatus) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO prefix_sync_status (bucket_profile, prefix, last_synced_at)
            VALUES (?, ?, ?)
            ON CONFLICT(bucket_profile, prefix) DO UPDATE SET
                last_synced_at = excluded.last_synced_at
            "#,
        )
        .bind(&status.bucket_profile)
        .bind(&status.prefix)
        .bind(status.last_synced_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_session(&self, token_hash: &str) -> Result<Option<Session>, AppError> {
        let row = sqlx::query(
            r#"
            SELECT token_hash, created_at, expires_at
            FROM sessions
            WHERE token_hash = ?
            "#,
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let created_at_raw: String = row.try_get("created_at")?;
                let expires_at_raw: String = row.try_get("expires_at")?;
                Ok(Some(Session {
                    token_hash: row.try_get("token_hash")?,
                    created_at: parse_datetime(&created_at_raw)?,
                    expires_at: parse_datetime(&expires_at_raw)?,
                }))
            }
            None => Ok(None),
        }
    }

    async fn save_session(&self, session: &Session) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO sessions (token_hash, created_at, expires_at)
            VALUES (?, ?, ?)
            ON CONFLICT(token_hash) DO UPDATE SET
                expires_at = excluded.expires_at
            "#,
        )
        .bind(&session.token_hash)
        .bind(session.created_at.to_rfc3339())
        .bind(session.expires_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn delete_session(&self, token_hash: &str) -> Result<(), AppError> {
        sqlx::query(
            r#"
            DELETE FROM sessions
            WHERE token_hash = ?
            "#,
        )
        .bind(token_hash)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_multipart_session(
        &self,
        upload_id: &str,
    ) -> Result<Option<MultipartSessionRecord>, AppError> {
        let row = sqlx::query(
            r#"
            SELECT upload_id, bucket_profile, object_key, file_size, part_size, total_parts, created_at, last_activity_at
            FROM multipart_sessions
            WHERE upload_id = ?
            "#,
        )
        .bind(upload_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let created_at_raw: String = row.try_get("created_at")?;
                let last_activity_at_raw: String = row.try_get("last_activity_at")?;
                Ok(Some(MultipartSessionRecord {
                    upload_id: row.try_get("upload_id")?,
                    bucket_profile: row.try_get("bucket_profile")?,
                    object_key: row.try_get("object_key")?,
                    file_size: row.try_get("file_size")?,
                    part_size: row.try_get("part_size")?,
                    total_parts: row.try_get("total_parts")?,
                    created_at: parse_datetime(&created_at_raw)?,
                    last_activity_at: parse_datetime(&last_activity_at_raw)?,
                }))
            }
            None => Ok(None),
        }
    }

    async fn save_multipart_session(
        &self,
        session: &MultipartSessionRecord,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO multipart_sessions (
                upload_id, bucket_profile, object_key, file_size, part_size, total_parts, created_at, last_activity_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(upload_id) DO UPDATE SET
                bucket_profile = excluded.bucket_profile,
                object_key = excluded.object_key,
                file_size = excluded.file_size,
                part_size = excluded.part_size,
                total_parts = excluded.total_parts,
                last_activity_at = excluded.last_activity_at
            "#,
        )
        .bind(&session.upload_id)
        .bind(&session.bucket_profile)
        .bind(&session.object_key)
        .bind(session.file_size)
        .bind(session.part_size)
        .bind(session.total_parts)
        .bind(session.created_at.to_rfc3339())
        .bind(session.last_activity_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn delete_multipart_session(&self, upload_id: &str) -> Result<(), AppError> {
        sqlx::query(
            r#"
            DELETE FROM multipart_sessions
            WHERE upload_id = ?
            "#,
        )
        .bind(upload_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn list_stale_multipart_sessions(
        &self,
        older_than: DateTime<Utc>,
    ) -> Result<Vec<MultipartSessionRecord>, AppError> {
        let older_than_str = older_than.to_rfc3339();
        let rows = sqlx::query(
            r#"
            SELECT upload_id, bucket_profile, object_key, file_size, part_size, total_parts, created_at, last_activity_at
            FROM multipart_sessions
            WHERE last_activity_at < ?
            ORDER BY last_activity_at ASC
            "#,
        )
        .bind(older_than_str)
        .fetch_all(&self.pool)
        .await?;

        let mut result = Vec::with_capacity(rows.len());
        for row in rows {
            let created_at_raw: String = row.try_get("created_at")?;
            let last_activity_at_raw: String = row.try_get("last_activity_at")?;
            result.push(MultipartSessionRecord {
                upload_id: row.try_get("upload_id")?,
                bucket_profile: row.try_get("bucket_profile")?,
                object_key: row.try_get("object_key")?,
                file_size: row.try_get("file_size")?,
                part_size: row.try_get("part_size")?,
                total_parts: row.try_get("total_parts")?,
                created_at: parse_datetime(&created_at_raw)?,
                last_activity_at: parse_datetime(&last_activity_at_raw)?,
            });
        }

        Ok(result)
    }

    async fn get_bucket(&self, profile_name: &str) -> Result<Option<BucketRecord>, AppError> {
        let row = sqlx::query(
            r#"
            SELECT id, profile_name, bucket_name, account_id, created_at
            FROM buckets
            WHERE profile_name = ?
            "#,
        )
        .bind(profile_name)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let created_at_raw: String = row.try_get("created_at")?;
                Ok(Some(BucketRecord {
                    id: row.try_get("id")?,
                    profile_name: row.try_get("profile_name")?,
                    bucket_name: row.try_get("bucket_name")?,
                    account_id: row.try_get("account_id")?,
                    created_at: parse_datetime(&created_at_raw)?,
                }))
            }
            None => Ok(None),
        }
    }

    async fn save_bucket(&self, bucket: &BucketRecord) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO buckets (id, profile_name, bucket_name, account_id, created_at)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                profile_name = excluded.profile_name,
                bucket_name = excluded.bucket_name,
                account_id = excluded.account_id
            "#,
        )
        .bind(&bucket.id)
        .bind(&bucket.profile_name)
        .bind(&bucket.bucket_name)
        .bind(&bucket.account_id)
        .bind(bucket.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::*;
    use chrono::Duration;

    #[tokio::test]
    async fn test_sqlite_object_crud() {
        let repo = SqliteMetadataRepo::connect("sqlite::memory:")
            .await
            .unwrap();
        let obj = DbObject {
            id: None,
            bucket_profile: "primary".into(),
            object_key: "docs/readme.txt".into(),
            parent_prefix: "docs/".into(),
            is_directory: false,
            size_bytes: 1024,
            etag: Some("\"etag123\"".into()),
            content_type: Some("text/plain".into()),
            last_modified: Utc::now(),
            synced_at: Utc::now(),
        };
        repo.upsert_objects("primary", std::slice::from_ref(&obj))
            .await
            .unwrap();

        // Query by parent prefix
        let listed = repo
            .list_objects_by_prefix("primary", "docs/")
            .await
            .unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].object_key, "docs/readme.txt");
        assert_eq!(listed[0].size_bytes, 1024);
        assert!(!listed[0].is_directory);

        // Update existing object (conflict resolution)
        let mut updated_obj = obj.clone();
        updated_obj.size_bytes = 2048;
        repo.upsert_objects("primary", &[updated_obj])
            .await
            .unwrap();
        let listed2 = repo
            .list_objects_by_prefix("primary", "docs/")
            .await
            .unwrap();
        assert_eq!(listed2.len(), 1);
        assert_eq!(listed2[0].size_bytes, 2048);

        // Delete object
        repo.delete_object("primary", "docs/readme.txt")
            .await
            .unwrap();
        let listed3 = repo
            .list_objects_by_prefix("primary", "docs/")
            .await
            .unwrap();
        assert_eq!(listed3.len(), 0);
    }

    #[tokio::test]
    async fn test_sqlite_prefix_sync_status() {
        let repo = SqliteMetadataRepo::connect("sqlite::memory:")
            .await
            .unwrap();

        // Initially empty
        let initial = repo
            .get_prefix_sync_status("primary", "photos/")
            .await
            .unwrap();
        assert!(initial.is_none());

        // Update status
        let now = Utc::now();
        let status = PrefixSyncStatus {
            bucket_profile: "primary".into(),
            prefix: "photos/".into(),
            last_synced_at: now,
        };
        repo.update_prefix_sync_status(&status).await.unwrap();

        let fetched = repo
            .get_prefix_sync_status("primary", "photos/")
            .await
            .unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.bucket_profile, "primary");
        assert_eq!(fetched.prefix, "photos/");
        assert_eq!(fetched.last_synced_at.timestamp(), now.timestamp());

        // Update again (upsert conflict test)
        let later = now + Duration::hours(1);
        let updated_status = PrefixSyncStatus {
            bucket_profile: "primary".into(),
            prefix: "photos/".into(),
            last_synced_at: later,
        };
        repo.update_prefix_sync_status(&updated_status)
            .await
            .unwrap();

        let refetched = repo
            .get_prefix_sync_status("primary", "photos/")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(refetched.last_synced_at.timestamp(), later.timestamp());
    }

    #[tokio::test]
    async fn test_sqlite_session_crud() {
        let repo = SqliteMetadataRepo::connect("sqlite::memory:")
            .await
            .unwrap();

        let session = Session {
            token_hash: "hash_abc_123".into(),
            created_at: Utc::now(),
            expires_at: Utc::now() + Duration::hours(24),
        };

        // None initially
        assert!(repo.get_session("hash_abc_123").await.unwrap().is_none());

        // Save session
        repo.save_session(&session).await.unwrap();
        let retrieved = repo.get_session("hash_abc_123").await.unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.token_hash, "hash_abc_123");
        assert_eq!(
            retrieved.expires_at.timestamp(),
            session.expires_at.timestamp()
        );

        // Delete session
        repo.delete_session("hash_abc_123").await.unwrap();
        assert!(repo.get_session("hash_abc_123").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_sqlite_multipart_session_and_stale() {
        let repo = SqliteMetadataRepo::connect("sqlite::memory:")
            .await
            .unwrap();

        let now = Utc::now();
        let stale_time = now - Duration::hours(3);
        let fresh_time = now - Duration::minutes(10);

        let stale_session = MultipartSessionRecord {
            upload_id: "upload_stale".into(),
            bucket_profile: "primary".into(),
            object_key: "large_stale.bin".into(),
            file_size: 50_000_000,
            part_size: 10_000_000,
            total_parts: 5,
            created_at: stale_time,
            last_activity_at: stale_time,
        };

        let fresh_session = MultipartSessionRecord {
            upload_id: "upload_fresh".into(),
            bucket_profile: "primary".into(),
            object_key: "large_fresh.bin".into(),
            file_size: 20_000_000,
            part_size: 10_000_000,
            total_parts: 2,
            created_at: fresh_time,
            last_activity_at: fresh_time,
        };

        repo.save_multipart_session(&stale_session).await.unwrap();
        repo.save_multipart_session(&fresh_session).await.unwrap();

        // Get single
        let retrieved = repo
            .get_multipart_session("upload_fresh")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retrieved.object_key, "large_fresh.bin");
        assert_eq!(retrieved.total_parts, 2);

        // List stale (older than 1 hour ago)
        let cutoff = now - Duration::hours(1);
        let stale_list = repo.list_stale_multipart_sessions(cutoff).await.unwrap();
        assert_eq!(stale_list.len(), 1);
        assert_eq!(stale_list[0].upload_id, "upload_stale");

        // Delete multipart session
        repo.delete_multipart_session("upload_stale").await.unwrap();
        let stale_list_after = repo.list_stale_multipart_sessions(cutoff).await.unwrap();
        assert_eq!(stale_list_after.len(), 0);
    }

    #[tokio::test]
    async fn test_sqlite_bucket_crud() {
        let repo = SqliteMetadataRepo::connect("sqlite::memory:")
            .await
            .unwrap();

        let bucket = BucketRecord {
            id: "bkt_1".into(),
            profile_name: "primary".into(),
            bucket_name: "my-r2-bucket".into(),
            account_id: "acc_123".into(),
            created_at: Utc::now(),
        };

        assert!(repo.get_bucket("primary").await.unwrap().is_none());

        repo.save_bucket(&bucket).await.unwrap();
        let fetched = repo.get_bucket("primary").await.unwrap().unwrap();
        assert_eq!(fetched.profile_name, "primary");
        assert_eq!(fetched.bucket_name, "my-r2-bucket");
        assert_eq!(fetched.account_id, "acc_123");
    }
}
