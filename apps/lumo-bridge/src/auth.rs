//! Bearer token middleware. /healthz e exento; demais rotas exigem header valido.

use axum::{
    extract::{Extension, Request},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};

use crate::AppState;

pub async fn require_bearer(
    Extension(state): Extension<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let Some(token) = auth.strip_prefix("Bearer ") else {
        return Err(StatusCode::UNAUTHORIZED);
    };
    if !constant_time_eq(token.trim().as_bytes(), state.token.as_bytes()) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(next.run(req).await)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::constant_time_eq;

    #[test]
    fn constant_time_eq_matches() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
    }
}
