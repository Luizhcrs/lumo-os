//! GET /log/tail?path=...&n=50 -- tail allowlisted log files.

use axum::{
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;

const ALLOWED: &[&str] = &["/tmp/lumo-wm-tty.log", "/tmp/lumo-bar.log", "/tmp/lumo-bridge.log"];

#[derive(Deserialize)]
pub struct TailParams {
    pub path: String,
    #[serde(default = "default_n")]
    pub n: usize,
}

fn default_n() -> usize {
    50
}

pub async fn tail(Query(p): Query<TailParams>) -> Response {
    if !ALLOWED.contains(&p.path.as_str()) {
        return (StatusCode::FORBIDDEN, "path not in allowlist").into_response();
    }
    let n = p.n.min(2000);
    let content = match std::fs::read_to_string(&p.path) {
        Ok(s) => s,
        Err(e) => {
            return (StatusCode::NOT_FOUND, format!("read {}: {}", p.path, e)).into_response();
        }
    };
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(n);
    let tail = lines[start..].join("\n");
    (StatusCode::OK, [("content-type", "text/plain; charset=utf-8")], tail).into_response()
}
