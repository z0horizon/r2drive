use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::models::{BucketRecord, DbObject, MultipartSessionRecord, PrefixSyncStatus, Session};
use crate::error::AppError;

#[async_trait]
pub trait MetadataRepo: Send + Sync {
    async fn list_objects_by_prefix(
        &self,
        bucket_profile: &str,
        prefix: &str,
    ) -> Result<Vec<DbObject>, AppError>;

    async fn upsert_objects(
        &self,
        bucket_profile: &str,
        objects: &[DbObject],
    ) -> Result<(), AppError>;

    async fn delete_object(&self, bucket_profile: &str, object_key: &str) -> Result<(), AppError>;

    async fn get_prefix_sync_status(
        &self,
        bucket_profile: &str,
        prefix: &str,
    ) -> Result<Option<PrefixSyncStatus>, AppError>;

    async fn update_prefix_sync_status(&self, status: &PrefixSyncStatus) -> Result<(), AppError>;

    async fn get_session(&self, token_hash: &str) -> Result<Option<Session>, AppError>;

    async fn save_session(&self, session: &Session) -> Result<(), AppError>;

    async fn delete_session(&self, token_hash: &str) -> Result<(), AppError>;

    async fn get_multipart_session(
        &self,
        upload_id: &str,
    ) -> Result<Option<MultipartSessionRecord>, AppError>;

    async fn save_multipart_session(
        &self,
        session: &MultipartSessionRecord,
    ) -> Result<(), AppError>;

    async fn delete_multipart_session(&self, upload_id: &str) -> Result<(), AppError>;

    async fn list_stale_multipart_sessions(
        &self,
        older_than: DateTime<Utc>,
    ) -> Result<Vec<MultipartSessionRecord>, AppError>;

    async fn get_bucket(&self, _profile_name: &str) -> Result<Option<BucketRecord>, AppError> {
        Ok(None)
    }

    async fn save_bucket(&self, _bucket: &BucketRecord) -> Result<(), AppError> {
        Ok(())
    }
}
