use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

/// Serializable data representation of a multipart session snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SnapshotData {
    pub bucket: String,
    pub key: String,
    pub upload_id: String,
    pub file_size: u64,
    pub part_size: u64,
}

/// Resolves the cache directory path for upload session snapshots:
/// `~/.cache/r2drive/uploads/`
pub fn get_cache_dir() -> PathBuf {
    if let Some(home) = std::env::var("HOME").ok().filter(|h| !h.trim().is_empty()) {
        PathBuf::from(home)
            .join(".cache")
            .join("r2drive")
            .join("uploads")
    } else {
        PathBuf::from(".cache").join("r2drive").join("uploads")
    }
}

/// Generates a sanitized and bounded snapshot cache file path for a given bucket and object key.
/// Bounded to ensure it never exceeds filesystem limits (<= 255 bytes).
pub fn snapshot_path_for(bucket_name: &str, key: &str) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    bucket_name.hash(&mut hasher);
    key.hash(&mut hasher);
    let hash = hasher.finish();

    // Sanitize and limit prefix to at most 60 chars to avoid ENAMETOOLONG
    let sanitized_key: String = key
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(60)
        .collect();
    let sanitized_bucket: String = bucket_name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(30)
        .collect();

    let filename = format!("{}_{}_{:016x}.json", sanitized_bucket, sanitized_key, hash);
    get_cache_dir().join(filename)
}

/// Saves a snapshot to disk in `~/.cache/r2drive/uploads/<hash_or_name>.json`.
pub async fn save_snapshot(
    snapshot: &r2kit::MultipartSessionSnapshot,
) -> Result<PathBuf, AppError> {
    let dir = get_cache_dir();
    tokio::fs::create_dir_all(&dir).await?;

    let snap_file = snapshot_path_for(snapshot.bucket(), snapshot.key());
    let data = SnapshotData {
        bucket: snapshot.bucket().to_string(),
        key: snapshot.key().to_string(),
        upload_id: snapshot.expose_upload_id().to_string(),
        file_size: snapshot.file_size(),
        part_size: snapshot.part_size(),
    };

    let json = serde_json::to_string_pretty(&data)
        .map_err(|e| AppError::Config(format!("Failed to serialize upload snapshot: {e}")))?;

    tokio::fs::write(&snap_file, json).await?;
    Ok(snap_file)
}

async fn try_resume_snapshot(
    bucket: &r2kit::Bucket,
    snap_file: &Path,
    key: &str,
    file_size: u64,
) -> Option<r2kit::ManagedMultipartBuilder> {
    if !snap_file.exists() {
        return None;
    }
    let content = tokio::fs::read_to_string(snap_file).await.ok()?;
    let data = serde_json::from_str::<SnapshotData>(&content).ok()?;
    if data.bucket != bucket.name() || data.key != key || data.file_size != file_size {
        return None;
    }
    let snapshot = r2kit::MultipartSessionSnapshot::restore(
        &data.bucket,
        &data.key,
        &data.upload_id,
        data.file_size,
        data.part_size,
    )
    .ok()?;
    bucket.resume_managed_multipart(snapshot).ok()
}

/// Execute the `upload` subcommand.
pub async fn execute(
    bucket: &r2kit::Bucket,
    local_path: &Path,
    remote_key: Option<&str>,
) -> Result<(), AppError> {
    if !local_path.exists() {
        return Err(AppError::BadRequest(format!(
            "Local file not found: {}",
            local_path.display()
        )));
    }
    if !local_path.is_file() {
        return Err(AppError::BadRequest(format!(
            "Path is not a regular file: {}",
            local_path.display()
        )));
    }

    let file_metadata = tokio::fs::metadata(local_path).await?;
    let file_size = file_metadata.len();

    let resolved_key = match remote_key {
        Some(k) if !k.trim().is_empty() => k.to_string(),
        _ => local_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| {
                AppError::BadRequest("Could not derive remote key from local file path".to_string())
            })?
            .to_string(),
    };

    // Edge case: 0-byte file cannot be uploaded via multipart plan
    if file_size == 0 {
        bucket
            .put_bytes(&resolved_key, Vec::new())
            .await
            .map_err(crate::r2::map_r2_error)?;
        println!(
            "Uploaded {} to {} (0 bytes)",
            local_path.display(),
            resolved_key
        );
        return Ok(());
    }

    let snap_file = snapshot_path_for(bucket.name(), &resolved_key);

    let resumed_builder = try_resume_snapshot(bucket, &snap_file, &resolved_key, file_size).await;
    let was_resuming = resumed_builder.is_some();

    // Setup cancellation signal hooked up to Ctrl+C
    let cancellation = r2kit::ManagedUploadCancellation::new();
    let cancel_handle = cancellation.clone();
    let ctrl_c_task = tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            eprintln!(
                "\nReceived interruption signal (Ctrl+C). Aborting transfer and preserving snapshot..."
            );
            cancel_handle.cancel();
        }
    });

    let mut builder = match resumed_builder {
        Some(b) => b.abort_on_error(false),
        None => bucket
            .managed_multipart(&resolved_key)
            .map_err(crate::r2::map_r2_error)?
            .abort_on_error(false),
    };

    builder = builder.cancellation_token(cancellation);

    let upload_result = builder.upload_file(local_path).await;
    ctrl_c_task.abort();

    match upload_result {
        Ok(result) => {
            // Cleanup persisted snapshot on completion
            let _ = tokio::fs::remove_file(&snap_file).await;
            println!(
                "Uploaded {} to {} ({} bytes)",
                local_path.display(),
                resolved_key,
                result.file_size()
            );
            Ok(())
        }
        Err(err) => {
            if let Some(snapshot) = err.snapshot() {
                if let Ok(path) = save_snapshot(snapshot).await {
                    eprintln!(
                        "Upload interrupted. Saved resume snapshot to {}",
                        path.display()
                    );
                }
            } else if was_resuming {
                // If resuming from an existing snapshot and upload permanently failed without a snapshot,
                // purge the stale/corrupted snapshot file so subsequent runs don't get stuck.
                let _ = tokio::fs::remove_file(&snap_file).await;
            }

            Err(crate::r2::map_r2_error(err.error().clone()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_path_sanitization() {
        let path = snapshot_path_for("my-bucket", "folder/sub/my file.txt");
        let filename = path.file_name().unwrap().to_str().unwrap();
        assert!(filename.starts_with("my-bucket_folder_sub_my_file_txt_"));
        assert!(filename.ends_with(".json"));
        assert!(filename.len() <= 255);
    }

    #[test]
    fn test_snapshot_path_very_long_key() {
        let long_key = "a".repeat(500);
        let path = snapshot_path_for("my-bucket", &long_key);
        let filename = path.file_name().unwrap().to_str().unwrap();
        // Check that filename is well under the 255 character limit
        assert!(filename.len() <= 120);
        assert!(filename.ends_with(".json"));
    }

    #[test]
    fn test_snapshot_data_serde() {
        let data = SnapshotData {
            bucket: "test-bucket".to_string(),
            key: "test-key".to_string(),
            upload_id: "up12345".to_string(),
            file_size: 10485760,
            part_size: 5242880,
        };

        let json = serde_json::to_string(&data).unwrap();
        let decoded: SnapshotData = serde_json::from_str(&json).unwrap();
        assert_eq!(data, decoded);
    }
}
