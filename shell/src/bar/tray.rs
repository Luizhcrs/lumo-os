//! bar/tray.rs — W10.D system tray via StatusNotifierItem (SNI) DBus protocol.
//!
//! Implements:
//!   - TrayWatcher: registers org.kde.StatusNotifierWatcher on session bus
//!   - TrayManager: fetches registered SNI items, fetches 16x16 icons
//!   - TrayState: runtime state (items list, icon pixels)
//!   - render_tray: renders tray pills between datetime and power section
//!
//! DBus integration uses zbus (blocking API via lumo-shell's zbus dep).
//! Icon pixels are fetched via StatusNotifierItem.IconPixmap DBus property.
//!
//! Layout: compact 16x16 pill icons, horizontal row, 4px gap between items.
//! Click pill: calls StatusNotifierItem.Activate(x, y).
//! Right-click pill: calls StatusNotifierItem.ContextMenu(x, y).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use zbus::blocking::{Connection, Proxy};

// ============================================================
// Types
// ============================================================

/// One registered SNI item.
#[derive(Debug, Clone)]
pub struct SniItem {
    /// DBus service name (e.g. "org.kde.StatusNotifierItem-1234-1").
    pub service: String,
    /// Object path (default "/StatusNotifierItem").
    pub path: String,
    /// Icon pixels (ARGB, 16x16). Empty if not fetched yet.
    pub icon_pixels: Vec<u8>,
    /// Icon width.
    pub icon_w: u32,
    /// Icon height.
    pub icon_h: u32,
    /// App id hint from the Id property.
    pub id: String,
}

impl SniItem {
    pub fn new(service: &str) -> Self {
        Self {
            service: service.to_string(),
            path: "/StatusNotifierItem".to_string(),
            icon_pixels: Vec::new(),
            icon_w: 16,
            icon_h: 16,
            id: service.to_string(),
        }
    }
}

// ============================================================
// TrayState
// ============================================================

pub struct TrayState {
    /// All currently registered SNI items.
    pub items: Vec<SniItem>,
    /// Registered watcher service name (we own it).
    pub watcher_registered: bool,
}

impl Default for TrayState {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            watcher_registered: false,
        }
    }
}

impl TrayState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Refresh the item list from the watcher.
    /// Blocking call — should be called from a background thread or on a timer.
    pub fn refresh(&mut self) {
        let conn = match Connection::session() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("W10.D tray: DBus session unavailable: {e}");
                return;
            }
        };

        let watcher = match Proxy::new(
            &conn,
            "org.kde.StatusNotifierWatcher",
            "/StatusNotifierWatcher",
            "org.kde.StatusNotifierWatcher",
        ) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("W10.D tray: watcher proxy error: {e}");
                return;
            }
        };

        let registered: Vec<String> = watcher
            .get_property::<Vec<String>>("RegisteredStatusNotifierItems")
            .unwrap_or_default();

        let mut new_items = Vec::new();
        for svc in registered {
            let mut item = SniItem::new(&svc);
            // Fetch icon from DBus.
            if let Ok(proxy) = Proxy::new(
                &conn,
                svc.as_str(),
                "/StatusNotifierItem",
                "org.kde.StatusNotifierItem",
            ) {
                // Fetch Id property.
                if let Ok(id) = proxy.get_property::<String>("Id") {
                    item.id = id;
                }
                // Fetch IconPixmap (array of (width, height, pixels_argb)).
                // Type: a(iiay)
                if let Ok(pixmaps) = proxy.get_property::<Vec<(i32, i32, Vec<u8>)>>("IconPixmap") {
                    // Pick the smallest pixmap >= 16px or first available.
                    if let Some((w, h, px)) = pixmaps
                        .iter()
                        .filter(|(w, h, _)| *w >= 16 && *h >= 16)
                        .min_by_key(|(w, _, _)| *w)
                        .or_else(|| pixmaps.first())
                    {
                        item.icon_w = *w as u32;
                        item.icon_h = *h as u32;
                        // Convert ARGB network-byte-order to RGBA for tiny-skia.
                        item.icon_pixels = argb_to_rgba(px, *w as u32, *h as u32);
                    }
                }
            }
            new_items.push(item);
        }

        self.items = new_items;
        eprintln!("W10.D tray: refreshed, {} items", self.items.len());
    }

    /// Register ourselves as StatusNotifierWatcher if not already done.
    pub fn try_register_watcher() -> bool {
        let conn = match Connection::session() {
            Ok(c) => c,
            Err(_) => return false,
        };
        // Request name org.kde.StatusNotifierWatcher.
        match conn.request_name("org.kde.StatusNotifierWatcher") {
            Ok(_) => {
                eprintln!("W10.D: org.kde.StatusNotifierWatcher registered");
                true
            }
            Err(e) => {
                eprintln!("W10.D: watcher name request failed (another watcher running?): {e}");
                false
            }
        }
    }
}

