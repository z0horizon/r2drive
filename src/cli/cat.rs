use crate::error::AppError;

/// Execute the `cat` subcommand by streaming object bytes to stdout.
pub async fn execute(bucket: &r2kit::Bucket, remote_key: &str) -> Result<(), AppError> {
    let downloaded = bucket
        .get(remote_key)
        .await
        .map_err(crate::r2::map_r2_error)?;

    let mut reader = downloaded.into_body().into_async_read();
    let mut stdout = tokio::io::stdout();
    tokio::io::copy(&mut reader, &mut stdout).await?;

    Ok(())
}
