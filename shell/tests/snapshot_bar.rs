//! snapshot_bar.rs — renders bar 1920x32 in light + dark mode and
//! either generates baseline PNGs (first run) or compares against them.
//!
//! Threshold: 0.02 (2% max pixel difference).
//! Baseline path: shell/tests/snapshots/bar_light.png / bar_dark.png
//!
//! Run: cargo test -p lumo-shell --test snapshot_bar

use lumo_foundation::{LumoColors, LumoTheme};
use std::path::PathBuf;
use tiny_skia::{Color, Pixmap, PixmapMut};

const BAR_W: u32 = 1920;
const BAR_H: u32 = 32;
const THRESHOLD: f64 = 0.02;

fn snapshots_dir() -> PathBuf {
    // Tests run from workspace root; resolve relative to this file's dir.
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/snapshots");
    p
}

/// Render a minimal bar: fill with bg color + draw a 28px pill placeholder.
fn render_bar(palette: &LumoColors) -> Pixmap {
    let mut pixmap = Pixmap::new(BAR_W, BAR_H).expect("Pixmap::new failed");

    let bg_r = ((palette.bg >> 16) & 0xFF) as f32 / 255.0;
    let bg_g = ((palette.bg >> 8) & 0xFF) as f32 / 255.0;
    let bg_b = (palette.bg & 0xFF) as f32 / 255.0;
    let bg = Color::from_rgba(bg_r, bg_g, bg_b, 1.0).unwrap();
    pixmap.fill(bg);

    // Draw pill placeholder: dark rounded rect at x=16, y=2, w=200, h=28
    let mut canvas = pixmap.as_mut();
    let pill_r = ((palette.pill_bg >> 16) & 0xFF) as f32 / 255.0;
    let pill_g = ((palette.pill_bg >> 8) & 0xFF) as f32 / 255.0;
    let pill_b = (palette.pill_bg & 0xFF) as f32 / 255.0;
    let pill_alpha = palette.pill_bg_alpha as f32 / 255.0;
    let pill_color = Color::from_rgba(pill_r, pill_g, pill_b, pill_alpha).unwrap();

    fill_rect_simple(&mut canvas, 16.0, 2.0, 200.0, 28.0, pill_color);

    pixmap
}

fn fill_rect_simple(canvas: &mut PixmapMut, x: f32, y: f32, w: f32, h: f32, color: Color) {
    use tiny_skia::{FillRule, Paint, PathBuilder, Transform};
    let mut pb = PathBuilder::new();
    pb.move_to(x, y);
    pb.line_to(x + w, y);
    pb.line_to(x + w, y + h);
    pb.line_to(x, y + h);
    pb.close();
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = false;
        canvas.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

/// Compare two pixmaps pixel-by-pixel. Returns fraction of differing pixels.
fn pixel_diff_ratio(a: &Pixmap, b: &Pixmap) -> f64 {
    assert_eq!(a.width(), b.width());
    assert_eq!(a.height(), b.height());
    let total = (a.width() * a.height()) as f64;
    let diff_count = a
        .data()
        .chunks(4)
        .zip(b.data().chunks(4))
        .filter(|(pa, pb)| pa != pb)
        .count() as f64;
    diff_count / total
}

fn run_snapshot(mode_name: &str, palette: LumoColors) {
    let dir = snapshots_dir();
    std::fs::create_dir_all(&dir).expect("create snapshots dir");
    let path = dir.join(format!("bar_{mode_name}.png"));

    let rendered = render_bar(&palette);

    // T1.9: baseline ausente sem UPDATE_SNAPSHOTS = erro explicito.
    // Para regenerar: UPDATE_SNAPSHOTS=1 cargo test -p lumo-shell --test snapshot_bar
    if !path.exists() {
        if std::env::var("UPDATE_SNAPSHOTS").is_ok() {
            rendered.save_png(&path).expect("save baseline PNG");
            eprintln!("[snapshot] baseline saved: {}", path.display());
            return;
        } else {
            panic!(
                "baseline ausente: {}. Rode com UPDATE_SNAPSHOTS=1 para gerar.",
                path.display()
            );
        }
    }
    // Regenera baseline se UPDATE_SNAPSHOTS definido (mesmo se ja existe).
    if std::env::var("UPDATE_SNAPSHOTS").is_ok() {
        rendered.save_png(&path).expect("save baseline PNG");
        eprintln!("[snapshot] baseline atualizado: {}", path.display());
        return;
    }

    // Load baseline and compare.
    let baseline_data = std::fs::read(&path).expect("read baseline PNG");
    let baseline = Pixmap::decode_png(&baseline_data).expect("decode baseline PNG");

    let ratio = pixel_diff_ratio(&rendered, &baseline);
    assert!(
        ratio <= THRESHOLD,
        "bar_{mode_name} snapshot diff {:.2}% exceeds threshold {:.2}%",
        ratio * 100.0,
        THRESHOLD * 100.0
    );
}

#[test]
fn snapshot_bar_light() {
    run_snapshot("light", LumoColors::light());
}

#[test]
fn snapshot_bar_dark() {
    run_snapshot("dark", LumoColors::dark());
}
