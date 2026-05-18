//! desktop/state.rs - LumoDesktop struct + MenuActive + fonts + draw helpers
//! + IPC.
//!
//! Tudo que o lumo-desktop precisa rodar fora do main loop. Render do menu
//! delegado pra `menu_overlay`.

use std::io::{ErrorKind, Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use cosmic_text::{
    Attrs, Buffer as CosmicBuffer, Color as CosmicColor, Family, FontSystem, Metrics, Shaping,
    SwashCache,
};
use smithay_client_toolkit::{
    output::OutputState,
    registry::RegistryState,
    seat::{pointer::ThemedPointer, SeatState},
    shell::wlr_layer::LayerSurface,
    shm::{slot::SlotPool, Shm},
};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, PixmapMut, Transform};

use lumo_foundation::LumoColors;
use lumo_ipc::{default_socket_path, LumoCommand, LumoEvent};

// ============================================================
// Layout constants A27 (menu redesign Apple-style).
// ============================================================

/// Output Galaxy nativo (DEPS.md A19.18 mesmo padrao bar).
pub const OUTPUT_W: u32 = 1920;
pub const OUTPUT_H: u32 = 1080;

/// Margem entre cursor e canto do menu. 2px = grude no cursor sem encavalar.
pub const MENU_OFFSET: f32 = 2.0;

// ============================================================
// FontSystem singleton (alinhado com lumo-bar: Geist/JetBrains).
// ============================================================

static FONT_SYSTEM: OnceLock<Mutex<FontSystem>> = OnceLock::new();
static SWASH_CACHE: OnceLock<Mutex<SwashCache>> = OnceLock::new();
static FONT_FAMILY: OnceLock<String> = OnceLock::new();

pub fn font_system() -> &'static Mutex<FontSystem> {
    FONT_SYSTEM.get_or_init(|| {
        let mut fs = FontSystem::new();
        load_extra_fonts(&mut fs);
        let family = pick_font_family(&fs);
        eprintln!("[lumo-desktop] font_family escolhida = {}", family);
        let _ = FONT_FAMILY.set(family);
        Mutex::new(fs)
    })
}

pub fn swash_cache() -> &'static Mutex<SwashCache> {
    SWASH_CACHE.get_or_init(|| Mutex::new(SwashCache::new()))
}

fn load_extra_fonts(fs: &mut FontSystem) {
    let candidates = [
        std::env::var("HOME").ok().map(|h| format!("{}/.local/share/fonts", h)),
        std::env::var("HOME").ok().map(|h| format!("{}/.fonts", h)),
        Some("/usr/share/fonts/geist-mono".to_string()),
        Some("/usr/local/share/fonts".to_string()),
    ];
    for opt in candidates.iter().flatten() {
        walk_load(fs, std::path::Path::new(opt));
    }
}

fn walk_load(fs: &mut FontSystem, dir: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            walk_load(fs, &p);
            continue;
        }
        let ext_ok = p
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| {
                let l = e.to_ascii_lowercase();
                l == "ttf" || l == "otf"
            })
            .unwrap_or(false);
        if !ext_ok {
            continue;
        }
        let name = p.to_string_lossy().to_lowercase();
        if name.contains("geist") || name.contains("jetbrains") || name.contains("inter") {
            fs.db_mut().load_font_file(&p).ok();
        }
    }
}

fn pick_font_family(fs: &FontSystem) -> String {
    // A29: desktop renderiza SO menus (UI). Geist Sans first.
    let preferred = [
        "Geist",
        "Inter",
        "JetBrainsMono Nerd Font",
        "sans-serif",
    ];
    let faces: Vec<String> = fs
        .db()
        .faces()
        .flat_map(|f| f.families.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>())
        .collect();
    for p in preferred {
        if faces.iter().any(|f| f.eq_ignore_ascii_case(p)) {
            return p.to_string();
        }
    }
    for p in preferred {
        let pl = p.to_lowercase();
        let token = pl.split_whitespace().next().unwrap_or("sans-serif");
        if let Some(found) = faces.iter().find(|f| f.to_lowercase().contains(token)) {
            return found.clone();
        }
    }
    "sans-serif".to_string()
}

pub fn current_family() -> &'static str {
    FONT_FAMILY.get().map(|s| s.as_str()).unwrap_or("sans-serif")
}

// ============================================================
// Color helpers.
// ============================================================

fn to_cosmic(c: Color) -> CosmicColor {
    let r = (c.red() * 255.0).round().clamp(0.0, 255.0) as u8;
    let g = (c.green() * 255.0).round().clamp(0.0, 255.0) as u8;
    let b = (c.blue() * 255.0).round().clamp(0.0, 255.0) as u8;
    let a = (c.alpha() * 255.0).round().clamp(0.0, 255.0) as u8;
    CosmicColor::rgba(r, g, b, a)
}

// ============================================================
// Drawing helpers.
// ============================================================

fn rrect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    let r = r.min(w / 2.0).min(h / 2.0);
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    pb.finish()
}

pub fn fill_rrect(canvas: &mut PixmapMut, x: f32, y: f32, w: f32, h: f32, r: f32, color: Color) {
    if let Some(path) = rrect_path(x, y, w, h, r) {
        let mut p = Paint::default();
        p.set_color(color);
        p.anti_alias = true;
        canvas.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
    }
}

