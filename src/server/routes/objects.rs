use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::error::AppError;
use crate::server::state::AppState;
use crate::sync::engine::{PrefixListing, sync_prefix_if_needed};

#[derive(Debug, Deserialize, Default)]
pub struct ListObjectsQuery {
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub refresh: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteObjectQuery {
    pub key: String,
}

/// Handler for GET /api/buckets/{profile}/objects
pub async fn list_objects(
    State(state): State<AppState>,
    Path(profile): Path<String>,
    Query(query): Query<ListObjectsQuery>,
) -> Result<Json<PrefixListing>, AppError> {
    let prefix = query.prefix.as_deref().unwrap_or("");
    let refresh = query.refresh.unwrap_or(false);

    let listing = sync_prefix_if_needed(&state, &profile, prefix, refresh).await?;
    Ok(Json(listing))
}

/// Handler for DELETE /api/buckets/{profile}/objects
pub async fn delete_object(
    State(state): State<AppState>,
    Path(profile): Path<String>,
    Query(query): Query<DeleteObjectQuery>,
) -> Result<Json<Value>, AppError> {
    if query.key.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Object key cannot be empty".to_string(),
        ));
    }

    let bucket = state.r2.get_bucket(&profile)?;

    bucket
        .delete(&query.key)
        .await
        .map_err(crate::r2::map_r2_error)?;

    state.db.delete_object(&profile, &query.key).await?;

    Ok(Json(json!({
        "status": "deleted",
        "key": query.key,
    })))
}
