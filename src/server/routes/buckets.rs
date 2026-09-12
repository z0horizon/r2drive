use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::server::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BucketProfileResponse {
    pub name: String,
    pub bucket: String,
    #[serde(rename = "default")]
    pub is_default: bool,
}

/// Handler for GET /api/buckets
pub async fn list_buckets(
    State(state): State<AppState>,
) -> Result<Json<Vec<BucketProfileResponse>>, AppError> {
    let mut profiles: Vec<BucketProfileResponse> = state
        .config
        .profiles
        .iter()
        .map(|(name, profile)| {
            let is_default = *name == state.config.default_profile
                || state.r2.default_profile_name() == Some(name.as_str());
            BucketProfileResponse {
                name: name.clone(),
                bucket: profile.bucket_name.clone(),
                is_default,
            }
        })
        .collect();

    profiles.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(Json(profiles))
}
