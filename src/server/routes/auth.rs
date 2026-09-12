use axum::{
    Extension, Json,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;

use crate::db::models::Session;
use crate::error::AppError;
use crate::server::middleware::AuthenticatedSession;
use crate::server::state::AppState;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub password: String,
}

/// Handler for POST /api/auth/login
pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Response, AppError> {
    if payload.password != state.config.server.admin_password {
        return Err(AppError::Auth("Invalid admin password".to_string()));
    }

    let token = format!("{:032x}", rand::random::<u128>());
    let now = Utc::now();
    let expires_at = now + chrono::Duration::hours(state.config.server.session_ttl_hours as i64);

    let session = Session {
        token_hash: token.clone(),
        created_at: now,
        expires_at,
    };
    state.db.save_session(&session).await?;

    let cookie_val = format!("r2drive_session={token}; HttpOnly; SameSite=Lax; Path=/");
    let mut response = (
        StatusCode::OK,
        Json(json!({
            "status": "authenticated",
            "expires_at": expires_at.to_rfc3339(),
        })),
    )
        .into_response();

    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie_val)
            .map_err(|e| AppError::Config(format!("Invalid cookie value: {e}")))?,
    );

    Ok(response)
}

/// Handler for POST /api/auth/logout
pub async fn logout(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedSession>>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    // 1. Delete session from DB if present via extension
    if let Some(Extension(auth_session)) = auth
        && let Some(ref token) = auth_session.token
    {
        let _ = state.db.delete_session(token).await;
    }

    // 2. Also check Cookie header directly in case extension wasn't set
    if let Some(cookie_header) = headers.get(header::COOKIE)
        && let Ok(cookie_str) = cookie_header.to_str()
    {
        for part in cookie_str.split(';') {
            let part = part.trim();
            if let Some(token) = part.strip_prefix("r2drive_session=") {
                let _ = state.db.delete_session(token.trim()).await;
            }
        }
    }

    let mut response = (StatusCode::OK, Json(json!({ "status": "logged_out" }))).into_response();

    let clear_cookie = "r2drive_session=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0";
    response
        .headers_mut()
        .insert(header::SET_COOKIE, HeaderValue::from_static(clear_cookie));

    Ok(response)
}

/// Handler for GET /api/auth/me
pub async fn me() -> Result<Response, AppError> {
    Ok((StatusCode::OK, Json(json!({ "authenticated": true }))).into_response())
}
