use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode, header};
use chrono::Utc;
use r2drive::config::{BucketProfile, Config};
use r2drive::db::create_metadata_store;
use r2drive::db::models::{DbObject, MultipartSessionRecord, PrefixSyncStatus};
use r2drive::r2::R2Manager;
use r2drive::server::{AppState, cleanup_stale_multipart_sessions, create_router};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use tower::ServiceExt;

async fn setup_test_app_with_profiles(
    headless: bool,
    profiles: HashMap<String, BucketProfile>,
    default_profile: &str,
) -> (axum::Router, AppState) {
    let config = Config {
        server: r2drive::config::ServerConfig {
            admin_password: "super-secret-password".to_string(),
            session_ttl_hours: 24,
            headless,
            ..Default::default()
        },
        default_profile: default_profile.to_string(),
        profiles,
        ..Default::default()
    };

    let db = create_metadata_store("sqlite::memory:").await.unwrap();
    let r2 = Arc::new(R2Manager::new(&config).unwrap());
    let config = Arc::new(config);

    let state = AppState { config, db, r2 };
    let app = create_router(state.clone());

    (app, state)
}

async fn setup_test_app_with_headless(headless: bool) -> (axum::Router, AppState) {
    let mut profiles = HashMap::new();
    profiles.insert(
        "primary".to_string(),
        BucketProfile {
            account_id: "0123456789abcdef0123456789abcdef".to_string(),
            access_key_id: "test-access-key".to_string(),
            secret_access_key: "test-secret-key".to_string(),
            bucket_name: "test-bucket".to_string(),
            public_url: Some("https://cdn.example.com".to_string()),
        },
    );
    profiles.insert(
        "backup".to_string(),
        BucketProfile {
            account_id: "0123456789abcdef0123456789abcdef".to_string(),
            access_key_id: "test-access-key".to_string(),
            secret_access_key: "test-secret-key".to_string(),
            bucket_name: "backup-bucket".to_string(),
            public_url: None,
        },
    );

    setup_test_app_with_profiles(headless, profiles, "primary").await
}

async fn setup_test_app() -> (axum::Router, AppState) {
    setup_test_app_with_headless(false).await
}

#[tokio::test]
async fn test_unauthenticated_requests_return_401() {
    let (app, _) = setup_test_app().await;

    let endpoints = [
        ("GET", "/api/auth/me"),
        ("GET", "/api/buckets"),
        ("GET", "/api/buckets/primary/objects"),
        ("DELETE", "/api/buckets/primary/objects?key=test.txt"),
        ("POST", "/api/buckets/primary/upload/init"),
        ("POST", "/api/buckets/primary/upload/resume"),
        ("POST", "/api/buckets/primary/upload/complete"),
        ("POST", "/api/buckets/primary/upload/abort"),
        ("GET", "/api/buckets/primary/download?key=test.txt"),
        ("GET", "/api/buckets/primary/cors-probe"),
        ("POST", "/api/buckets/primary/upload/proxy?key=test.txt"),
    ];

    for (method, uri) in endpoints {
        let req = Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::empty())
            .unwrap();

        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(
            res.status(),
            StatusCode::UNAUTHORIZED,
            "Endpoint {method} {uri} should return 401 Unauthorized"
        );
    }
}

