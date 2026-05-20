//! lumo-bridge -- daemon HTTP que expoe controle remoto do Lumo OS para agentes LLM.
//!
//! Bind: 0.0.0.0:7778
//! Auth: Bearer token em ~/.config/lumo/bridge-token (gerado no startup se ausente)
//! Log: /tmp/lumo-bridge.log

use anyhow::{Context, Result};
use axum::{
    extract::Extension,
    http::StatusCode,
    middleware,
    response::Json,
    routing::{get, post},
    Router,
};
use serde_json::json;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

mod auth;
mod exec;
mod lumo_ipc;
mod routes;

#[derive(Clone)]
pub struct AppState {
    pub token: Arc<String>,
    pub started_at: u64,
    pub screenshot_cache: Arc<tokio::sync::Mutex<Option<(Instant, bytes::Bytes)>>>,
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const BIND_ADDR: &str = "0.0.0.0:7778";

fn token_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".config/lumo/bridge-token")
}

fn load_or_create_token() -> Result<String> {
    let path = token_path();
    if let Ok(existing) = std::fs::read_to_string(&path) {
        let t = existing.trim().to_string();
        if !t.is_empty() {
            return Ok(t);
        }
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let new_token = uuid::Uuid::new_v4().to_string();
    std::fs::write(&path, &new_token).context("write token file")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&path, perms)?;
    }
    Ok(new_token)
}

async fn healthz(Extension(state): Extension<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::OK,
        Json(json!({
            "service": "lumo-bridge",
            "version": VERSION,
            "started_at": state.started_at,
        })),
    )
}

pub fn build_router(state: AppState) -> Router {
    let protected = Router::new()
        .route("/state", get(routes::state::get_state))
        .route("/screenshot", get(routes::screenshot::get_screenshot))
        .route("/pointer/click", post(routes::pointer::click))
        .route("/pointer/move", post(routes::pointer::pointer_move))
        .route("/pointer/drag", post(routes::pointer::drag))
        .route("/pointer/scroll", post(routes::pointer::scroll))
        .route("/keyboard/type", post(routes::keyboard::type_text))
        .route("/keyboard/key", post(routes::keyboard::key_sequence))
        .route("/log/tail", get(routes::log::tail))
        .route("/state/dump", get(routes::state::dump))
        .route("/procs", get(routes::state::procs))
        .layer(middleware::from_fn_with_state(state.clone(), auth::require_bearer));

    Router::new()
        .route("/healthz", get(healthz))
        .merge(protected)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .layer(Extension(state))
}

#[tokio::main]
async fn main() -> Result<()> {
    // Log writer pra arquivo + stdout
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/lumo-bridge.log")
        .ok();
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    if let Some(f) = log_file {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(f)
            .with_ansi(false)
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(filter).init();
    }

    let token = load_or_create_token()?;
    tracing::info!("lumo-bridge v{} starting on {}", VERSION, BIND_ADDR);
    tracing::info!("token path: {}", token_path().display());

    let started_at = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let state = AppState {
        token: Arc::new(token),
        started_at,
        screenshot_cache: Arc::new(tokio::sync::Mutex::new(None)),
    };

    let app = build_router(state);
    let addr: SocketAddr = BIND_ADDR.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("listening on {}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod http_tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::util::ServiceExt;

    fn test_state() -> AppState {
        AppState {
            token: Arc::new("test-token-abc".to_string()),
            started_at: 0,
            screenshot_cache: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    #[tokio::test]
    async fn auth_missing_token_returns_401() {
        let app = build_router(test_state());
        let req = Request::builder().uri("/state").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn auth_bad_token_returns_401() {
        let app = build_router(test_state());
        let req = Request::builder()
            .uri("/state")
            .header("Authorization", "Bearer wrong-token")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn healthz_is_public() {
        let app = build_router(test_state());
        let req = Request::builder().uri("/healthz").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn screenshot_returns_png_or_503() {
        let app = build_router(test_state());
        let req = Request::builder()
            .uri("/screenshot")
            .header("Authorization", "Bearer test-token-abc")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        // Em ambiente sem WAYLAND_DISPLAY do test runner, grim falha -> 503.
        // Em ambiente com Wayland ativo, retorna 200 image/png.
        let status = resp.status();
        assert!(
            status == StatusCode::OK || status == StatusCode::SERVICE_UNAVAILABLE,
            "expected 200 or 503, got {}",
            status
        );
        if status == StatusCode::OK {
            let ct = resp.headers().get("content-type").and_then(|v| v.to_str().ok());
            assert_eq!(ct, Some("image/png"));
        }
    }
}
