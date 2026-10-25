pub mod audit;
pub mod demo;
pub mod routes;
pub mod state;

use std::sync::Arc;

use axum::Router;
use axum::http::HeaderValue;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

pub use state::{AppState, SecurityConfig, SharedState};

pub fn build_api_router(state: SharedState) -> Router {
    routes::api_routes().with_state(state)
}

pub fn build_app(state: SharedState) -> Router {
    let web_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web");
    build_api_router(state.clone())
        .fallback_service(ServeDir::new(web_dir))
        .layer(TraceLayer::new_for_http())
        .layer(cors_layer())
}

pub async fn new_shared_state() -> anyhow::Result<SharedState> {
    Ok(Arc::new(RwLock::new(AppState::new().await?)))
}

pub async fn new_shared_state_with_audit_url(database_url: &str) -> anyhow::Result<SharedState> {
    Ok(Arc::new(RwLock::new(
        AppState::new_with_audit_url(database_url).await?,
    )))
}

pub async fn new_shared_state_with_audit_url_and_security(
    database_url: &str,
    security: SecurityConfig,
) -> anyhow::Result<SharedState> {
    Ok(Arc::new(RwLock::new(
        AppState::new_with_audit_url_and_security(database_url, security).await?,
    )))
}

fn cors_layer() -> CorsLayer {
    let layer = CorsLayer::new().allow_methods(Any).allow_headers(Any);
    match std::env::var("AML_CORS_ORIGINS") {
        Ok(origins) => {
            let origins = origins
                .split(',')
                .filter_map(|origin| origin.trim().parse::<HeaderValue>().ok())
                .collect::<Vec<_>>();
            if origins.is_empty() {
                layer.allow_origin(Any)
            } else {
                layer.allow_origin(origins)
            }
        }
        Err(_) => layer.allow_origin(Any),
    }
}