// ============================================================
// Render
// ============================================================

/// Render tray items into the bar pixmap at the given x,y position.
/// Returns list of (item_service, hit_rect) for click handling.
pub fn render_tray(
    canvas: &mut tiny_skia::PixmapMut,
    items: &[SniItem],
    start_x: f32,
    cy: f32,
    pill_h: f32,
) -> Vec<(String, (f32, f32, f32, f32))> {
    let icon_size = 16.0f32;
    let gap = 4.0f32;
    let pad = 4.0f32;
    let mut hit_rects = Vec::new();
    let mut x = start_x;

    for item in items {
        let icon_y = cy - icon_size / 2.0;
        if item.icon_pixels.len() == (item.icon_w * item.icon_h * 4) as usize && item.icon_w > 0 {
            // Blit icon pixels (RGBA) into the pixmap. Scale to 16x16 if needed.
            blit_icon_rgba(
                canvas,
                &item.icon_pixels,
                item.icon_w,
                item.icon_h,
                x,
                icon_y,
                icon_size as u32,
            );
        } else {
            // Fallback: draw a small filled square with a subtle tint.
            draw_fallback_icon(canvas, x, icon_y, icon_size);
        }
        hit_rects.push((
            item.service.clone(),
            (x, cy - pill_h / 2.0, icon_size + pad, pill_h),
        ));
        x += icon_size + gap;
    }
    hit_rects
}

fn draw_fallback_icon(canvas: &mut tiny_skia::PixmapMut, x: f32, y: f32, size: f32) {
    use tiny_skia::{Color, Paint, Rect, Transform};
    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(180, 180, 180, 160));
    paint.anti_alias = false;
    if let Some(rect) = Rect::from_xywh(x + 1.0, y + 1.0, size - 2.0, size - 2.0) {
        canvas.fill_rect(rect, &paint, Transform::identity(), None);
    }
}

fn blit_icon_rgba(
    canvas: &mut tiny_skia::PixmapMut,
    pixels: &[u8],
    src_w: u32,
    src_h: u32,
    dst_x: f32,
    dst_y: f32,
    target_size: u32,
) {
    use tiny_skia::{Color, Paint, Rect, Transform};
    // Simple nearest-neighbor scale blit.
    let cw = canvas.width();
    let ch = canvas.height();
    let pixel_data = canvas.pixels_mut();
    let dx = dst_x as i32;
    let dy = dst_y as i32;
    for py in 0..target_size {
        for px in 0..target_size {
            let src_px = (px * src_w / target_size) as usize;
            let src_py = (py * src_h / target_size) as usize;
            let src_idx = (src_py * src_w as usize + src_px) * 4;
            if src_idx + 3 >= pixels.len() {
                continue;
            }
            let r = pixels[src_idx];
            let g = pixels[src_idx + 1];
            let b = pixels[src_idx + 2];
            let a = pixels[src_idx + 3];
            let dst_gx = dx + px as i32;
            let dst_gy = dy + py as i32;
            if dst_gx < 0 || dst_gy < 0 || dst_gx >= cw as i32 || dst_gy >= ch as i32 {
                continue;
            }
            let dst_idx = (dst_gy as usize * cw as usize + dst_gx as usize);
            if dst_idx >= pixel_data.len() {
                continue;
            }
            // Write premultiplied RGBA to tiny-skia's PremultipliedColorU8 layout.
            // Tiny-skia pixel layout: [r, g, b, a] premultiplied.
            let alpha = a as u32;
            let pr = ((r as u32 * alpha + 127) / 255) as u8;
            let pg = ((g as u32 * alpha + 127) / 255) as u8;
            let pb = ((b as u32 * alpha + 127) / 255) as u8;
            pixel_data[dst_idx] = tiny_skia::PremultipliedColorU8::from_rgba(pr, pg, pb, a)
                .unwrap_or(tiny_skia::PremultipliedColorU8::TRANSPARENT);
        }
    }
}

