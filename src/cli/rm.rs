use crate::error::AppError;

/// Execute the `rm` subcommand.
pub async fn execute(bucket: &r2kit::Bucket, remote_key: &str) -> Result<(), AppError> {
    bucket
        .delete(remote_key)
        .await
        .map_err(crate::r2::map_r2_error)?;

    println!("Deleted {remote_key}");

    Ok(())
}
