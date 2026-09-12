use crate::error::AppError;

pub mod client;
pub mod transfer;

pub use client::R2Manager;
pub use transfer::{
    ChunkRange, CompletedPartReceipt, DEFAULT_PART_SIZE, MAX_PARTS, MIN_PART_SIZE, PresignedPart,
    PresignedUploadPlan, ResumedUploadPlan, abort_multipart_upload, calculate_chunk_ranges,
    calculate_part_count, complete_multipart_upload, generate_download_url, init_presigned_upload,
    init_presigned_upload_with_content_type, init_single_presigned_upload,
    init_single_presigned_upload_with_content_type, resume_multipart_upload,
};

/// Maps an `r2kit::Error` to domain `AppError` respecting HTTP semantic status codes:
/// - Validation / InvalidInput -> BadRequest
/// - NotFound -> NotFound
/// - Remote Authentication -> Auth
/// - Config -> Config
/// - Other -> R2 (Bad Gateway)
pub fn map_r2_error(err: r2kit::Error) -> AppError {
    err.into()
}
