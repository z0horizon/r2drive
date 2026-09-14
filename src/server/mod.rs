pub mod assets;
pub mod middleware;
pub mod routes;
pub mod state;

pub use state::AppState;

use axum::{
    Router,
    routing::{get, post},
};
use chrono::Utc;
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::config::Config;
use crate::error::AppError;

/// Builds the Axum router with all API routes and middleware.
pub fn create_router(state: AppState) -> Router {
    let public_routes = Router::new().route("/api/auth/login", post(routes::auth::login));

    let protected_routes = Router::new()
        .route("/api/auth/logout", post(routes::auth::logout))
        .route("/api/auth/me", get(routes::auth::me))
        .route("/api/buckets", get(routes::buckets::list_buckets))
        .route(
            "/api/buckets/{profile}/objects",
            get(routes::objects::list_objects).delete(routes::objects::delete_object),
        )
        .route(
            "/api/buckets/{profile}/upload/init",
            post(routes::transfers::init_upload),
        )
        .route(
            "/api/buckets/{profile}/upload/resume",
            post(routes::transfers::resume_upload),
        )
        .route(
            "/api/buckets/{profile}/upload/complete",
            post(routes::transfers::complete_upload),
        )
        .route(
            "/api/buckets/{profile}/upload/abort",
            post(routes::transfers::abort_upload),
        )
        .route(
            "/api/buckets/{profile}/download",
            get(routes::transfers::download_object),
        )
        .route(
            "/api/buckets/{profile}/cors-probe",
            get(routes::transfers::cors_probe),
        )
        .route(
            "/api/buckets/{profile}/upload/proxy",
            post(routes::transfers::upload_proxy),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::require_auth,
        ));

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::mirror_request())
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::DELETE,
            axum::http::Method::PUT,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::ACCEPT,
            axum::http::header::CONTENT_TYPE,
            axum::http::header::COOKIE,
        ])
        .allow_credentials(true);

    let router = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(cors);

    let router = if !state.config.server.headless {
        router.fallback(assets::static_handler)
    } else {
        router.fallback(headless_fallback)
    };

    router.with_state(state)
}

/// Fallback handler when WebConsole is disabled in headless mode.
async fn headless_fallback() -> (axum::http::StatusCode, &'static str) {
    (
        axum::http::StatusCode::NOT_FOUND,
        "WebConsole disabled in headless mode",
    )
}

/// Cleans up stale multipart upload sessions older than 24 hours.
pub async fn cleanup_stale_multipart_sessions(state: &AppState) -> Result<usize, AppError> {
    let cutoff = Utc::now() - chrono::Duration::hours(24);
    let stale_sessions = state.db.list_stale_multipart_sessions(cutoff).await?;
    let mut cleaned = 0;

    for session in stale_sessions {
        if let Ok(bucket) = state.r2.get_bucket(&session.bucket_profile)
            && let Err(e) = crate::r2::transfer::abort_multipart_upload(
                &bucket,
                &session.object_key,
                &session.upload_id,
            )
            .await
        {
            tracing::warn!(
                "Failed to abort multipart upload on R2 for session {}: {e}",
                session.upload_id
            );
        }

        if let Err(e) = state.db.delete_multipart_session(&session.upload_id).await {
            tracing::warn!(
                "Failed to delete multipart session {} from DB: {e}",
                session.upload_id
            );
        } else {
            cleaned += 1;
        }
    }

    Ok(cleaned)
}

/// Helper to wait for SIGINT or SIGTERM signals.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(e) => {
                tracing::warn!("Failed to install SIGTERM handler: {e}");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

/// Starts the StorageNode REST API server and background workers.
pub async fn run(
    mut config: Config,
    port_override: Option<u16>,
    headless: bool,
) -> Result<(), AppError> {
    if headless {
        config.server.headless = true;
    }
    let port = port_override.unwrap_or(config.server.port);
    let host = &config.server.host;
    let addr = format!("{}:{}", host, port);

    let db = crate::db::create_metadata_store(&config.database.url).await?;
    let r2 = Arc::new(crate::r2::R2Manager::new(&config)?);
    let config = Arc::new(config);

    let state = AppState::new(config, db, r2);

    // Spawn background cleanup daemon
    let cleanup_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        // Skip immediate first tick
        interval.tick().await;
        loop {
            interval.tick().await;
            match cleanup_stale_multipart_sessions(&cleanup_state).await {
                Ok(count) => {
                    if count > 0 {
                        tracing::info!("Cleaned up {count} stale multipart upload sessions");
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to cleanup stale multipart sessions: {e}");
                }
            }
        }
    });

    let app = create_router(state);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("StorageNode REST API listening on {}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("StorageNode REST API server stopped gracefully");
    Ok(())
}
