use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const MIN_PART_SIZE: u64 = 5 * 1024 * 1024; // 5MB
pub const DEFAULT_PART_SIZE: u64 = 10 * 1024 * 1024; // 10MB
pub const MAX_PARTS: u16 = 10_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkRange {
    pub part_number: u16,
    pub start_byte: u64,
    pub end_byte: u64,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresignedPart {
    pub part_number: u16,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresignedUploadPlan {
    pub upload_id: String,
    pub part_size: u64,
    pub parts: Vec<PresignedPart>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletedPartReceipt {
    pub part_number: u16,
    pub etag: String,
}

impl From<CompletedPartReceipt> for (u16, String) {
    fn from(r: CompletedPartReceipt) -> Self {
        (r.part_number, r.etag)
    }
}

impl From<(u16, String)> for CompletedPartReceipt {
    fn from((part_number, etag): (u16, String)) -> Self {
        Self { part_number, etag }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResumedUploadPlan {
    pub upload_id: String,
    pub completed_parts: Vec<CompletedPartReceipt>,
    pub remaining_parts: Vec<PresignedPart>,
}

/// Calculate the number of parts needed to upload a file of `file_size` bytes
/// with `part_size` chunk size.
pub fn calculate_part_count(file_size: u64, part_size: u64) -> Result<u16, AppError> {
    if file_size == 0 {
        return Err(AppError::BadRequest(
            "File size must be greater than 0".to_string(),
        ));
    }
    if part_size < MIN_PART_SIZE {
        return Err(AppError::BadRequest(format!(
            "Part size ({part_size} bytes) is below minimum of {MIN_PART_SIZE} bytes (5MB)"
        )));
    }
    let part_count = file_size.div_ceil(part_size);
    if part_count > MAX_PARTS as u64 {
        return Err(AppError::BadRequest(format!(
            "Calculated part count ({part_count}) exceeds maximum allowed ({MAX_PARTS})"
        )));
    }
    Ok(part_count as u16)
}

/// Calculate byte ranges for each chunk of a file upload.
pub fn calculate_chunk_ranges(file_size: u64, part_size: u64) -> Result<Vec<ChunkRange>, AppError> {
    let part_count = calculate_part_count(file_size, part_size)?;
    let mut chunks = Vec::with_capacity(part_count as usize);

    for part_num in 1..=part_count {
        let start_byte = (part_num as u64 - 1) * part_size;
        let size = std::cmp::min(part_size, file_size - start_byte);
        let end_byte = start_byte + size - 1;
        chunks.push(ChunkRange {
            part_number: part_num,
            start_byte,
            end_byte,
            size,
        });
    }

    Ok(chunks)
}

use crate::r2::map_r2_error;

/// Initiates a presigned multipart upload on R2 and generates presigned URLs
/// for every part in the upload plan.
pub async fn init_presigned_upload(
    bucket: &r2kit::Bucket,
    key: &str,
    file_size: u64,
    part_size: u64,
    expires_in: Duration,
) -> Result<PresignedUploadPlan, AppError> {
    init_presigned_upload_with_content_type(bucket, key, file_size, part_size, expires_in, None)
        .await
}

/// Initiates a presigned multipart upload on R2 with an optional MIME content type.
pub async fn init_presigned_upload_with_content_type(
    bucket: &r2kit::Bucket,
    key: &str,
    file_size: u64,
    part_size: u64,
    expires_in: Duration,
    content_type: Option<&str>,
) -> Result<PresignedUploadPlan, AppError> {
    let mut builder = bucket
        .presigned_multipart(key)
        .map_err(map_r2_error)?
        .file_size(file_size)
        .part_size(part_size);

    if let Some(ct) = content_type {
        builder = builder.content_type(ct);
    }

    let session = builder.create().await.map_err(map_r2_error)?;

    let upload_id = session.upload_id().to_string();
    let part_count = session.part_count();
    let mut parts = Vec::with_capacity(part_count as usize);

    for part_num in 1..=part_count {
        let number = r2kit::PartNumber::try_from(part_num).map_err(map_r2_error)?;
        let presigned = session
            .presign_part(number, expires_in)
            .await
            .map_err(map_r2_error)?;

        parts.push(PresignedPart {
            part_number: part_num,
            url: presigned.into_url_string(),
        });
    }

    Ok(PresignedUploadPlan {
        upload_id,
        part_size,
        parts,
    })
}

/// Initiates a single-part presigned PUT upload URL for files smaller than multipart threshold.
pub async fn init_single_presigned_upload(
    bucket: &r2kit::Bucket,
    key: &str,
    file_size: u64,
    expires_in: Duration,
) -> Result<String, AppError> {
    init_single_presigned_upload_with_content_type(bucket, key, file_size, expires_in, None).await
}

/// Initiates a single-part presigned PUT upload URL with optional MIME content type.
pub async fn init_single_presigned_upload_with_content_type(
    bucket: &r2kit::Bucket,
    key: &str,
    file_size: u64,
    expires_in: Duration,
    content_type: Option<&str>,
) -> Result<String, AppError> {
    let mut options = r2kit::ObjectUploadOptions::default();
    if let Some(ct) = content_type {
        options = options.with_content_type(ct);
    }

    let presigned = bucket
        .presign_put_with_options(key, file_size, expires_in, options)
        .await
        .map_err(map_r2_error)?;

    Ok(presigned.into_url_string())
}

/// Completes a multipart upload on R2 by submitting all part numbers and ETags.
pub async fn complete_multipart_upload(
    bucket: &r2kit::Bucket,
    key: &str,
    upload_id: &str,
    file_size: u64,
    part_size: u64,
    parts: Vec<(u16, String)>,
) -> Result<String, AppError> {
    let snapshot = r2kit::MultipartSessionSnapshot::restore(
        bucket.name(),
        key,
        upload_id,
        file_size,
        part_size,
    )
    .map_err(map_r2_error)?;

    let session = bucket
        .resume_presigned_multipart(snapshot)
        .map_err(map_r2_error)?;

    let receipts: Vec<r2kit::MultipartPartReceipt> = parts
        .into_iter()
        .map(|(num, etag)| r2kit::MultipartPartReceipt::new(num, etag))
        .collect();

    let manifest = r2kit::CompletionManifest::try_from_receipts(receipts).map_err(map_r2_error)?;

    let completed = session.complete(manifest).await.map_err(map_r2_error)?;

    Ok(completed.etag().unwrap_or_default().to_string())
}

/// Resumes an interrupted multipart upload session by querying R2 for uploaded parts
/// and signing presigned PUT URLs for only the missing parts.
pub async fn resume_multipart_upload(
    bucket: &r2kit::Bucket,
    key: &str,
    upload_id: &str,
    file_size: u64,
    part_size: u64,
    expires_in: Duration,
) -> Result<ResumedUploadPlan, AppError> {
    let snapshot = r2kit::MultipartSessionSnapshot::restore(
        bucket.name(),
        key,
        upload_id,
        file_size,
        part_size,
    )
    .map_err(map_r2_error)?;

    let session = bucket
        .resume_presigned_multipart(snapshot)
        .map_err(map_r2_error)?;

    let reconciliation = session.reconcile().await.map_err(map_r2_error)?;

    let completed_parts: Vec<CompletedPartReceipt> = reconciliation
        .uploaded_parts()
        .map(|p| CompletedPartReceipt {
            part_number: p.part_number().get(),
            etag: p.etag().to_string(),
        })
        .collect();

    let mut remaining_parts = Vec::new();
    for missing in reconciliation.missing_parts() {
        let presigned = session
            .presign_part(missing, expires_in)
            .await
            .map_err(map_r2_error)?;

        remaining_parts.push(PresignedPart {
            part_number: missing.get(),
            url: presigned.into_url_string(),
        });
    }

    Ok(ResumedUploadPlan {
        upload_id: upload_id.to_string(),
        completed_parts,
        remaining_parts,
    })
}

/// Aborts an in-flight multipart upload session on R2 directly without needing snapshot restoration.
pub async fn abort_multipart_upload(
    bucket: &r2kit::Bucket,
    key: &str,
    upload_id: &str,
) -> Result<(), AppError> {
    bucket
        .abort_multipart_upload(key, upload_id)
        .await
        .map_err(map_r2_error)?;

    Ok(())
}

/// Generates a download URL for an object key. If `custom_public_url` is provided,
/// formats the public domain URL directly with proper percent encoding of path segments.
/// Otherwise, signs an authenticated presigned GET request on R2.
pub async fn generate_download_url(
    bucket: &r2kit::Bucket,
    key: &str,
    expires_in: Duration,
    custom_public_url: Option<&str>,
) -> Result<String, AppError> {
    if let Some(base) = custom_public_url.filter(|s| !s.trim().is_empty()) {
        let base_trimmed = base.trim();
        let base_with_scheme =
            if base_trimmed.starts_with("http://") || base_trimmed.starts_with("https://") {
                base_trimmed.to_string()
            } else {
                format!("https://{base_trimmed}")
            };

        let mut parsed = url::Url::parse(&base_with_scheme).map_err(|e| {
            AppError::BadRequest(format!("Invalid custom_public_url '{base}': {e}"))
        })?;

        let key_clean = key.trim_start_matches('/');
        if !key_clean.is_empty() {
            let mut segments = parsed.path_segments_mut().map_err(|_| {
                AppError::BadRequest(format!("Cannot format path segments for URL: '{base}'"))
            })?;
            segments.pop_if_empty();
            for segment in key_clean.split('/') {
                segments.push(segment);
            }
        }
        Ok(parsed.to_string())
    } else {
        let presigned = bucket
            .presign_get(key, expires_in)
            .await
            .map_err(map_r2_error)?;
        Ok(presigned.into_url_string())
    }
}

pub struct SyncBody<B> {
    inner: std::sync::Mutex<B>,
}

impl<B> SyncBody<B> {
    pub fn new(inner: B) -> Self {
        Self {
            inner: std::sync::Mutex::new(inner),
        }
    }
}

impl<B> http_body::Body for SyncBody<B>
where
    B: http_body::Body + Send + Unpin + 'static,
{
    type Data = B::Data;
    type Error = B::Error;

    fn poll_frame(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        let inner = self.inner.get_mut().unwrap_or_else(|e| e.into_inner());
        std::pin::Pin::new(inner).poll_frame(cx)
    }

    fn is_end_stream(&self) -> bool {
        self.inner
            .lock()
            .map(|b| b.is_end_stream())
            .unwrap_or(false)
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.lock().map(|b| b.size_hint()).unwrap_or_default()
    }
}

/// Validates and trims an object key for transfer operations.
pub fn clean_object_key(key: &str) -> Result<&str, AppError> {
    let clean = key.trim().trim_start_matches('/');
    if clean.is_empty() {
        return Err(AppError::BadRequest(
            "Object key cannot be empty".to_string(),
        ));
    }
    if clean.ends_with('/') {
        return Err(AppError::BadRequest(
            "Object key cannot end with a slash".to_string(),
        ));
    }
    Ok(clean)
}

/// Resolves the effective content type in precedence order:
/// 1. Query parameter `query_ct` (trimmed, non-empty)
/// 2. Request header `header_ct` (trimmed, non-empty)
/// 3. Inferred from object key extension via `mime_guess`
pub fn resolve_content_type(
    query_ct: Option<&str>,
    header_ct: Option<&str>,
    key: &str,
) -> Option<String> {
    query_ct
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            header_ct
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
        .or_else(|| {
            mime_guess::from_path(key)
                .first_raw()
                .map(ToString::to_string)
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyUploadResult {
    pub key: String,
    pub size_bytes: u64,
    pub etag: Option<String>,
    pub content_type: Option<String>,
}

pub struct ProxyTransferEngine<'a> {
    pub bucket: &'a r2kit::Bucket,
    pub profile: &'a str,
    pub db: Option<&'a (dyn crate::db::MetadataRepo + 'static)>,
}

impl<'a> ProxyTransferEngine<'a> {
    pub fn new(
        bucket: &'a r2kit::Bucket,
        profile: &'a str,
        db: Option<&'a (dyn crate::db::MetadataRepo + 'static)>,
    ) -> Self {
        Self {
            bucket,
            profile,
            db,
        }
    }

    pub async fn upload<B>(
        &self,
        key: &str,
        content_length: u64,
        content_type: Option<String>,
        body: B,
    ) -> Result<ProxyUploadResult, AppError>
    where
        B: http_body::Body<Data = axum::body::Bytes> + Send + Unpin + 'static,
        B::Data: Send,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let clean_key = clean_object_key(key)?;

        let mut options = r2kit::ObjectUploadOptions::default();
        if let Some(ref ct) = content_type {
            options = options.with_content_type(ct);
        }

        let byte_stream = aws_sdk_s3::primitives::ByteStream::from_body_1_x(SyncBody::new(body));

        let put_res = self
            .bucket
            .put_stream_with_options(clean_key, byte_stream, content_length, options)
            .await
            .map_err(map_r2_error)?;

        let etag = put_res.etag().map(String::from);

        if let Some(db) = self.db {
            let now = chrono::Utc::now();
            let parent_prefix = clean_key
                .rfind('/')
                .map_or(String::new(), |idx| clean_key[..=idx].to_string());

            let record = crate::db::models::DbObject {
                id: None,
                bucket_profile: self.profile.to_string(),
                object_key: clean_key.to_string(),
                parent_prefix,
                is_directory: false,
                size_bytes: content_length as i64,
                etag: etag.clone(),
                content_type: content_type.clone(),
                last_modified: now,
                synced_at: now,
            };

            if let Err(e) = db.upsert_objects(self.profile, &[record]).await {
                tracing::error!("Failed to record proxy uploaded object into metadata store: {e}");
            }
        }

        Ok(ProxyUploadResult {
            key: clean_key.to_string(),
            size_bytes: content_length,
            etag,
            content_type,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;

    fn test_bucket() -> r2kit::Bucket {
        let config = r2kit::R2Config::builder()
            .account_id("0123456789abcdef0123456789abcdef")
            .access_key_id("test-access-key")
            .secret_access_key("test-secret-key")
            .build()
            .unwrap();
        r2kit::R2Client::new(config).bucket("test-bucket").unwrap()
    }

    #[test]
    fn test_chunk_calculation_5mb_single_part() {
        let file_size = 5 * MB;
        let part_size = 5 * MB;
        let count = calculate_part_count(file_size, part_size).unwrap();
        assert_eq!(count, 1);

        let chunks = calculate_chunk_ranges(file_size, part_size).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(
            chunks[0],
            ChunkRange {
                part_number: 1,
                start_byte: 0,
                end_byte: 5 * MB - 1,
                size: 5 * MB,
            }
        );
    }

    #[test]
    fn test_chunk_calculation_50mb_five_parts() {
        let file_size = 50 * MB;
        let part_size = 10 * MB;
        let count = calculate_part_count(file_size, part_size).unwrap();
        assert_eq!(count, 5);

        let chunks = calculate_chunk_ranges(file_size, part_size).unwrap();
        assert_eq!(chunks.len(), 5);
        for (i, chunk) in chunks.iter().enumerate() {
            let part_num = (i + 1) as u16;
            let start = (i as u64) * 10 * MB;
            let end = start + 10 * MB - 1;
            assert_eq!(chunk.part_number, part_num);
            assert_eq!(chunk.start_byte, start);
            assert_eq!(chunk.end_byte, end);
            assert_eq!(chunk.size, 10 * MB);
        }
    }

    #[test]
    fn test_chunk_calculation_1gb_parts() {
        let file_size = GB; // 1,073,741,824 bytes
        let part_size = 10 * MB; // 10,485,760 bytes
        let count = calculate_part_count(file_size, part_size).unwrap();
        assert_eq!(count, 103);

        let chunks = calculate_chunk_ranges(file_size, part_size).unwrap();
        assert_eq!(chunks.len(), 103);

        // First 102 parts are 10MB
        for chunk in &chunks[..102] {
            assert_eq!(chunk.size, 10 * MB);
        }
        // Part 103 is remainder (4MB)
        let last = &chunks[102];
        assert_eq!(last.part_number, 103);
        assert_eq!(last.size, 4 * MB);
        assert_eq!(last.end_byte, GB - 1);

        // Verify total covered size
        let total_size: u64 = chunks.iter().map(|c| c.size).sum();
        assert_eq!(total_size, file_size);
    }

    #[test]
    fn test_chunk_calculation_zero_file_size_error() {
        let err = calculate_part_count(0, 10 * MB).unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));

        let err2 = calculate_chunk_ranges(0, 10 * MB).unwrap_err();
        assert!(matches!(err2, AppError::BadRequest(_)));
    }

    #[test]
    fn test_chunk_calculation_below_minimum_part_size_error() {
        let err = calculate_part_count(10 * MB, 4 * MB).unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));

        let err2 = calculate_chunk_ranges(10 * MB, 1024).unwrap_err();
        assert!(matches!(err2, AppError::BadRequest(_)));
    }

    #[test]
    fn test_chunk_calculation_exceeds_max_parts_error() {
        // 10,001 parts of 5MB = 50,005 MB
        let part_size = 5 * MB;
        let file_size = (MAX_PARTS as u64 + 1) * part_size;
        let err = calculate_part_count(file_size, part_size).unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[tokio::test]
    async fn test_generate_download_url_custom_public_url() {
        let bucket = test_bucket();
        let url = generate_download_url(
            &bucket,
            "images/photo.jpg",
            Duration::from_secs(3600),
            Some("https://cdn.example.com/"),
        )
        .await
        .unwrap();

        assert_eq!(url, "https://cdn.example.com/images/photo.jpg");

        // Leading slash in key
        let url2 = generate_download_url(
            &bucket,
            "/docs/manual.pdf",
            Duration::from_secs(3600),
            Some("https://cdn.example.com"),
        )
        .await
        .unwrap();

        assert_eq!(url2, "https://cdn.example.com/docs/manual.pdf");
    }

    #[tokio::test]
    async fn test_generate_download_url_presigned_r2() {
        let bucket = test_bucket();
        let url =
            generate_download_url(&bucket, "private/data.csv", Duration::from_secs(3600), None)
                .await
                .unwrap();

        assert!(url.contains("test-bucket"));
        assert!(url.contains("/private/data.csv"));
        assert!(url.contains("X-Amz-Signature="));
        assert!(url.contains("X-Amz-Expires=3600"));
    }

    #[tokio::test]
    async fn test_init_single_presigned_upload() {
        let bucket = test_bucket();
        let url = init_single_presigned_upload(
            &bucket,
            "uploads/avatar.png",
            1024 * 100, // 100KB
            Duration::from_secs(1800),
        )
        .await
        .unwrap();

        assert!(url.contains("test-bucket"));
        assert!(url.contains("/uploads/avatar.png"));
        assert!(url.contains("X-Amz-Signature="));
        assert!(url.contains("X-Amz-Expires=1800"));
    }

    #[tokio::test]
    async fn test_session_restoration_and_offline_presign_part() {
        let bucket = test_bucket();
        let file_size = 20 * MB;
        let part_size = 10 * MB;
        let snapshot = r2kit::MultipartSessionSnapshot::restore(
            bucket.name(),
            "video.mp4",
            "test-upload-session-123",
            file_size,
            part_size,
        )
        .unwrap();

        let session = bucket.resume_presigned_multipart(snapshot).unwrap();
        assert_eq!(session.part_count(), 2);

        // Sign part 1
        let part_1_num = r2kit::PartNumber::try_from(1).unwrap();
        let presigned_1 = session
            .presign_part(part_1_num, Duration::from_secs(3600))
            .await
            .unwrap();
        let url_1 = presigned_1.request().url().expose();
        assert!(url_1.contains("video.mp4"));
        assert!(url_1.contains("uploadId=test-upload-session-123"));
        assert!(url_1.contains("partNumber=1"));

        // Sign part 2
        let part_2_num = r2kit::PartNumber::try_from(2).unwrap();
        let presigned_2 = session
            .presign_part(part_2_num, Duration::from_secs(3600))
            .await
            .unwrap();
        let url_2 = presigned_2.request().url().expose();
        assert!(url_2.contains("video.mp4"));
        assert!(url_2.contains("uploadId=test-upload-session-123"));
        assert!(url_2.contains("partNumber=2"));
    }

    #[tokio::test]
    async fn test_complete_multipart_invalid_parts_returns_error() {
        let bucket = test_bucket();
        // Empty parts list is invalid
        let err = complete_multipart_upload(
            &bucket,
            "large.bin",
            "mock-upload-id",
            10 * MB,
            10 * MB,
            vec![],
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[tokio::test]
    async fn test_complete_multipart_duplicate_parts_returns_error() {
        let bucket = test_bucket();
        // Duplicate part numbers are invalid
        let err = complete_multipart_upload(
            &bucket,
            "large.bin",
            "mock-upload-id",
            20 * MB,
            10 * MB,
            vec![(1, "\"etag1\"".to_string()), (1, "\"etag2\"".to_string())],
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[tokio::test]
    async fn test_abort_multipart_invalid_upload_id() {
        let bucket = test_bucket();
        // Empty upload_id is invalid
        let err = abort_multipart_upload(&bucket, "large.bin", "")
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn test_completed_part_receipt_conversions() {
        let receipt = CompletedPartReceipt {
            part_number: 2,
            etag: "\"test-etag\"".to_string(),
        };
        let tuple: (u16, String) = receipt.clone().into();
        assert_eq!(tuple, (2, "\"test-etag\"".to_string()));

        let from_tuple: CompletedPartReceipt = tuple.into();
        assert_eq!(from_tuple, receipt);
    }

    #[tokio::test]
    async fn test_generate_download_url_custom_public_url_percent_encoding() {
        let bucket = test_bucket();

        // Spaces and special characters in key
        let url = generate_download_url(
            &bucket,
            "photos/summer vacation 2026/my beach & sun [1] #cool?.jpg",
            Duration::from_secs(3600),
            Some("https://cdn.example.com/assets"),
        )
        .await
        .unwrap();

        assert_eq!(
            url,
            "https://cdn.example.com/assets/photos/summer%20vacation%202026/my%20beach%20&%20sun%20[1]%20%23cool%3F.jpg"
        );

        // Host without scheme
        let url2 = generate_download_url(
            &bucket,
            "folder with space/file.txt",
            Duration::from_secs(3600),
            Some("cdn.example.com"),
        )
        .await
        .unwrap();

        assert_eq!(
            url2,
            "https://cdn.example.com/folder%20with%20space/file.txt"
        );
    }

    #[tokio::test]
    async fn test_malformed_content_type_rejected() {
        let bucket = test_bucket();

        // Single upload with invalid MIME
        let err_single = init_single_presigned_upload_with_content_type(
            &bucket,
            "test.txt",
            100,
            Duration::from_secs(3600),
            Some("not a valid mime format @@!!"),
        )
        .await
        .unwrap_err();

        assert!(matches!(err_single, AppError::BadRequest(_)));
        assert!(err_single.to_string().contains("content_type"));

        // Multipart upload with invalid MIME
        let err_multi = init_presigned_upload_with_content_type(
            &bucket,
            "large.bin",
            10 * MB,
            10 * MB,
            Duration::from_secs(3600),
            Some("invalid-mime"),
        )
        .await
        .unwrap_err();

        assert!(matches!(err_multi, AppError::BadRequest(_)));
        assert!(err_multi.to_string().contains("content_type"));
    }

    #[tokio::test]
    async fn test_valid_content_type_single_upload() {
        let bucket = test_bucket();
        let url = init_single_presigned_upload_with_content_type(
            &bucket,
            "image.png",
            1024,
            Duration::from_secs(3600),
            Some("image/png"),
        )
        .await
        .unwrap();

        assert!(url.contains("test-bucket"));
        assert!(url.contains("/image.png"));
        assert!(url.contains("X-Amz-Signature="));
    }

    #[test]
    fn test_clean_object_key() {
        // empty or whitespace
        assert!(matches!(clean_object_key(""), Err(AppError::BadRequest(_))));
        assert!(matches!(
            clean_object_key("   "),
            Err(AppError::BadRequest(_))
        ));
        assert!(matches!(
            clean_object_key("\t\n"),
            Err(AppError::BadRequest(_))
        ));

        // slash only
        assert!(matches!(
            clean_object_key("/"),
            Err(AppError::BadRequest(_))
        ));
        assert!(matches!(
            clean_object_key("///"),
            Err(AppError::BadRequest(_))
        ));
        assert!(matches!(
            clean_object_key("  ///  "),
            Err(AppError::BadRequest(_))
        ));

        // trailing slash
        let err_trailing = clean_object_key("folder/").unwrap_err();
        assert!(matches!(err_trailing, AppError::BadRequest(_)));
        assert_eq!(
            err_trailing.to_string(),
            "Invalid request: Object key cannot end with a slash"
        );

        let err_trailing_leading = clean_object_key("/folder/sub/").unwrap_err();
        assert!(matches!(err_trailing_leading, AppError::BadRequest(_)));

        // leading slash trimmed
        assert_eq!(clean_object_key("/file.txt").unwrap(), "file.txt");
        assert_eq!(clean_object_key("///file.txt").unwrap(), "file.txt");
        assert_eq!(
            clean_object_key("  /path/to/file.txt  ").unwrap(),
            "path/to/file.txt"
        );

        // nested path
        assert_eq!(
            clean_object_key("nested/dir/doc.pdf").unwrap(),
            "nested/dir/doc.pdf"
        );
        assert_eq!(
            clean_object_key("/nested/dir/doc.pdf").unwrap(),
            "nested/dir/doc.pdf"
        );
    }

    #[test]
    fn test_resolve_content_type() {
        // query precedence
        assert_eq!(
            resolve_content_type(Some("application/json"), Some("text/plain"), "doc.txt"),
            Some("application/json".to_string())
        );
        assert_eq!(
            resolve_content_type(Some("  application/json  "), Some("text/plain"), "doc.txt"),
            Some("application/json".to_string())
        );

        // query empty / whitespace falls back to header
        assert_eq!(
            resolve_content_type(Some("   "), Some("text/plain"), "doc.txt"),
            Some("text/plain".to_string())
        );
        assert_eq!(
            resolve_content_type(Some(""), Some("text/plain"), "doc.txt"),
            Some("text/plain".to_string())
        );

        // header fallback
        assert_eq!(
            resolve_content_type(None, Some("text/plain"), "image.png"),
            Some("text/plain".to_string())
        );
        assert_eq!(
            resolve_content_type(None, Some("  text/html  "), "image.png"),
            Some("text/html".to_string())
        );

        // extension fallback
        assert_eq!(
            resolve_content_type(None, None, "image.png"),
            Some("image/png".to_string())
        );
        assert_eq!(
            resolve_content_type(Some(""), Some("   "), "image.png"),
            Some("image/png".to_string())
        );
        assert_eq!(
            resolve_content_type(None, None, "archive.tar.gz"),
            Some("application/gzip".to_string())
        );

        // unknown extension
        assert_eq!(
            resolve_content_type(None, None, "unknown_file.custom_ext_12345"),
            None
        );
    }

    #[tokio::test]
    async fn test_sync_body_streaming() {
        let raw = b"streaming payload test";
        let body = axum::body::Body::from(raw.to_vec());
        let sync_body = SyncBody::new(body);
        let byte_stream = aws_sdk_s3::primitives::ByteStream::from_body_1_x(sync_body);
        let collected = byte_stream.collect().await.unwrap().into_bytes();
        assert_eq!(collected.as_ref(), raw);
    }

    #[tokio::test]
    async fn test_proxy_transfer_engine_key_validation() {
        let bucket = test_bucket();
        let engine = ProxyTransferEngine::new(&bucket, "default", None);
        let body = axum::body::Body::from(b"test".to_vec());
        let err = engine.upload("", 4, None, body).await.unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
        assert_eq!(
            err.to_string(),
            "Invalid request: Object key cannot be empty"
        );

        let body2 = axum::body::Body::from(b"test".to_vec());
        let err2 = engine.upload("folder/", 4, None, body2).await.unwrap_err();
        assert!(matches!(err2, AppError::BadRequest(_)));
        assert_eq!(
            err2.to_string(),
            "Invalid request: Object key cannot end with a slash"
        );
    }
}
