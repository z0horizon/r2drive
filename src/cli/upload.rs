use crate::error::AppError;
use serde::{Deserialize, Serialize};
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
        PathBuf::from(home).join(".cache").join("r2drive").join("uploads")
    } else {
        PathBuf::from(".cache").join("r2drive").join("uploads")
    }
}

/// Generates a sanitized snapshot cache file path for a given bucket and object key.
pub fn snapshot_path_for(bucket_name: &str, key: &str) -> PathBuf {
    let sanitized_key = key.replace(['/', '\\', ' ', ':', '.'], "_");
    let filename = format!("{}_{}.json", bucket_name, sanitized_key);
    get_cache_dir().join(filename)
}

/// Saves a snapshot to disk in `~/.cache/r2drive/uploads/<hash_or_name>.json`.
pub async fn save_snapshot(snapshot: &r2kit::MultipartSessionSnapshot) -> Result<PathBuf, AppError> {
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

    let builder = match resumed_builder {
        Some(b) => b.abort_on_error(false),
        None => bucket
            .managed_multipart(&resolved_key)
            .map_err(crate::r2::map_r2_error)?
            .abort_on_error(false),
    };

    match builder.upload_file(local_path).await {
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
            if let Some(snapshot) = err.snapshot()
                && let Ok(path) = save_snapshot(snapshot).await
            {
                eprintln!(
                    "Upload interrupted. Saved resume snapshot to {}",
                    path.display()
                );
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
        assert_eq!(filename, "my-bucket_folder_sub_my_file_txt.json");
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
