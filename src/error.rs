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
    #[error("Database migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("R2 storage error: {0}")]
    R2(String),
    #[error("Authentication error: {0}")]
    Auth(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Invalid request: {0}")]
    BadRequest(String),
    #[error("Forbidden: {0}")]
    Forbidden(String),
    #[error("Payload too large: {0}")]
    PayloadTooLarge(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Auth(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::PayloadTooLarge(msg) => (StatusCode::PAYLOAD_TOO_LARGE, msg.clone()),
            AppError::Config(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            AppError::Db(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::Migration(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::R2(msg) => (StatusCode::BAD_GATEWAY, msg.clone()),
            AppError::Io(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };

        let body = Json(json!({ "error": message }));
        (status, body).into_response()
    }
}

impl From<r2kit::ConfigError> for AppError {
    fn from(err: r2kit::ConfigError) -> Self {
        AppError::Config(err.to_string())
    }
}

impl From<r2kit::Error> for AppError {
    fn from(err: r2kit::Error) -> Self {
        if err.is_not_found() {
            return AppError::NotFound(err.to_string());
        }
        match err {
            r2kit::Error::Validation(e) => AppError::BadRequest(e.to_string()),
            r2kit::Error::InvalidInput { field, reason } => {
                AppError::BadRequest(format!("Invalid input for {field}: {reason}"))
            }
            r2kit::Error::Remote(ref se)
                if se.kind() == r2kit::ServiceErrorKind::Authentication =>
            {
                AppError::Auth(se.to_string())
            }
            r2kit::Error::Config(ce) => AppError::Config(ce.to_string()),
            other => AppError::R2(other.to_string()),
        }
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

    #[test]
    fn test_migration_status() {
        let err = AppError::Migration(sqlx::migrate::MigrateError::VersionMissing(1));
        let res = err.into_response();
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_r2kit_validation_error_mapping() {
        let r2_err = r2kit::Error::Validation(r2kit::ValidationError::MultipartFileSizeZero);
        let app_err: AppError = r2_err.into();
        assert!(matches!(app_err, AppError::BadRequest(_)));
        assert_eq!(app_err.into_response().status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_r2kit_invalid_input_error_mapping() {
        let r2_err = r2kit::Error::InvalidInput {
            field: "key",
            reason: "must not be empty",
        };
        let app_err: AppError = r2_err.into();
        assert!(matches!(app_err, AppError::BadRequest(_)));
        assert_eq!(app_err.into_response().status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_r2kit_not_found_error_mapping() {
        let r2_err = r2kit::Error::NotFound;
        assert!(r2_err.is_not_found());
        let app_err: AppError = r2_err.into();
        assert!(matches!(app_err, AppError::NotFound(_)));
        assert_eq!(app_err.into_response().status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_r2kit_config_error_mapping() {
        let cfg_err = r2kit::ConfigError::InvalidAccountId;
        let app_err: AppError = cfg_err.into();
        assert!(matches!(app_err, AppError::Config(_)));
        assert_eq!(
            app_err.into_response().status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn test_r2kit_presign_error_mapping() {
        let r2_err = r2kit::Error::Presign;
        let app_err: AppError = r2_err.into();
        assert!(matches!(app_err, AppError::R2(_)));
        assert_eq!(app_err.into_response().status(), StatusCode::BAD_GATEWAY);
    }

    #[test]
    fn test_forbidden_status() {
        let err = AppError::Forbidden("access denied".to_string());
        let res = err.into_response();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn test_payload_too_large_status() {
        let err = AppError::PayloadTooLarge("too large".to_string());
        let res = err.into_response();
        assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }
}
