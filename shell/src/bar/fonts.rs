//! bar/fonts.rs - FontSystem singleton + descoberta de familias + render texto.
//!
//! A29: 2 familias. UI = Geist Sans (menus, labels, dropdowns). MONO = Geist
//! Mono (clock HH:MM, workspace numero, valores calendario tabular).
//! Cosmic-text 0.12 + tiny-skia. Glyphs grayscale AA (sem rainbow subpixel
//! artifact em panel TN 6-bit + FRC do Galaxy Book - ver DEPS.md).

use cosmic_text::{
    Attrs, Buffer as CosmicBuffer, Color as CosmicColor, Family, FontSystem, Metrics, Shaping,
    SwashCache,
};
use std::sync::{Mutex, OnceLock};
use tiny_skia::{Color, Paint, PixmapMut, Rect, Transform};

// ============================================================
// Color helpers.
// ============================================================

pub fn rgba_hex(hex: u32, alpha: u8) -> Color {
    let r = ((hex >> 16) & 0xff) as f32 / 255.0;
    let g = ((hex >> 8) & 0xff) as f32 / 255.0;
    let b = (hex & 0xff) as f32 / 255.0;
    let a = alpha as f32 / 255.0;
    Color::from_rgba(r, g, b, a).expect("r,g,b,a derivados de u8: sempre em [0,1]")
}

pub fn opaque(hex: u32) -> Color {
    rgba_hex(hex, 0xff)
}

/// tiny-skia Color -> cosmic-text Color (RGBA, sem premul).
fn to_cosmic(c: Color) -> CosmicColor {
    let r = (c.red() * 255.0).round().clamp(0.0, 255.0) as u8;
    let g = (c.green() * 255.0).round().clamp(0.0, 255.0) as u8;
    let b = (c.blue() * 255.0).round().clamp(0.0, 255.0) as u8;
    let a = (c.alpha() * 255.0).round().clamp(0.0, 255.0) as u8;
    CosmicColor::rgba(r, g, b, a)
}

// ============================================================
// FontSystem singleton + SwashCache.
// ============================================================

static FONT_SYSTEM: OnceLock<Mutex<FontSystem>> = OnceLock::new();
static SWASH_CACHE: OnceLock<Mutex<SwashCache>> = OnceLock::new();
static FONT_FAMILY_UI: OnceLock<String> = OnceLock::new();
static FONT_FAMILY_MONO: OnceLock<String> = OnceLock::new();

pub fn font_system() -> &'static Mutex<FontSystem> {
    FONT_SYSTEM.get_or_init(|| {
        let mut fs = FontSystem::new();
        load_extra_fonts(&mut fs);
        // R4: ler tokens do theme.toml pra respeitar font_sans/font_mono configurados.
        let tokens = lumo_foundation::LumoTokens::load_from_disk();
        let sans_override = tokens.font_sans.clone();
        let mono_override = tokens.font_mono.clone();
        let ui = pick_font_family(&fs, false, sans_override.as_deref());
        let mono = pick_font_family(&fs, true, mono_override.as_deref());
        eprintln!("[lumo-bar] font_family UI = {} | MONO = {}", ui, mono);
        let _ = FONT_FAMILY_UI.set(ui);
        let _ = FONT_FAMILY_MONO.set(mono);
        Mutex::new(fs)
    })
}

pub fn swash_cache() -> &'static Mutex<SwashCache> {
    SWASH_CACHE.get_or_init(|| Mutex::new(SwashCache::new()))
}

fn load_extra_fonts(fs: &mut FontSystem) {
    let candidates = [
        std::env::var("HOME")
            .ok()
            .map(|h| format!("{}/.local/share/fonts", h)),
        std::env::var("HOME").ok().map(|h| format!("{}/.fonts", h)),
        Some("/usr/share/fonts/geist-mono".to_string()),
        // R4: Inter (ttf-inter pacman) e outros paths comuns.
        Some("/usr/share/fonts/inter".to_string()),
        Some("/usr/share/fonts/TTF".to_string()),
        Some("/usr/share/fonts/OTF".to_string()),
        Some("/usr/local/share/fonts".to_string()),
    ];
    for opt in candidates.iter().flatten() {
        walk_load(fs, std::path::Path::new(opt));
    }
}

