use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DbObject {
    pub id: Option<i64>,
    pub bucket_profile: String,
    pub object_key: String,
    pub parent_prefix: String,
    pub is_directory: bool,
    pub size_bytes: i64,
    pub etag: Option<String>,
    pub content_type: Option<String>,
    pub last_modified: DateTime<Utc>,
    pub synced_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefixSyncStatus {
    pub bucket_profile: String,
    pub prefix: String,
    pub last_synced_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub token_hash: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultipartSessionRecord {
    pub upload_id: String,
    pub bucket_profile: String,
    pub object_key: String,
    pub file_size: i64,
    pub part_size: i64,
    pub total_parts: i64,
    pub created_at: DateTime<Utc>,
    pub last_activity_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BucketRecord {
    pub id: String,
    pub profile_name: String,
    pub bucket_name: String,
    pub account_id: String,
    pub created_at: DateTime<Utc>,
}
