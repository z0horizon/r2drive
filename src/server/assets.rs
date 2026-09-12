use axum::Json;
use axum::http::{Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;
use serde_json::json;

#[derive(RustEmbed)]
#[folder = "web/dist/"]
pub struct Assets;

/// Serves static assets from embedded web/dist/ folder with SPA fallback to index.html.
pub async fn static_handler(method: Method, uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    if path.starts_with("api/") || path == "api" {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "API endpoint not found" })),
        )
            .into_response();
    }

    if method != Method::GET && method != Method::HEAD {
        return (StatusCode::METHOD_NOT_ALLOWED, "Method Not Allowed").into_response();
    }

    if let Some(content) = Assets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
    } else if let Some(index) = Assets::get("index.html") {
        ([(header::CONTENT_TYPE, "text/html")], index.data).into_response()
    } else {
        (StatusCode::NOT_FOUND, "WebConsole assets not found").into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_assets_contain_index_html() {
        let index = Assets::get("index.html");
        assert!(index.is_some(), "Embedded assets must contain index.html");
        let data = index.unwrap().data;
        let html = std::str::from_utf8(&data).unwrap();
        assert!(html.contains("<html"));
        assert!(html.contains("r2drive"));
    }

    #[tokio::test]
    async fn test_static_handler_root_returns_index_html() {
        let uri: Uri = "/".parse().unwrap();
        let res = static_handler(Method::GET, uri).await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/html"
        );
    }

    #[tokio::test]
    async fn test_static_handler_spa_fallback() {
        let uri: Uri = "/buckets/my-bucket/nested/view".parse().unwrap();
        let res = static_handler(Method::GET, uri).await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/html"
        );
    }

    #[tokio::test]
    async fn test_static_handler_api_404() {
        let uri: Uri = "/api/unknown/endpoint".parse().unwrap();
        let res = static_handler(Method::GET, uri).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_static_handler_method_not_allowed() {
        let uri: Uri = "/".parse().unwrap();
        let res = static_handler(Method::POST, uri).await;
        assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);
    }
}
