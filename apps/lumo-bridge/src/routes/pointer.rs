//! Pointer routes: click, move, drag, scroll. Tudo via ydotool com coords absolutas.

use axum::{http::StatusCode, response::Json};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::exec;

#[derive(Deserialize)]
pub struct Click {
    pub x: i32,
    pub y: i32,
    #[serde(default = "default_button")]
    pub button: String,
}

#[derive(Deserialize)]
pub struct PointerMove {
    pub x: i32,
    pub y: i32,
}

#[derive(Deserialize)]
pub struct Drag {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
    #[serde(default = "default_button")]
    pub button: String,
}

#[derive(Deserialize)]
pub struct Scroll {
    #[serde(default)]
    pub dx: i32,
    #[serde(default)]
    pub dy: i32,
}

fn default_button() -> String {
    "left".into()
}

/// Mapeia label -> codigo base. 0x40=down, 0x80=up, 0xC0=click (down+up).
fn button_base(b: &str) -> Option<u8> {
    match b.to_ascii_lowercase().as_str() {
        "left" => Some(0x00),
        "right" => Some(0x01),
        "middle" => Some(0x02),
        _ => None,
    }
}

async fn ydotool_mousemove_abs(x: i32, y: i32) -> Result<(), String> {
    let x_s = x.to_string();
    let y_s = y.to_string();
    let out = exec::run("/usr/bin/ydotool", &["mousemove", "-a", "-x", &x_s, "-y", &y_s])
        .await
        .map_err(|e| e.to_string())?;
    if out.status != 0 {
        return Err(format!(
            "ydotool mousemove exit {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

async fn ydotool_click(base: u8) -> Result<(), String> {
    let code = format!("0x{:02X}", base | 0xC0);
    let out =
        exec::run("/usr/bin/ydotool", &["click", &code]).await.map_err(|e| e.to_string())?;
    if out.status != 0 {
        return Err(format!(
            "ydotool click exit {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

async fn ydotool_button(base: u8, down: bool) -> Result<(), String> {
    let mask = if down { 0x40 } else { 0x80 };
    let code = format!("0x{:02X}", base | mask);
    let out =
        exec::run("/usr/bin/ydotool", &["click", &code]).await.map_err(|e| e.to_string())?;
    if out.status != 0 {
        return Err(format!(
            "ydotool click {} exit {}: {}",
            code,
            out.status,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

pub async fn click(Json(p): Json<Click>) -> Result<Json<Value>, (StatusCode, String)> {
    let base = button_base(&p.button)
        .ok_or((StatusCode::BAD_REQUEST, format!("invalid button: {}", p.button)))?;
    ydotool_mousemove_abs(p.x, p.y)
        .await
        .map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, e))?;
    ydotool_click(base).await.map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, e))?;
    Ok(Json(json!({"ok": true, "x": p.x, "y": p.y, "button": p.button})))
}

pub async fn pointer_move(Json(p): Json<PointerMove>) -> Result<Json<Value>, (StatusCode, String)> {
    ydotool_mousemove_abs(p.x, p.y)
        .await
        .map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, e))?;
    Ok(Json(json!({"ok": true, "x": p.x, "y": p.y})))
}

pub async fn drag(Json(p): Json<Drag>) -> Result<Json<Value>, (StatusCode, String)> {
    let base = button_base(&p.button)
        .ok_or((StatusCode::BAD_REQUEST, format!("invalid button: {}", p.button)))?;
    ydotool_mousemove_abs(p.x1, p.y1).await.map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, e))?;
    ydotool_button(base, true).await.map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, e))?;
    ydotool_mousemove_abs(p.x2, p.y2).await.map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, e))?;
    ydotool_button(base, false).await.map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, e))?;
    Ok(Json(json!({"ok": true, "from": [p.x1, p.y1], "to": [p.x2, p.y2], "button": p.button})))
}

pub async fn scroll(Json(p): Json<Scroll>) -> Result<Json<Value>, (StatusCode, String)> {
    // ydotool mousemove --wheel -x dx -y dy (vertical = y, horizontal = x)
    if p.dx == 0 && p.dy == 0 {
        return Ok(Json(json!({"ok": true, "dx": 0, "dy": 0})));
    }
    let dx = p.dx.to_string();
    let dy = p.dy.to_string();
    let out = exec::run("/usr/bin/ydotool", &["mousemove", "--wheel", "-x", &dx, "-y", &dy])
        .await
        .map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, e.to_string()))?;
    if out.status != 0 {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            format!("scroll exit {}: {}", out.status, String::from_utf8_lossy(&out.stderr)),
        ));
    }
    Ok(Json(json!({"ok": true, "dx": p.dx, "dy": p.dy})))
}

#[cfg(test)]
mod tests {
    use super::button_base;

    #[test]
    fn button_base_maps() {
        assert_eq!(button_base("left"), Some(0x00));
        assert_eq!(button_base("RIGHT"), Some(0x01));
        assert_eq!(button_base("middle"), Some(0x02));
        assert_eq!(button_base("garbage"), None);
    }
}
