use crate::error::AppError;
use tokio::io::AsyncWriteExt;

/// Execute the `cat` subcommand.
pub async fn execute(bucket: &r2kit::Bucket, remote_key: &str) -> Result<(), AppError> {
    let object_bytes = bucket
        .get_bytes(remote_key)
        .await
        .map_err(crate::r2::map_r2_error)?;

    let mut stdout = tokio::io::stdout();
    stdout.write_all(&object_bytes.bytes).await?;
    stdout.flush().await?;

    Ok(())
}
