use axum::{
    Json,
    extract::{Path, Query, State},
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;

use crate::db::models::MultipartSessionRecord;
use crate::error::AppError;
use crate::r2::transfer::{
    CompletedPartReceipt, DEFAULT_PART_SIZE, abort_multipart_upload, complete_multipart_upload,
    generate_download_url, init_presigned_upload_with_content_type,
    init_single_presigned_upload_with_content_type, resume_multipart_upload,
};
use crate::server::state::AppState;

const MULTIPART_THRESHOLD: u64 = DEFAULT_PART_SIZE; // 10MB (10_485_760 bytes)
const URL_EXPIRATION: Duration = Duration::from_secs(3600); // 1 hour

#[derive(Debug, Deserialize)]
pub struct InitUploadRequest {
    pub key: String,
    pub size_bytes: u64,
    #[serde(default)]
    pub content_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ResumeUploadRequest {
    pub upload_id: String,
}

#[derive(Debug, Deserialize)]
pub struct CompleteUploadRequest {
    #[serde(default)]
    pub key: Option<String>,
    pub upload_id: String,
    pub parts: Vec<CompletedPartReceipt>,
}

#[derive(Debug, Deserialize)]
pub struct AbortUploadRequest {
    pub upload_id: String,
}

#[derive(Debug, Deserialize)]
pub struct DownloadQuery {
    pub key: String,
}

/// Handler for POST /api/buckets/{profile}/upload/init
pub async fn init_upload(
    State(state): State<AppState>,
    Path(profile): Path<String>,
    Json(payload): Json<InitUploadRequest>,
) -> Result<Json<Value>, AppError> {
    if payload.key.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Object key cannot be empty".to_string(),
        ));
    }

    let bucket = state.r2.get_bucket(&profile)?;

    if payload.size_bytes < MULTIPART_THRESHOLD {
        let upload_url = init_single_presigned_upload_with_content_type(
            &bucket,
            &payload.key,
            payload.size_bytes,
            URL_EXPIRATION,
            payload.content_type.as_deref(),
        )
        .await?;

        Ok(Json(json!({
            "mode": "single",
            "upload_url": upload_url,
        })))
    } else {
        let part_size = DEFAULT_PART_SIZE;
        let plan = init_presigned_upload_with_content_type(
            &bucket,
            &payload.key,
            payload.size_bytes,
            part_size,
            URL_EXPIRATION,
            payload.content_type.as_deref(),
        )
        .await?;

        let now = Utc::now();
        let record = MultipartSessionRecord {
            upload_id: plan.upload_id.clone(),
            bucket_profile: profile.clone(),
            object_key: payload.key.clone(),
            file_size: payload.size_bytes as i64,
            part_size: part_size as i64,
            total_parts: plan.parts.len() as i64,
            created_at: now,
            last_activity_at: now,
        };
        state.db.save_multipart_session(&record).await?;

        Ok(Json(json!({
            "mode": "multipart",
            "upload_id": plan.upload_id,
            "part_size": plan.part_size,
            "parts": plan.parts,
        })))
    }
}

/// Handler for POST /api/buckets/{profile}/upload/resume
pub async fn resume_upload(
    State(state): State<AppState>,
    Path(profile): Path<String>,
    Json(payload): Json<ResumeUploadRequest>,
) -> Result<Json<Value>, AppError> {
    let session = state
        .db
        .get_multipart_session(&payload.upload_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "Multipart upload session '{}' not found",
                payload.upload_id
            ))
        })?;

    if session.bucket_profile != profile {
        return Err(AppError::NotFound(format!(
            "Multipart upload session '{}' does not belong to bucket profile '{}'",
            payload.upload_id, profile
        )));
    }

    let bucket = state.r2.get_bucket(&profile)?;
    let resumed = resume_multipart_upload(
        &bucket,
        &session.object_key,
        &session.upload_id,
        session.file_size as u64,
        session.part_size as u64,
        URL_EXPIRATION,
    )
    .await?;

    Ok(Json(json!({
        "upload_id": resumed.upload_id,
        "completed_parts": resumed.completed_parts,
        "remaining_parts": resumed.remaining_parts,
    })))
}

/// Handler for POST /api/buckets/{profile}/upload/complete
pub async fn complete_upload(
    State(state): State<AppState>,
    Path(profile): Path<String>,
    Json(payload): Json<CompleteUploadRequest>,
) -> Result<Json<Value>, AppError> {
    let session = state
        .db
        .get_multipart_session(&payload.upload_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "Multipart upload session '{}' not found",
                payload.upload_id
            ))
        })?;

    if session.bucket_profile != profile {
        return Err(AppError::NotFound(format!(
            "Multipart upload session '{}' does not belong to bucket profile '{}'",
            payload.upload_id, profile
        )));
    }

    let bucket = state.r2.get_bucket(&profile)?;
    let key = payload.key.as_deref().unwrap_or(&session.object_key);
    let parts: Vec<(u16, String)> = payload.parts.into_iter().map(Into::into).collect();

    let etag = complete_multipart_upload(
        &bucket,
        key,
        &session.upload_id,
        session.file_size as u64,
        session.part_size as u64,
        parts,
    )
    .await?;

    state
        .db
        .delete_multipart_session(&session.upload_id)
        .await?;

    Ok(Json(json!({
        "status": "completed",
        "etag": etag,
    })))
}

/// Handler for POST /api/buckets/{profile}/upload/abort
pub async fn abort_upload(
    State(state): State<AppState>,
    Path(profile): Path<String>,
    Json(payload): Json<AbortUploadRequest>,
) -> Result<Json<Value>, AppError> {
    let session = state
        .db
        .get_multipart_session(&payload.upload_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "Multipart upload session '{}' not found",
                payload.upload_id
            ))
        })?;

    if session.bucket_profile != profile {
        return Err(AppError::NotFound(format!(
            "Multipart upload session '{}' does not belong to bucket profile '{}'",
            payload.upload_id, profile
        )));
    }

    let bucket = state.r2.get_bucket(&profile)?;
    if let Err(err) = abort_multipart_upload(&bucket, &session.object_key, &session.upload_id).await
        && !matches!(err, AppError::NotFound(_))
    {
        return Err(err);
    }

    state
        .db
        .delete_multipart_session(&session.upload_id)
        .await?;

    Ok(Json(json!({
        "status": "aborted",
    })))
}

/// Handler for GET /api/buckets/{profile}/download?key=path/to/file.ext
pub async fn download_object(
    State(state): State<AppState>,
    Path(profile): Path<String>,
    Query(query): Query<DownloadQuery>,
) -> Result<Json<Value>, AppError> {
    if query.key.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Query parameter 'key' cannot be empty".to_string(),
        ));
    }

    let bucket = state.r2.get_bucket(&profile)?;
    let custom_public_url = state
        .config
        .get_profile(&profile)
        .and_then(|p| p.public_url.as_deref());

    let download_url =
        generate_download_url(&bucket, &query.key, URL_EXPIRATION, custom_public_url).await?;

    Ok(Json(json!({
        "download_url": download_url,
    })))
}
