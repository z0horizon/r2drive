use super::state::AppState;
use crate::error::AppError;
use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::Response,
};
use chrono::Utc;

/// Context for the authenticated caller.
#[derive(Clone, Debug, Default)]
pub struct AuthenticatedSession {
    pub token: Option<String>,
}

/// Middleware to enforce authentication via either:
/// 1. `Authorization: Bearer <token>` (matches admin_password or active session in DB)
/// 2. `Cookie: r2drive_session=<token>` (matches active session in DB)
pub async fn require_auth(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    // 1. Check Authorization header: Bearer <token>
    let bearer_token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|auth_str| auth_str.strip_prefix("Bearer "))
        .map(|token| token.trim().to_string());

    if let Some(token) = bearer_token {
        // Admin password match
        if token == state.config.server.admin_password {
            req.extensions_mut()
                .insert(AuthenticatedSession { token: None });
            return Ok(next.run(req).await);
        }

        // Session token match
        if let Ok(Some(session)) = state.db.get_session(&token).await
            && session.expires_at > Utc::now()
        {
            req.extensions_mut()
                .insert(AuthenticatedSession { token: Some(token) });
            return Ok(next.run(req).await);
        }
    }

    // 2. Check Cookie header: r2drive_session=<token>
    let cookie_token = req
        .headers()
        .get(header::COOKIE)
        .and_then(|h| h.to_str().ok())
        .and_then(|cookie_str| {
            cookie_str.split(';').find_map(|part| {
                part.trim()
                    .strip_prefix("r2drive_session=")
                    .map(|t| t.trim().to_string())
            })
        });

    if let Some(token) = cookie_token
        && let Ok(Some(session)) = state.db.get_session(&token).await
        && session.expires_at > Utc::now()
    {
        req.extensions_mut()
            .insert(AuthenticatedSession { token: Some(token) });
        return Ok(next.run(req).await);
    }

    Err(AppError::Auth(
        "Unauthorized: Missing or invalid authentication token".to_string(),
    ))
}
