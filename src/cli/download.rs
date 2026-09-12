use crate::error::AppError;
use std::path::Path;

/// Execute the `download` subcommand.
pub async fn execute(
    bucket: &r2kit::Bucket,
    remote_key: &str,
    local_path: &Path,
) -> Result<(), AppError> {
    if let Some(parent) = local_path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        tokio::fs::create_dir_all(parent).await?;
    }

    let metadata = bucket
        .download_file(remote_key, local_path)
        .await
        .map_err(crate::r2::map_r2_error)?;

    println!(
        "Downloaded {} to {} ({} bytes)",
        remote_key,
        local_path.display(),
        metadata.size()
    );

    Ok(())
}
