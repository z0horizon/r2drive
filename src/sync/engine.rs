use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::db::models::{DbObject, PrefixSyncStatus};
use crate::error::AppError;
use crate::server::AppState;

/// A file object returned in prefix listings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectItem {
    pub key: String,
    pub name: String,
    pub size_bytes: i64,
    pub etag: Option<String>,
    pub content_type: Option<String>,
    pub last_modified: DateTime<Utc>,
}

/// Structured listing separating virtual directories and files for a prefix.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PrefixListing {
    pub prefix: String,
    pub directories: Vec<String>,
    pub objects: Vec<ObjectItem>,
    pub synced_at: DateTime<Utc>,
}

/// Synchronizes a bucket prefix with Cloudflare R2 if the cache is expired or refresh is requested.
pub async fn sync_prefix_if_needed(
    state: &AppState,
    profile: &str,
    prefix: &str,
    force_refresh: bool,
) -> Result<PrefixListing, AppError> {
    let sync_status = state.db.get_prefix_sync_status(profile, prefix).await?;
    let needs_refresh = force_refresh
        || match &sync_status {
            None => true,
            Some(status) => {
                let elapsed = Utc::now().signed_duration_since(status.last_synced_at);
                elapsed.num_seconds() >= state.config.sync.default_ttl_seconds as i64
            }
        };

    let now = Utc::now();

    if needs_refresh {
        let bucket = state.r2.get_bucket(profile)?;
        let mut continuation_token = None;
        let mut r2_objects = Vec::new();
        let mut seen_dirs = HashSet::new();

        loop {
            let mut builder = bucket.list().delimiter("/");
            if !prefix.is_empty() {
                builder = builder.prefix(prefix);
            }
            if let Some(token) = continuation_token {
                builder = builder.continuation_token(token);
            }

            let page = builder.send().await.map_err(crate::r2::map_r2_error)?;

            // Process virtual directories
            for dir_prefix in page.common_prefixes() {
                if seen_dirs.insert(dir_prefix.clone()) {
                    r2_objects.push(DbObject {
                        id: None,
                        bucket_profile: profile.to_string(),
                        object_key: dir_prefix.clone(),
                        parent_prefix: prefix.to_string(),
                        is_directory: true,
                        size_bytes: 0,
                        etag: None,
                        content_type: None,
                        last_modified: now,
                        synced_at: now,
                    });
                }
            }

            // Process concrete objects
            for obj in page.objects() {
                if obj.key() == prefix {
                    continue;
                }

                let last_modified = obj
                    .last_modified()
                    .map(DateTime::<Utc>::from)
                    .unwrap_or(now);

                let content_type = mime_guess::from_path(obj.key())
                    .first_raw()
                    .map(ToString::to_string);

                r2_objects.push(DbObject {
                    id: None,
                    bucket_profile: profile.to_string(),
                    object_key: obj.key().to_string(),
                    parent_prefix: prefix.to_string(),
                    is_directory: false,
                    size_bytes: obj.size() as i64,
                    etag: obj.etag().map(ToString::to_string),
                    content_type,
                    last_modified,
                    synced_at: now,
                });
            }

            if let Some(token) = page.next_continuation_token() {
                continuation_token = Some(token.to_string());
            } else {
                break;
            }
        }

        // Remove deleted items from metadata store for this prefix
        let existing = state.db.list_objects_by_prefix(profile, prefix).await?;
        let fresh_keys: HashSet<&str> = r2_objects.iter().map(|o| o.object_key.as_str()).collect();

        for item in existing {
            if !fresh_keys.contains(item.object_key.as_str()) {
                state.db.delete_object(profile, &item.object_key).await?;
            }
        }

        // Upsert all fetched items
        state.db.upsert_objects(profile, &r2_objects).await?;

        // Update prefix sync timestamp
        let status = PrefixSyncStatus {
            bucket_profile: profile.to_string(),
            prefix: prefix.to_string(),
            last_synced_at: now,
        };
        state.db.update_prefix_sync_status(&status).await?;
    }

    let synced_at = if needs_refresh {
        now
    } else {
        sync_status.map(|s| s.last_synced_at).unwrap_or(now)
    };

    let db_objects = state.db.list_objects_by_prefix(profile, prefix).await?;
    let mut directories = Vec::new();
    let mut objects = Vec::new();

    for obj in db_objects {
        if obj.is_directory {
            directories.push(obj.object_key);
        } else {
            let name = obj
                .object_key
                .strip_prefix(prefix)
                .unwrap_or(&obj.object_key)
                .to_string();

            objects.push(ObjectItem {
                key: obj.object_key,
                name,
                size_bytes: obj.size_bytes,
                etag: obj.etag,
                content_type: obj.content_type,
                last_modified: obj.last_modified,
            });
        }
    }

    Ok(PrefixListing {
        prefix: prefix.to_string(),
        directories,
        objects,
        synced_at,
    })
}
