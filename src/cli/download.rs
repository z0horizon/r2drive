use crate::error::AppError;
use std::path::Path;

/// Execute the `download` subcommand.
pub async fn execute(
    bucket: &r2kit::Bucket,
    remote_key: &str,
    local_path: &Path,
) -> Result<(), AppError> {
    // If local_path is an existing directory, append the filename component of remote_key
    let target_path = if local_path.is_dir() {
        let file_name = Path::new(remote_key)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(remote_key);
        local_path.join(file_name)
    } else {
        local_path.to_path_buf()
    };

    if let Some(parent) = target_path.parent()
        && !parent.as_os_str().is_empty()
    {
        tokio::fs::create_dir_all(parent).await?;
    }

    let metadata = bucket
        .download_file(remote_key, &target_path)
        .await
        .map_err(crate::r2::map_r2_error)?;

    println!(
        "Downloaded {} to {} ({} bytes)",
        remote_key,
        target_path.display(),
        metadata.size()
    );

    Ok(())
}