fn walk_load(fs: &mut FontSystem, dir: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
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
        // R4: incluir "inter" e "noto" na lista de fontes carregadas.
        if name.contains("geist")
            || name.contains("jetbrains")
            || name.contains("inter")
            || name.contains("noto")
        {
            fs.db_mut().load_font_file(&p).ok();
        }
    }
}

/// R4: escolhe familia com base no perfil.
/// `override_name` = familia explicitamente configurada no theme.toml.
/// Se presente, tenta essa primeiro (exact + fuzzy). Fallback: lista padrao.
fn pick_font_family(fs: &FontSystem, prefer_mono: bool, override_name: Option<&str>) -> String {
    let faces: Vec<String> = fs
        .db()
        .faces()
        .flat_map(|f| {
            f.families
                .iter()
                .map(|(n, _)| n.clone())
                .collect::<Vec<_>>()
        })
        .collect();

    // Tenta override configurado primeiro.
    if let Some(ov) = override_name {
        if faces.iter().any(|f| f.eq_ignore_ascii_case(ov)) {
            eprintln!("[lumo-bar] R4 font override aceito: {}", ov);
            return ov.to_string();
        }
        // Fuzzy: primeiro token do nome.
        let tok = ov.to_lowercase();
        let tok = tok.split_whitespace().next().unwrap_or(ov);
        if let Some(found) = faces.iter().find(|f| f.to_lowercase().contains(tok)) {
            eprintln!("[lumo-bar] R4 font override fuzzy: {} -> {}", ov, found);
            return found.clone();
        }
        eprintln!(
            "[lumo-bar] R4 font override {:?} nao encontrada; usando lista padrao",
            ov
        );
    }

    // R4: lista padrao com Inter em primeiro pra sans.
    let preferred: &[&str] = if prefer_mono {
        &[
            "Geist Mono",
            "GeistMono Nerd Font",
            "JetBrainsMono Nerd Font Mono",
            "JetBrainsMono Nerd Font",
            "JetBrains Mono",
        ]
    } else {
        &["Inter", "Geist", "Noto Sans", "sans-serif"]
    };
    for p in preferred {
        if faces.iter().any(|f| f.eq_ignore_ascii_case(p)) {
            return (*p).to_string();
        }
    }
    for p in preferred {
        let pl = p.to_lowercase();
        let token = pl.split_whitespace().next().unwrap_or("monospace");
        if let Some(found) = faces.iter().find(|f| f.to_lowercase().contains(token)) {
            return found.clone();
        }
    }
    if prefer_mono {
        eprintln!("[lumo-bar] warning: fonte mono nao encontrada; fallback monospace");
        "monospace".to_string()
    } else {
        eprintln!("[lumo-bar] warning: Inter/Geist nao encontrada; fallback sans-serif");
        "sans-serif".to_string()
    }
}

fn current_family_ui() -> &'static str {
    FONT_FAMILY_UI
        .get()
        .map(|s| s.as_str())
        .unwrap_or("sans-serif")
}

fn current_family_mono() -> &'static str {
    FONT_FAMILY_MONO
        .get()
        .map(|s| s.as_str())
        .unwrap_or("monospace")
}

/// A29: helper pra escolher familia conforme contexto. `mono=true` -> Geist Mono.
fn family_for(mono: bool) -> &'static str {
    if mono {
        current_family_mono()
    } else {
        current_family_ui()
    }
}

// ============================================================
// Text rendering.
// ============================================================