#[tokio::test]
async fn test_login_invalid_password() {
    let (app, _) = setup_test_app().await;

    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "password": "wrong-password" }).to_string(),
        ))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_login_valid_password_sets_cookie_and_returns_session() {
    let (app, state) = setup_test_app().await;

    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "password": "super-secret-password" }).to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let cookie_header = res
        .headers()
        .get(header::SET_COOKIE)
        .expect("Should set Set-Cookie header")
        .to_str()
        .unwrap();

    assert!(cookie_header.contains("r2drive_session="));
    assert!(cookie_header.contains("HttpOnly"));
    assert!(cookie_header.contains("SameSite=Lax"));

    // Extract token
    let token = cookie_header
        .split(';')
        .find_map(|part| {
            let p = part.trim();
            p.strip_prefix("r2drive_session=")
        })
        .expect("r2drive_session cookie present");

    // Verify session stored in DB
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let token_hash = format!("{:x}", hasher.finalize());
    let session = state.db.get_session(&token_hash).await.unwrap();
    assert!(session.is_some(), "Session must exist in database");

    // Test GET /api/auth/me using Cookie
    let req = Request::builder()
        .method("GET")
        .uri("/api/auth/me")
        .header(header::COOKIE, format!("r2drive_session={token}"))
        .body(Body::empty())
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(body["authenticated"], true);

    // Test GET /api/buckets using Bearer token (admin password)
    let req = Request::builder()
        .method("GET")
        .uri("/api/buckets")
        .header(header::AUTHORIZATION, "Bearer super-secret-password")
        .body(Body::empty())
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let buckets: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert!(buckets.is_array());
    assert_eq!(buckets.as_array().unwrap().len(), 2);
    assert_eq!(buckets[0]["name"], "backup");
    assert_eq!(buckets[0]["bucket"], "backup-bucket");
    assert_eq!(buckets[0]["default"], false);
    assert_eq!(buckets[1]["name"], "primary");
    assert_eq!(buckets[1]["bucket"], "test-bucket");
    assert_eq!(buckets[1]["default"], true);

    // Test POST /api/auth/logout clears cookie and purges session
    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/logout")
        .header(header::COOKIE, format!("r2drive_session={token}"))
        .body(Body::empty())
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let logout_cookie = res
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(logout_cookie.contains("Max-Age=0") || logout_cookie.contains("r2drive_session=;"));

    // Verify session removed from DB
    let session = state.db.get_session(&token_hash).await.unwrap();
    assert!(
        session.is_none(),
        "Session must be deleted from DB on logout"
    );
}

#[tokio::test]
async fn test_list_objects_cached_hit() {
    let (app, state) = setup_test_app().await;

    let now = Utc::now();
    // Pre-populate cache in SQLite
    let sync_status = PrefixSyncStatus {
        bucket_profile: "primary".to_string(),
        prefix: "documents/".to_string(),
        last_synced_at: now,
    };
    state
        .db
        .update_prefix_sync_status(&sync_status)
        .await
        .unwrap();

    let objects = vec![
        DbObject {
            id: None,
            bucket_profile: "primary".to_string(),
            object_key: "documents/subfolder/".to_string(),
            parent_prefix: "documents/".to_string(),
            is_directory: true,
            size_bytes: 0,
            etag: None,
            content_type: None,
            last_modified: now,
            synced_at: now,
        },
        DbObject {
            id: None,
            bucket_profile: "primary".to_string(),
            object_key: "documents/report.pdf".to_string(),
            parent_prefix: "documents/".to_string(),
            is_directory: false,
            size_bytes: 1024 * 50,
            etag: Some("\"etag-123\"".to_string()),
            content_type: Some("application/pdf".to_string()),
            last_modified: now,
            synced_at: now,
        },
    ];
    state.db.upsert_objects("primary", &objects).await.unwrap();

    let req = Request::builder()
        .method("GET")
        .uri("/api/buckets/primary/objects?prefix=documents/&refresh=false")
        .header(header::AUTHORIZATION, "Bearer super-secret-password")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(body["prefix"], "documents/");
    let dirs = body["directories"].as_array().unwrap();
    assert_eq!(dirs.len(), 1);
    assert_eq!(dirs[0], "documents/subfolder/");

    let objs = body["objects"].as_array().unwrap();
    assert_eq!(objs.len(), 1);
    assert_eq!(objs[0]["key"], "documents/report.pdf");
    assert_eq!(objs[0]["name"], "report.pdf");
    assert_eq!(objs[0]["size_bytes"], 1024 * 50);
    assert_eq!(objs[0]["etag"], "\"etag-123\"");
    assert_eq!(objs[0]["content_type"], "application/pdf");
}