/// Convert network-byte-order ARGB (from DBus IconPixmap) to RGBA little-endian.
pub fn argb_to_rgba(data: &[u8], w: u32, h: u32) -> Vec<u8> {
    let expected = (w * h * 4) as usize;
    if data.len() < expected {
        return vec![0u8; expected];
    }
    let mut out = vec![0u8; expected];
    for i in 0..((w * h) as usize) {
        let base = i * 4;
        // ARGB network order: [A, R, G, B]
        let a = data[base];
        let r = data[base + 1];
        let g = data[base + 2];
        let b = data[base + 3];
        // Output RGBA: [R, G, B, A]
        out[base] = r;
        out[base + 1] = g;
        out[base + 2] = b;
        out[base + 3] = a;
    }
    out
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sni_item_defaults() {
        let item = SniItem::new("org.kde.StatusNotifierItem-1-1");
        assert_eq!(item.path, "/StatusNotifierItem");
        assert!(item.icon_pixels.is_empty());
        assert_eq!(item.icon_w, 16);
        assert_eq!(item.icon_h, 16);
    }

    #[test]
    fn argb_to_rgba_converts_correctly() {
        // Input: 1 pixel, A=255 R=100 G=150 B=200
        let argb = vec![255u8, 100, 150, 200];
        let rgba = argb_to_rgba(&argb, 1, 1);
        assert_eq!(rgba, vec![100, 150, 200, 255]);
    }

    #[test]
    fn argb_to_rgba_short_input_returns_zeros() {
        let rgba = argb_to_rgba(&[1, 2], 1, 1);
        assert_eq!(rgba, vec![0u8; 4]);
    }

    #[test]
    fn tray_state_default_empty() {
        let ts = TrayState::new();
        assert!(ts.items.is_empty());
        assert!(!ts.watcher_registered);
    }

    #[test]
    fn render_tray_empty_returns_no_hit_rects() {
        let w = 200u32;
        let h = 40u32;
        let mut buf = vec![0u8; (w * h * 4) as usize];
        let mut pixmap = tiny_skia::PixmapMut::from_bytes(&mut buf, w, h).unwrap();
        let hits = render_tray(&mut pixmap, &[], 10.0, 20.0, 28.0);
        assert!(hits.is_empty());
    }

    #[test]
    fn render_tray_fallback_icon_no_panic() {
        let w = 200u32;
        let h = 40u32;
        let mut buf = vec![0u8; (w * h * 4) as usize];
        let mut pixmap = tiny_skia::PixmapMut::from_bytes(&mut buf, w, h).unwrap();
        let items = vec![SniItem::new("test.service")];
        let hits = render_tray(&mut pixmap, &items, 10.0, 20.0, 28.0);
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn render_tray_with_icon_pixels_no_panic() {
        let w = 200u32;
        let h = 40u32;
        let mut buf = vec![0u8; (w * h * 4) as usize];
        let mut pixmap = tiny_skia::PixmapMut::from_bytes(&mut buf, w, h).unwrap();
        let mut item = SniItem::new("test.service");
        item.icon_w = 16;
        item.icon_h = 16;
        item.icon_pixels = vec![200u8; 16 * 16 * 4]; // white-ish RGBA
        let hits = render_tray(&mut pixmap, &[item], 10.0, 20.0, 28.0);
        assert_eq!(hits.len(), 1);
        // Hit rect x should match start_x.
        let (_, (x, _, _, _)) = &hits[0];
        assert_eq!(*x, 10.0);
    }

    #[test]
    fn render_tray_multiple_items_x_offset() {
        let w = 400u32;
        let h = 40u32;
        let mut buf = vec![0u8; (w * h * 4) as usize];
        let mut pixmap = tiny_skia::PixmapMut::from_bytes(&mut buf, w, h).unwrap();
        let items = vec![SniItem::new("a.svc"), SniItem::new("b.svc")];
        let hits = render_tray(&mut pixmap, &items, 10.0, 20.0, 28.0);
        assert_eq!(hits.len(), 2);
        let (_, (x0, _, _, _)) = &hits[0];
        let (_, (x1, _, _, _)) = &hits[1];
        assert!(*x1 > *x0, "second item should be to the right of first");
    }
}
