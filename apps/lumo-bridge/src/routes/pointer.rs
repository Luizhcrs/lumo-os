//! Pointer routes: click, move, drag, scroll.
//!
//! SI.1: pivot ydotool -> IPC sintetico. As routes enviam LumoCommand
//! direto pro compositor via socket unix.
//!
//! Codigos de botao seguem linux/input-event-codes.h:
//!   BTN_LEFT   = 0x110
//!   BTN_RIGHT  = 0x111
//!   BTN_MIDDLE = 0x112

use axum::{http::StatusCode, response::Json};
use lumo_ipc::LumoCommand;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::exec;
use crate::lumo_ipc::{send_command_async, ydotool_fallback_enabled};

#[derive(Deserialize)]
pub struct Click {
    pub x: f64,
    pub y: f64,
    #[serde(default = "default_button")]
    pub button: String,
}

#[derive(Deserialize)]
pub struct PointerMove {
    pub x: f64,
    pub y: f64,
}

#[derive(Deserialize)]
pub struct Drag {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    #[serde(default = "default_button")]
    pub button: String,
}

#[derive(Deserialize)]
pub struct Scroll {
    #[serde(default)]
    pub dx: f64,
    #[serde(default)]
    pub dy: f64,
}

fn default_button() -> String {
    "left".into()
}

/// Mapeia label -> codigo BTN_* (linux/input-event-codes.h).
pub fn button_code(b: &str) -> Option<u32> {
    match b.to_ascii_lowercase().as_str() {
        "left" => Some(0x110),
        "right" => Some(0x111),
        "middle" => Some(0x112),
        _ => None,
    }
}

/// Envia LumoCommand via IPC. Em caso de erro, se LUMO_BRIDGE_FALLBACK_YDOTOOL=1,
/// invoca `fallback` (ydotool-based). Caso contrario, propaga 503.
async fn ipc_or_fallback<F, Fut>(cmd: LumoCommand, fallback: F) -> Result<(), (StatusCode, String)>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<(), String>>,
{
    match send_command_async(cmd).await {
        Ok(()) => Ok(()),
        Err(ipc_err) => {
            if ydotool_fallback_enabled() {
                tracing::warn!(err = %ipc_err, "SI.1: IPC falhou, tentando ydotool fallback");
                fallback().await.map_err(|e| {
                    (
                        StatusCode::SERVICE_UNAVAILABLE,
                        format!("ipc={ipc_err}; fallback={e}"),
                    )
                })
            } else {
                Err((StatusCode::SERVICE_UNAVAILABLE, ipc_err.to_string()))
            }
        }
    }
}

async fn ydotool_mousemove_abs(x: f64, y: f64) -> Result<(), String> {
    let x_s = (x as i64).to_string();
    let y_s = (y as i64).to_string();
    let out = exec::run(
        "/usr/bin/ydotool",
        &["mousemove", "-a", "-x", &x_s, "-y", &y_s],
    )
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

async fn ydotool_click_label(label: &str) -> Result<(), String> {
    let base = match label {
        "left" => 0x00u8,
        "right" => 0x01,
        "middle" => 0x02,
        _ => return Err(format!("invalid button: {label}")),
    };
    let code = format!("0x{:02X}", base | 0xC0);
    let out = exec::run("/usr/bin/ydotool", &["click", &code])
        .await
        .map_err(|e| e.to_string())?;
    if out.status != 0 {
        return Err(format!(
            "ydotool click exit {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

pub async fn click(Json(p): Json<Click>) -> Result<Json<Value>, (StatusCode, String)> {
    let btn = button_code(&p.button).ok_or((
        StatusCode::BAD_REQUEST,
        format!("invalid button: {}", p.button),
    ))?;
    // 1. move
    ipc_or_fallback(LumoCommand::SyntheticPointerMove { x: p.x, y: p.y }, || {
        ydotool_mousemove_abs(p.x, p.y)
    })
    .await?;
    // 2. press
    ipc_or_fallback(
        LumoCommand::SyntheticPointerButton {
            button: btn,
            pressed: true,
        },
        || async { Ok(()) },
    )
    .await?;
    // 3. release
    let label = p.button.clone();
    ipc_or_fallback(
        LumoCommand::SyntheticPointerButton {
            button: btn,
            pressed: false,
        },
        || ydotool_click_label(&label),
    )
    .await?;
    Ok(Json(
        json!({"ok": true, "x": p.x, "y": p.y, "button": p.button}),
    ))
}

pub async fn pointer_move(Json(p): Json<PointerMove>) -> Result<Json<Value>, (StatusCode, String)> {
    ipc_or_fallback(LumoCommand::SyntheticPointerMove { x: p.x, y: p.y }, || {
        ydotool_mousemove_abs(p.x, p.y)
    })
    .await?;
    Ok(Json(json!({"ok": true, "x": p.x, "y": p.y})))
}

pub async fn drag(Json(p): Json<Drag>) -> Result<Json<Value>, (StatusCode, String)> {
    let btn = button_code(&p.button).ok_or((
        StatusCode::BAD_REQUEST,
        format!("invalid button: {}", p.button),
    ))?;
    ipc_or_fallback(
        LumoCommand::SyntheticPointerMove { x: p.x1, y: p.y1 },
        || ydotool_mousemove_abs(p.x1, p.y1),
    )
    .await?;
    ipc_or_fallback(
        LumoCommand::SyntheticPointerButton {
            button: btn,
            pressed: true,
        },
        || async { Ok(()) },
    )
    .await?;
    ipc_or_fallback(
        LumoCommand::SyntheticPointerMove { x: p.x2, y: p.y2 },
        || ydotool_mousemove_abs(p.x2, p.y2),
    )
    .await?;
    ipc_or_fallback(
        LumoCommand::SyntheticPointerButton {
            button: btn,
            pressed: false,
        },
        || async { Ok(()) },
    )
    .await?;
    Ok(Json(
        json!({"ok": true, "from": [p.x1, p.y1], "to": [p.x2, p.y2], "button": p.button}),
    ))
}

pub async fn scroll(Json(p): Json<Scroll>) -> Result<Json<Value>, (StatusCode, String)> {
    if p.dx == 0.0 && p.dy == 0.0 {
        return Ok(Json(json!({"ok": true, "dx": 0.0, "dy": 0.0})));
    }
    let (dx, dy) = (p.dx, p.dy);
    ipc_or_fallback(
        LumoCommand::SyntheticPointerScroll { dx, dy },
        move || async move {
            let dx_s = (dx as i64).to_string();
            let dy_s = (dy as i64).to_string();
            let out = exec::run(
                "/usr/bin/ydotool",
                &["mousemove", "--wheel", "-x", &dx_s, "-y", &dy_s],
            )
            .await
            .map_err(|e| e.to_string())?;
            if out.status != 0 {
                return Err(format!(
                    "scroll exit {}: {}",
                    out.status,
                    String::from_utf8_lossy(&out.stderr)
                ));
            }
            Ok(())
        },
    )
    .await?;
    Ok(Json(json!({"ok": true, "dx": p.dx, "dy": p.dy})))
}

#[cfg(test)]
mod tests {
    use super::button_code;

    #[test]
    fn button_code_maps_btn_left_right_middle() {
        assert_eq!(button_code("left"), Some(0x110));
        assert_eq!(button_code("RIGHT"), Some(0x111));
        assert_eq!(button_code("middle"), Some(0x112));
        assert_eq!(button_code("garbage"), None);
    }
}