/// A29: medida com escolha de familia. `mono=true` -> Geist Mono.
pub fn measure_text_ex(text: &str, size: f32, bold: bool, mono: bool) -> f32 {
    let mut fs = font_system().lock().expect("font_system poisoned");
    let metrics = Metrics::new(size, size * 1.2);
    let mut buffer = CosmicBuffer::new(&mut fs, metrics);
    let family = family_for(mono).to_string();
    let mut attrs = Attrs::new().family(Family::Name(&family));
    if bold {
        attrs = attrs.weight(cosmic_text::Weight::BOLD);
    }
    buffer.set_text(&mut fs, text, attrs, Shaping::Advanced);
    buffer.set_size(&mut fs, Some(f32::INFINITY), Some(size * 1.4));
    buffer.shape_until_scroll(&mut fs, false);

    let mut w = 0.0f32;
    for run in buffer.layout_runs() {
        for glyph in run.glyphs.iter() {
            let r = glyph.x + glyph.w;
            if r > w {
                w = r;
            }
        }
    }
    w.ceil()
}

#[inline]
pub fn measure_text(text: &str, size: f32, bold: bool) -> f32 {
    measure_text_ex(text, size, bold, false)
}

#[inline]
pub fn measure_text_mono(text: &str, size: f32, bold: bool) -> f32 {
    measure_text_ex(text, size, bold, true)
}

/// A29: render com escolha de familia. `mono=true` -> Geist Mono.
pub fn draw_text_ex(
    canvas: &mut PixmapMut,
    x: f32,
    y_top: f32,
    text: &str,
    size: f32,
    color: Color,
    bold: bool,
    mono: bool,
) -> f32 {
    let mut fs = font_system().lock().expect("font_system poisoned");
    let mut cache = swash_cache().lock().expect("swash_cache poisoned");
    let metrics = Metrics::new(size, size * 1.2);
    let mut buffer = CosmicBuffer::new(&mut fs, metrics);
    let family = family_for(mono).to_string();
    let mut attrs = Attrs::new().family(Family::Name(&family));
    if bold {
        // A29: bold UI = 600 (SemiBold) padrao tipografia premium; mono usa BOLD natural.
        attrs = attrs.weight(if mono {
            cosmic_text::Weight::BOLD
        } else {
            cosmic_text::Weight::SEMIBOLD
        });
    } else {
        attrs = attrs.weight(cosmic_text::Weight::NORMAL);
    }
    buffer.set_text(&mut fs, text, attrs, Shaping::Advanced);
    buffer.set_size(&mut fs, Some(f32::INFINITY), Some(size * 1.4));
    buffer.shape_until_scroll(&mut fs, false);

    let cosmic_color = to_cosmic(color);
    let mut max_w = 0.0f32;

    buffer.draw(
        &mut fs,
        &mut cache,
        cosmic_color,
        |gx, gy, gw, gh, gcolor| {
            if gw == 0 || gh == 0 {
                return;
            }
            let a_mask = gcolor.a() as f32 / 255.0;
            if a_mask < 0.01 {
                return;
            }
            let c = Color::from_rgba(
                color.red(),
                color.green(),
                color.blue(),
                color.alpha() * a_mask,
            )
            .unwrap_or(color);
            let px = (x + gx as f32).round();
            let py = (y_top + gy as f32).round();
            if let Some(rect) = Rect::from_xywh(px, py, gw as f32, gh as f32) {
                let mut p = Paint::default();
                p.set_color(c);
                p.anti_alias = false;
                canvas.fill_rect(rect, &p, Transform::identity(), None);
            }
            let edge = gx as f32 + gw as f32;
            if edge > max_w {
                max_w = edge;
            }
        },
    );
    max_w
}

#[inline]
pub fn draw_text(
    canvas: &mut PixmapMut,
    x: f32,
    y_top: f32,
    text: &str,
    size: f32,
    color: Color,
    bold: bool,
) -> f32 {
    draw_text_ex(canvas, x, y_top, text, size, color, bold, false)
}

#[inline]
pub fn draw_text_mono(
    canvas: &mut PixmapMut,
    x: f32,
    y_top: f32,
    text: &str,
    size: f32,
    color: Color,
    bold: bool,
) -> f32 {
    draw_text_ex(canvas, x, y_top, text, size, color, bold, true)
}