pub fn draw_text(
    canvas: &mut PixmapMut,
    x: f32,
    y: f32,
    text: &str,
    size: f32,
    color: Color,
) {
    let fs_mutex = font_system();
    let sc_mutex = swash_cache();
    let mut fs = fs_mutex.lock().unwrap();
    let mut sc = sc_mutex.lock().unwrap();
    let metrics = Metrics::new(size, size * 1.4);
    let mut buffer = CosmicBuffer::new(&mut fs, metrics);
    let family_name = current_family().to_string();
    let attrs = Attrs::new().family(Family::Name(&family_name));
    buffer.set_size(&mut fs, Some(f32::INFINITY), Some(size * 1.4));
    buffer.set_text(&mut fs, text, attrs, Shaping::Advanced);
    buffer.shape_until_scroll(&mut fs, false);
    let cosmic_color = to_cosmic(color);
    let cw = canvas.width() as i32;
    let ch = canvas.height() as i32;
    buffer.draw(&mut fs, &mut sc, cosmic_color, |gx, gy, gw, gh, c| {
        let alpha = c.a();
        if alpha == 0 {
            return;
        }
        let fx = (x as i32) + gx;
        let fy = (y as i32) + gy;
        for dy in 0..gh as i32 {
            for dx in 0..gw as i32 {
                let px = fx + dx;
                let py = fy + dy;
                if px < 0 || py < 0 || px >= cw || py >= ch {
                    continue;
                }
                let idx = ((py as u32 * canvas.width() + px as u32) * 4) as usize;
                let data = canvas.data_mut();
                if idx + 3 >= data.len() {
                    continue;
                }
                let a = (alpha as f32) / 255.0;
                let r = (color.red() * 255.0 * a) as u8;
                let g = (color.green() * 255.0 * a) as u8;
                let b = (color.blue() * 255.0 * a) as u8;
                let aa = (a * 255.0) as u8;
                let inv = 1.0 - a;
                data[idx] = r.saturating_add((data[idx] as f32 * inv) as u8);
                data[idx + 1] = g.saturating_add((data[idx + 1] as f32 * inv) as u8);
                data[idx + 2] = b.saturating_add((data[idx + 2] as f32 * inv) as u8);
                data[idx + 3] = aa.saturating_add((data[idx + 3] as f32 * inv) as u8);
            }
        }
    });
}

// ============================================================
// IPC.
// ============================================================

pub fn connect_ipc() -> Option<UnixStream> {
    let path = default_socket_path()?;
    match UnixStream::connect(&path) {
        Ok(s) => {
            s.set_nonblocking(true).ok()?;
            eprintln!("[lumo-desktop] IPC conectado em {}", path.display());
            Some(s)
        }
        Err(e) => {
            eprintln!("[lumo-desktop] IPC nao conectou ({}): area de trabalho passiva", e);
            None
        }
    }
}

pub fn send_close_dropdowns(stream: &mut Option<UnixStream>) {
    let Some(s) = stream.as_mut() else { return };
    let mut payload = match serde_json::to_string(&LumoCommand::CloseDropdowns) {
        Ok(s) => s,
        Err(_) => return,
    };
    payload.push('\n');
    if let Err(e) = s.write_all(payload.as_bytes()) {
        if e.kind() != ErrorKind::WouldBlock {
            eprintln!("[lumo-desktop] IPC write erro: {}; dropando socket", e);
            *stream = None;
        }
    }
}

/// A26: drena eventos do compositor. Retorna (alive, close_menu_requested).
pub fn drain_ipc_events(stream: &mut UnixStream, rx_buf: &mut Vec<u8>) -> (bool, bool) {
    let mut tmp = [0u8; 256];
    let mut alive = true;
    let mut close_menu = false;
    loop {
        match stream.read(&mut tmp) {
            Ok(0) => { alive = false; break; }
            Ok(n) => rx_buf.extend_from_slice(&tmp[..n]),
            Err(e) if e.kind() == ErrorKind::WouldBlock => break,
            Err(_) => { alive = false; break; }
        }
    }
    while let Some(nl) = rx_buf.iter().position(|b| *b == b'\n') {
        let line: Vec<u8> = rx_buf.drain(..=nl).collect();
        if let Ok(s) = std::str::from_utf8(&line[..line.len() - 1]) {
            if let Ok(ev) = serde_json::from_str::<LumoEvent>(s.trim()) {
                if matches!(ev, LumoEvent::CloseDesktopMenu) {
                    close_menu = true;
                }
            }
        }
    }
    (alive, close_menu)
}

// ============================================================
// LumoDesktop state.
// ============================================================

#[derive(Debug, Clone, Copy)]
pub struct MenuActive {
    pub visible: bool,
    pub x: f32,
    pub y: f32,
    /// Indice do item Action/Toggle em hover. `usize::MAX` quando nenhum
    /// (sentinel; `menu::draw_menu` trata fora-de-range como sem hover).
    pub hover_idx: usize,
}

pub(crate) struct LumoDesktop {
    pub registry: RegistryState,
    pub output_state: OutputState,
    pub shm: Shm,
    pub seat_state: SeatState,
    pub layer: LayerSurface,
    pub pool: SlotPool,
    pub width: u32,
    pub height: u32,
    pub running: bool,
    pub first_configured: bool,
    pub pointer: Option<ThemedPointer>,
    pub pointer_pos: Option<(f64, f64)>,
    pub menu: MenuActive,
    pub ipc_stream: Option<UnixStream>,
    pub ipc_rx_buf: Vec<u8>,
    pub last_click_at: Option<Instant>,
    pub palette: LumoColors,
    /// A26: flag setado por drain_ipc_events quando compositor pede pra
    /// fechar menu (mutex bar dropdown vs desktop menu).
    pub need_redraw: bool,
}
