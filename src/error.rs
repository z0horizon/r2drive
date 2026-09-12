use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("Database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("R2 storage error: {0}")]
    R2(String),
    #[error("Authentication error: {0}")]
    Auth(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Invalid request: {0}")]
    BadRequest(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Auth(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Config(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            AppError::Db(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::R2(msg) => (StatusCode::BAD_GATEWAY, msg.clone()),
            AppError::Io(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };

        let body = Json(json!({ "error": message }));
        (status, body).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    #[test]
    fn test_auth_error_status() {
        let err = AppError::Auth("invalid token".into());
        let res = err.into_response();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_not_found_status() {
        let err = AppError::NotFound("missing item".into());
        let res = err.into_response();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_bad_request_status() {
        let err = AppError::BadRequest("malformed input".into());
        let res = err.into_response();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_config_status() {
        let err = AppError::Config("missing file".into());
        let res = err.into_response();
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_r2_status() {
        let err = AppError::R2("upstream failure".into());
        let res = err.into_response();
        assert_eq!(res.status(), StatusCode::BAD_GATEWAY);
    }

    #[test]
    fn test_io_status() {
        let err = AppError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        let res = err.into_response();
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
