//! GET /screenshot -- captura tela com grim, cache 200ms.

use axum::{
    extract::Extension,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use std::time::{Duration, Instant};

use crate::{exec, AppState};

const CACHE_TTL: Duration = Duration::from_millis(200);

pub async fn get_screenshot(Extension(state): Extension<AppState>) -> Response {
    // Cache hit?
    {
        let cache = state.screenshot_cache.lock().await;
        if let Some((at, bytes)) = cache.as_ref() {
            if at.elapsed() < CACHE_TTL {
                return png_response(bytes.clone());
            }
        }
    }

    let dir = std::path::Path::new("/tmp/lumo-bridge");
    let _ = std::fs::create_dir_all(dir);
    let path = dir.join(format!("{}.png", uuid::Uuid::new_v4()));
    let path_str = path.to_string_lossy().to_string();

    let out = match exec::run("/usr/bin/grim", &["-t", "png", &path_str]).await {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!("grim failed: {}", e);
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                format!("grim error: {}", e),
            )
                .into_response();
        }
    };
    if out.status != 0 {
        let err = String::from_utf8_lossy(&out.stderr).to_string();
        tracing::warn!("grim exit {}: {}", out.status, err);
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("grim exit {}: {}", out.status, err),
        )
            .into_response();
    }

    let data = match std::fs::read(&path) {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("read png: {}", e),
            )
                .into_response();
        }
    };
    let _ = std::fs::remove_file(&path);

    let bytes = Bytes::from(data);
    {
        let mut cache = state.screenshot_cache.lock().await;
        *cache = Some((Instant::now(), bytes.clone()));
    }
    png_response(bytes)
}

fn png_response(bytes: Bytes) -> Response {
    (StatusCode::OK, [(header::CONTENT_TYPE, "image/png")], bytes).into_response()
}