#[tokio::test]
async fn test_upload_init_single_part_presigned_url() {
    let (app, _) = setup_test_app().await;

    let req = Request::builder()
        .method("POST")
        .uri("/api/buckets/primary/upload/init")
        .header(header::AUTHORIZATION, "Bearer super-secret-password")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "key": "photos/avatar.png",
                "size_bytes": 1024 * 500, // 500KB < 10MB
                "content_type": "image/png"
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(body["mode"], "single");
    let upload_url = body["upload_url"].as_str().unwrap();
    assert!(upload_url.contains("test-bucket"));
    assert!(upload_url.contains("photos/avatar.png"));
    assert!(upload_url.contains("X-Amz-Signature="));
}

#[tokio::test]
async fn test_upload_init_empty_key_returns_bad_request() {
    let (app, _) = setup_test_app().await;

    let req = Request::builder()
        .method("POST")
        .uri("/api/buckets/primary/upload/init")
        .header(header::AUTHORIZATION, "Bearer super-secret-password")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "key": "   ",
                "size_bytes": 1024
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_upload_resume_session_not_found() {
    let (app, _) = setup_test_app().await;

    let req = Request::builder()
        .method("POST")
        .uri("/api/buckets/primary/upload/resume")
        .header(header::AUTHORIZATION, "Bearer super-secret-password")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "upload_id": "non-existent-upload-id"
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_upload_resume_session_mismatched_profile() {
    let (app, state) = setup_test_app().await;

    let now = Utc::now();
    let record = MultipartSessionRecord {
        upload_id: "mismatched-session-id".to_string(),
        bucket_profile: "backup".to_string(), // Registered for 'backup', called via 'primary'
        object_key: "large.bin".to_string(),
        file_size: 20 * 1024 * 1024,
        part_size: 10 * 1024 * 1024,
        total_parts: 2,
        created_at: now,
        last_activity_at: now,
    };
    state.db.save_multipart_session(&record).await.unwrap();

    let req = Request::builder()
        .method("POST")
        .uri("/api/buckets/primary/upload/resume")
        .header(header::AUTHORIZATION, "Bearer super-secret-password")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "upload_id": "mismatched-session-id"
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_upload_complete_session_not_found() {
    let (app, _) = setup_test_app().await;

    let req = Request::builder()
        .method("POST")
        .uri("/api/buckets/primary/upload/complete")
        .header(header::AUTHORIZATION, "Bearer super-secret-password")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "upload_id": "missing-upload-id",
                "parts": [
                    { "part_number": 1, "etag": "\"etag-1\"" }
                ]
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_upload_abort_session_not_found() {
    let (app, _) = setup_test_app().await;

    let req = Request::builder()
        .method("POST")
        .uri("/api/buckets/primary/upload/abort")
        .header(header::AUTHORIZATION, "Bearer super-secret-password")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "upload_id": "missing-upload-id"
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_download_url_custom_public_domain() {
    let (app, _) = setup_test_app().await;

    let req = Request::builder()
        .method("GET")
        .uri("/api/buckets/primary/download?key=media/summer%20trip/photo.jpg")
        .header(header::AUTHORIZATION, "Bearer super-secret-password")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(
        body["download_url"],
        "https://cdn.example.com/media/summer%20trip/photo.jpg"
    );
}

#[tokio::test]
async fn test_download_url_presigned_fallback() {
    let (app, _) = setup_test_app().await;

    let req = Request::builder()
        .method("GET")
        .uri("/api/buckets/backup/download?key=backups/db.tar.gz")
        .header(header::AUTHORIZATION, "Bearer super-secret-password")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body_bytes).unwrap();

    let url = body["download_url"].as_str().unwrap();
    assert!(url.contains("backup-bucket"));
    assert!(url.contains("backups/db.tar.gz"));
    assert!(url.contains("X-Amz-Signature="));
}

#[tokio::test]
async fn test_download_missing_key_returns_bad_request() {
    let (app, _) = setup_test_app().await;

    let req = Request::builder()
        .method("GET")
        .uri("/api/buckets/primary/download?key=%20")
        .header(header::AUTHORIZATION, "Bearer super-secret-password")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_cleanup_stale_multipart_sessions() {
    let (_, state) = setup_test_app().await;

    let now = Utc::now();
    let stale_time = now - chrono::Duration::hours(25);
    let fresh_time = now - chrono::Duration::hours(1);

    let stale_session = MultipartSessionRecord {
        upload_id: "stale-session-456".to_string(),
        bucket_profile: "primary".to_string(),
        object_key: "old-upload.bin".to_string(),
        file_size: 20 * 1024 * 1024,
        part_size: 10 * 1024 * 1024,
        total_parts: 2,
        created_at: stale_time,
        last_activity_at: stale_time,
    };

    let fresh_session = MultipartSessionRecord {
        upload_id: "fresh-session-789".to_string(),
        bucket_profile: "primary".to_string(),
        object_key: "new-upload.bin".to_string(),
        file_size: 20 * 1024 * 1024,
        part_size: 10 * 1024 * 1024,
        total_parts: 2,
        created_at: fresh_time,
        last_activity_at: fresh_time,
    };

    state
        .db
        .save_multipart_session(&stale_session)
        .await
        .unwrap();
    state
        .db
        .save_multipart_session(&fresh_session)
        .await
        .unwrap();

    let cleaned = cleanup_stale_multipart_sessions(&state).await.unwrap();
    assert_eq!(cleaned, 1);

    // Stale session removed from DB
    let stale_check = state
        .db
        .get_multipart_session("stale-session-456")
        .await
        .unwrap();
    assert!(stale_check.is_none());

    // Fresh session preserved in DB
    let fresh_check = state
        .db
        .get_multipart_session("fresh-session-789")
        .await
        .unwrap();
    assert!(fresh_check.is_some());
}

#[tokio::test]
async fn test_cors_preflight_request() {
    let (app, _) = setup_test_app().await;

    let req = Request::builder()
        .method(Method::OPTIONS)
        .uri("/api/buckets")
        .header(header::ORIGIN, "http://localhost:3000")
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    assert_eq!(
        res.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .unwrap(),
        "http://localhost:3000"
    );
    assert_eq!(
        res.headers()
            .get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
            .unwrap(),
        "true"
    );
}

#[tokio::test]
async fn test_assets_serving_root_when_not_headless() {
    let (app, _) = setup_test_app().await;

    let req = Request::builder()
        .method("GET")
        .uri("/")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let content_type = res
        .headers()
        .get(header::CONTENT_TYPE)
        .expect("Content-Type header present")
        .to_str()
        .unwrap();
    assert!(content_type.contains("text/html"));

    let body_bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let body = String::from_utf8_lossy(&body_bytes).to_lowercase();
    assert!(
        body.contains("r2drive") || body.contains("html"),
        "Body should contain 'r2drive' or 'html'"
    );
}

#[tokio::test]
async fn test_assets_spa_fallback_for_unknown_route_when_not_headless() {
    let (app, _) = setup_test_app().await;

    let req = Request::builder()
        .method("GET")
        .uri("/some/unknown/spa/route")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let content_type = res
        .headers()
        .get(header::CONTENT_TYPE)
        .expect("Content-Type header present")
        .to_str()
        .unwrap();
    assert!(content_type.contains("text/html"));

    let body_bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let body = String::from_utf8_lossy(&body_bytes).to_lowercase();
    assert!(
        body.contains("r2drive") || body.contains("html"),
        "Body should contain index.html fallback content"
    );
}

#[tokio::test]
async fn test_assets_serving_direct_file_when_not_headless() {
    let (app, _) = setup_test_app().await;

    let req = Request::builder()
        .method("GET")
        .uri("/index.html")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let content_type = res
        .headers()
        .get(header::CONTENT_TYPE)
        .expect("Content-Type header present")
        .to_str()
        .unwrap();
    assert!(content_type.contains("text/html"));
}

#[tokio::test]
async fn test_headless_mode_root_returns_404() {
    let (app, _) = setup_test_app_with_headless(true).await;

    let req = Request::builder()
        .method("GET")
        .uri("/")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let body_bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let body = String::from_utf8_lossy(&body_bytes);
    assert!(body.contains("WebConsole disabled in headless mode"));
}

#[tokio::test]
async fn test_headless_mode_unknown_route_returns_404() {
    let (app, _) = setup_test_app_with_headless(true).await;

    let req = Request::builder()
        .method("GET")
        .uri("/some/unknown/route")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let body_bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let body = String::from_utf8_lossy(&body_bytes);
    assert!(body.contains("WebConsole disabled in headless mode"));
}

#[tokio::test]
async fn test_cors_probe_endpoint() {
    let mut profiles = HashMap::new();
    profiles.insert(
        "default".to_string(),
        BucketProfile {
            account_id: "0123456789abcdef0123456789abcdef".to_string(),
            access_key_id: "test-access-key".to_string(),
            secret_access_key: "test-secret-key".to_string(),
            bucket_name: "test-bucket".to_string(),
            public_url: None,
        },
    );
    let (app, _) = setup_test_app_with_profiles(false, profiles, "default").await;

    // Unauthenticated GET /api/buckets/default/cors-probe returns 401 Unauthorized
    let unauth_req = Request::builder()
        .method(Method::GET)
        .uri("/api/buckets/default/cors-probe")
        .body(Body::empty())
        .unwrap();
    let unauth_res = app.clone().oneshot(unauth_req).await.unwrap();
    assert_eq!(unauth_res.status(), StatusCode::UNAUTHORIZED);

    // Authenticated GET /api/buckets/nonexistent/cors-probe returns 404 NotFound
    let auth_nonexistent_req = Request::builder()
        .method(Method::GET)
        .uri("/api/buckets/nonexistent/cors-probe")
        .header(header::AUTHORIZATION, "Bearer super-secret-password")
        .body(Body::empty())
        .unwrap();
    let auth_nonexistent_res = app.clone().oneshot(auth_nonexistent_req).await.unwrap();
    assert_eq!(auth_nonexistent_res.status(), StatusCode::NOT_FOUND);

    // Authenticated GET /api/buckets/default/cors-probe returns 200 OK with JSON {"probe_url": "..."} containing /.r2drive-probe
    let auth_probe_req = Request::builder()
        .method(Method::GET)
        .uri("/api/buckets/default/cors-probe")
        .header(header::AUTHORIZATION, "Bearer super-secret-password")
        .body(Body::empty())
        .unwrap();
    let auth_probe_res = app.clone().oneshot(auth_probe_req).await.unwrap();
    assert_eq!(auth_probe_res.status(), StatusCode::OK);

    let body_bytes = to_bytes(auth_probe_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&body_bytes).unwrap();
    let probe_url = body["probe_url"]
        .as_str()
        .expect("probe_url field in response");
    assert!(
        probe_url.contains(".r2drive-probe"),
        "probe_url should contain .r2drive-probe"
    );
    assert!(
        probe_url.contains("X-Amz-Signature="),
        "probe_url should be a signed URL"
    );

    let fallback_policy = &body["fallback_policy"];
    assert_eq!(fallback_policy["enabled"], true);
    assert_eq!(
        fallback_policy["max_payload_bytes"],
        5 * 1024 * 1024 * 1024u64
    );
}

#[tokio::test]
async fn test_upload_proxy_endpoint() {
    let mut profiles = HashMap::new();
    profiles.insert(
        "default".to_string(),
        BucketProfile {
            account_id: "0123456789abcdef0123456789abcdef".to_string(),
            access_key_id: "test-access-key".to_string(),
            secret_access_key: "test-secret-key".to_string(),
            bucket_name: "test-bucket".to_string(),
            public_url: None,
        },
    );
    let (app, _) = setup_test_app_with_profiles(false, profiles, "default").await;

    // 1. Unauthenticated POST /api/buckets/default/upload/proxy?key=hello.txt returns 401 Unauthorized
    let unauth_req = Request::builder()
        .method(Method::POST)
        .uri("/api/buckets/default/upload/proxy?key=hello.txt")
        .header(header::CONTENT_LENGTH, "11")
        .body(Body::from("hello world"))
        .unwrap();
    let unauth_res = app.clone().oneshot(unauth_req).await.unwrap();
    assert_eq!(unauth_res.status(), StatusCode::UNAUTHORIZED);

    // 2. Authenticated POST with empty key returns 400 BadRequest ("Object key cannot be empty")
    let empty_key_req = Request::builder()
        .method(Method::POST)
        .uri("/api/buckets/default/upload/proxy?key=")
        .header(header::AUTHORIZATION, "Bearer super-secret-password")
        .header(header::CONTENT_LENGTH, "11")
        .body(Body::from("hello world"))
        .unwrap();
    let empty_key_res = app.clone().oneshot(empty_key_req).await.unwrap();
    assert_eq!(empty_key_res.status(), StatusCode::BAD_REQUEST);
    let body_bytes = to_bytes(empty_key_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(body["error"], "Object key cannot be empty");

    // Also test whitespace key returns 400 BadRequest
    let ws_key_req = Request::builder()
        .method(Method::POST)
        .uri("/api/buckets/default/upload/proxy?key=%20%20")
        .header(header::AUTHORIZATION, "Bearer super-secret-password")
        .header(header::CONTENT_LENGTH, "11")
        .body(Body::from("hello world"))
        .unwrap();
    let ws_key_res = app.clone().oneshot(ws_key_req).await.unwrap();
    assert_eq!(ws_key_res.status(), StatusCode::BAD_REQUEST);
    let ws_bytes = to_bytes(ws_key_res.into_body(), usize::MAX).await.unwrap();
    let ws_body: Value = serde_json::from_slice(&ws_bytes).unwrap();
    assert_eq!(ws_body["error"], "Object key cannot be empty");

    // Also test slash-only key returns 400 BadRequest
    let slash_key_req = Request::builder()
        .method(Method::POST)
        .uri("/api/buckets/default/upload/proxy?key=///")
        .header(header::AUTHORIZATION, "Bearer super-secret-password")
        .header(header::CONTENT_LENGTH, "11")
        .body(Body::from("hello world"))
        .unwrap();
    let slash_key_res = app.clone().oneshot(slash_key_req).await.unwrap();
    assert_eq!(slash_key_res.status(), StatusCode::BAD_REQUEST);
    let slash_bytes = to_bytes(slash_key_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let slash_body: Value = serde_json::from_slice(&slash_bytes).unwrap();
    assert_eq!(slash_body["error"], "Object key cannot be empty");

    // 3. Authenticated POST with missing Content-Length header returns 400 BadRequest
    let no_cl_req = Request::builder()
        .method(Method::POST)
        .uri("/api/buckets/default/upload/proxy?key=hello.txt")
        .header(header::AUTHORIZATION, "Bearer super-secret-password")
        .body(Body::from("hello world"))
        .unwrap();
    let no_cl_res = app.clone().oneshot(no_cl_req).await.unwrap();
    assert_eq!(no_cl_res.status(), StatusCode::BAD_REQUEST);
    let no_cl_bytes = to_bytes(no_cl_res.into_body(), usize::MAX).await.unwrap();
    let no_cl_body: Value = serde_json::from_slice(&no_cl_bytes).unwrap();
    assert_eq!(
        no_cl_body["error"],
        "Content-Length header required for proxy upload"
    );

    // 3b. Authenticated POST with trailing slash key returns 400 BadRequest
    let trailing_slash_req = Request::builder()
        .method(Method::POST)
        .uri("/api/buckets/default/upload/proxy?key=folder/")
        .header(header::AUTHORIZATION, "Bearer super-secret-password")
        .header(header::CONTENT_LENGTH, "11")
        .body(Body::from("hello world"))
        .unwrap();
    let trailing_slash_res = app.clone().oneshot(trailing_slash_req).await.unwrap();
    assert_eq!(trailing_slash_res.status(), StatusCode::BAD_REQUEST);
    let trailing_slash_bytes = to_bytes(trailing_slash_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let trailing_slash_body: Value = serde_json::from_slice(&trailing_slash_bytes).unwrap();
    assert_eq!(
        trailing_slash_body["error"],
        "Object key cannot end with a slash"
    );

    // 4. Authenticated POST with missing/invalid profile returns 404 NotFound
    let not_found_req = Request::builder()
        .method(Method::POST)
        .uri("/api/buckets/nonexistent/upload/proxy?key=hello.txt")
        .header(header::AUTHORIZATION, "Bearer super-secret-password")
        .header(header::CONTENT_LENGTH, "11")
        .body(Body::from("hello world"))
        .unwrap();
    let not_found_res = app.clone().oneshot(not_found_req).await.unwrap();
    assert_eq!(not_found_res.status(), StatusCode::NOT_FOUND);

    // 5. Authenticated POST with whitespace-padded Content-Length is accepted
    let ws_cl_req = Request::builder()
        .method(Method::POST)
        .uri("/api/buckets/default/upload/proxy?key=test_ws_cl.txt")
        .header(header::AUTHORIZATION, "Bearer super-secret-password")
        .header(header::CONTENT_LENGTH, "  11 \t")
        .body(Body::from("hello world"))
        .unwrap();
    let ws_cl_res = app.clone().oneshot(ws_cl_req).await.unwrap();
    // Passes validation and attempts transfer (returns BAD_GATEWAY without real R2 creds, not BAD_REQUEST)
    assert_ne!(ws_cl_res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_cors_probe_broadcasts_configured_policy() {
    let transfers = r2drive::config::TransfersConfig {
        proxy_fallback: false,
        max_proxy_file_size: "250MB".to_string(),
    };
    let (app, _) = setup_test_app_with_transfers_config(transfers).await;

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/buckets/default/cors-probe")
        .header(header::AUTHORIZATION, "Bearer super-secret-password")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(body["fallback_policy"]["enabled"], false);
    assert_eq!(
        body["fallback_policy"]["max_payload_bytes"],
        250 * 1024 * 1024u64
    );
}

async fn setup_test_app_with_transfers_config(
    transfers: r2drive::config::TransfersConfig,
) -> (axum::Router, AppState) {
    let mut profiles = HashMap::new();
    profiles.insert(
        "default".to_string(),
        BucketProfile {
            account_id: "0123456789abcdef0123456789abcdef".to_string(),
            access_key_id: "test-access-key".to_string(),
            secret_access_key: "test-secret-key".to_string(),
            bucket_name: "test-bucket".to_string(),
            public_url: None,
        },
    );
    let config = Config {
        server: r2drive::config::ServerConfig {
            admin_password: "super-secret-password".to_string(),
            session_ttl_hours: 24,
            headless: false,
            ..Default::default()
        },
        default_profile: "default".to_string(),
        profiles,
        transfers,
        ..Default::default()
    };

    let db = create_metadata_store("sqlite::memory:").await.unwrap();
    let r2 = Arc::new(R2Manager::new(&config).unwrap());
    let config = Arc::new(config);

    let state = AppState { config, db, r2 };
    let app = create_router(state.clone());

    (app, state)
}

#[tokio::test]
async fn test_upload_proxy_fallback_disabled_returns_403() {
    let transfers = r2drive::config::TransfersConfig {
        proxy_fallback: false,
        max_proxy_file_size: "5GB".to_string(),
    };
    let (app, _) = setup_test_app_with_transfers_config(transfers).await;

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/buckets/default/upload/proxy?key=test.txt")
        .header(header::AUTHORIZATION, "Bearer super-secret-password")
        .header(header::CONTENT_LENGTH, "11")
        .body(Body::from("hello world"))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        body["error"],
        "Proxy upload fallback is disabled by server configuration"
    );
}

#[tokio::test]
async fn test_upload_proxy_payload_too_large_returns_413() {
    let transfers = r2drive::config::TransfersConfig {
        proxy_fallback: true,
        max_proxy_file_size: "10B".to_string(),
    };
    let (app, _) = setup_test_app_with_transfers_config(transfers).await;

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/buckets/default/upload/proxy?key=test.txt")
        .header(header::AUTHORIZATION, "Bearer super-secret-password")
        .header(header::CONTENT_LENGTH, "11")
        .body(Body::from("hello world"))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("exceeds maximum proxy upload size limit")
    );
}
