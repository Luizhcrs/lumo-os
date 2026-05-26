//! desktop/icons.rs - Desktop icons A33.
//!
//! Layout Mac-style: grid canto superior-direito, colunas pra esquerda.
//! Cell 96x110 (icon 64px + label 32px).
//! Scan ~/Desktop/ a cada 2s; ignora dotfiles.
//! Drag: persiste posicao em ~/.config/lumo/desktop-layout.json.
//! Context menu por icon: "Abrir", "Renomear", "Mover pra Lixeira".
//! "Criar pasta" acionado por menu_overlay.rs (item existente).

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, PixmapMut, Stroke, Transform};

use crate::desktop::state::{draw_text, fill_rrect, OUTPUT_H, OUTPUT_W};

// ============================================================
// Layout
// ============================================================

pub const CELL_W: f32 = 96.0;
pub const CELL_H: f32 = 110.0;
pub const ICON_SIZE: f32 = 64.0;
/// Margem interna: icon centralizado na celula horizontalmente.
pub const ICON_PAD_X: f32 = (CELL_W - ICON_SIZE) / 2.0;
pub const ICON_PAD_Y: f32 = 4.0;
/// Grid ancora: top-right com margem da borda.
/// T1.4: margem direita aumentada pra evitar icones sob pill direita da bar.
pub const GRID_MARGIN_RIGHT: f32 = 40.0;
/// T1.4: margem topo = bar height(32) + pill_margin(6) + gap (42px total) + folga de seguranca.
/// 80px garante que icones nao ficam sob pill da bar nem sob dropdown.
pub const GRID_MARGIN_TOP: f32 = 80.0;
/// Scan interval.
pub const SCAN_INTERVAL: Duration = Duration::from_secs(2);
/// Threshold de movimento pra iniciar drag (px).
pub const DRAG_THRESHOLD: f32 = 4.0;
/// Threshold duplo-click (ms).
pub const DBLCLICK_MS: u128 = 350;

// ============================================================
// Colors
// ============================================================

fn folder_color() -> Color {
    Color::from_rgba8(0xF2, 0xB5, 0x44, 0xFF)
}

fn file_color() -> Color {
    Color::from_rgba8(0xFF, 0xFF, 0xFF, 0xEE)
}

fn shadow_color() -> Color {
    Color::from_rgba8(0x00, 0x00, 0x00, 0x40)
}

fn selection_color(accent_hex: u32) -> Color {
    let r = ((accent_hex >> 16) & 0xFF) as u8;
    let g = ((accent_hex >> 8) & 0xFF) as u8;
    let b = (accent_hex & 0xFF) as u8;
    Color::from_rgba8(r, g, b, 0x4D)
}

// ============================================================
// Data types
// ============================================================

#[derive(Debug, Clone)]
pub struct DesktopIcon {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub custom_pos: Option<(f32, f32)>,
    pub screen_x: f32,
    pub screen_y: f32,
    pub selected: bool,
}

impl DesktopIcon {
    pub fn cell_rect(&self) -> (f32, f32, f32, f32) {
        (self.screen_x, self.screen_y, CELL_W, CELL_H)
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.screen_x
            && px < self.screen_x + CELL_W
            && py >= self.screen_y
            && py < self.screen_y + CELL_H
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DesktopLayout {
    pub positions: HashMap<String, (f32, f32)>,
}

impl DesktopLayout {
    pub fn load() -> Self {
        let path = layout_path();
        let Ok(data) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        serde_json::from_str(&data).unwrap_or_default()
    }

    pub fn save(&self) {
        let path = layout_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            std::fs::write(&path, json).ok();
        }
    }
}

fn layout_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".config/lumo/desktop-layout.json")
}

pub fn desktop_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join("Desktop")
}

// ============================================================
// Context menu
// ============================================================

pub const CTX_ITEMS: &[&str] = &["Abrir", "Renomear", "Mover pra Lixeira"];
pub const CTX_ITEM_H: f32 = 28.0;
pub const CTX_MENU_W: f32 = 160.0;
pub const CTX_PADDING: f32 = 6.0;

pub fn ctx_menu_h() -> f32 {
    CTX_ITEMS.len() as f32 * CTX_ITEM_H + CTX_PADDING * 2.0
}

pub fn ctx_menu_hit(mx: f32, my: f32, menu_x: f32, menu_y: f32) -> Option<usize> {
    let h = ctx_menu_h();
    if mx < menu_x || mx > menu_x + CTX_MENU_W || my < menu_y || my > menu_y + h {
        return None;
    }
    let rel_y = my - menu_y - CTX_PADDING;
    if rel_y < 0.0 {
        return None;
    }
    let item = (rel_y / CTX_ITEM_H) as usize;
    if item < CTX_ITEMS.len() {
        Some(item)
    } else {
        None
    }
}

// ============================================================
// State
// ============================================================

pub struct IconsState {
    pub icons: Vec<DesktopIcon>,
    pub last_scan: Instant,
    pub layout: DesktopLayout,
    /// Drag: (icon_idx, off_x, off_y, dragging_started, press_x, press_y).
    /// A33.fix: off precisa ser estatico (calculado no press), nao recalc no motion.
    pub drag: Option<(usize, f32, f32, bool, f32, f32)>,
    /// Duplo-click tracking: (icon_idx, timestamp).
    pub last_click: Option<(usize, Instant)>,
    /// Context menu de icon: (icon_idx, menu_x, menu_y).
    pub ctx_menu: Option<(usize, f32, f32)>,
    pub ctx_hover: usize,
}

impl IconsState {
    pub fn new() -> Self {
        let layout = DesktopLayout::load();
        let mut s = Self {
            icons: Vec::new(),
            last_scan: Instant::now() - SCAN_INTERVAL,
            layout,
            drag: None,
            last_click: None,
            ctx_menu: None,
            ctx_hover: usize::MAX,
        };
        s.scan();
        s
    }

    pub fn scan(&mut self) {
        self.last_scan = Instant::now();
        let dir = desktop_dir();
        let Ok(entries) = std::fs::read_dir(&dir) else {
            self.icons.clear();
            return;
        };

        let mut names: Vec<(String, PathBuf, bool)> = entries
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with('.') {
                    return None;
                }
                let path = e.path();
                let is_dir = path.is_dir();
                Some((name, path, is_dir))
            })
            .collect();

        names.sort_by(|a, b| a.0.cmp(&b.0));

        let old_selected: HashMap<String, bool> = self
            .icons
            .iter()
            .map(|i| (i.name.clone(), i.selected))
            .collect();

        self.icons = names
            .into_iter()
            .map(|(name, path, is_dir)| {
                let custom_pos = self.layout.positions.get(&name).copied();
                let selected = old_selected.get(&name).copied().unwrap_or(false);
                DesktopIcon {
                    name,
                    path,
                    is_dir,
                    custom_pos,
                    screen_x: 0.0,
                    screen_y: 0.0,
                    selected,
                }
            })
            .collect();

        self.recalc_positions(OUTPUT_W, OUTPUT_H);
    }

    pub fn recalc_positions(&mut self, surf_w: u32, surf_h: u32) {
        let max_rows = ((surf_h as f32 - GRID_MARGIN_TOP) / CELL_H).floor() as usize;
        let max_rows = max_rows.max(1);

        for (idx, icon) in self.icons.iter_mut().enumerate() {
            if let Some((cx, cy)) = icon.custom_pos {
                icon.screen_x = cx;
                icon.screen_y = cy;
                continue;
            }
            let row = idx % max_rows;
            let col = idx / max_rows;
            let x = surf_w as f32 - GRID_MARGIN_RIGHT - CELL_W - (col as f32 * CELL_W);
            let y = GRID_MARGIN_TOP + row as f32 * CELL_H;
            icon.screen_x = x;
            icon.screen_y = y;
        }
    }

    pub fn tick(&mut self) {
        if self.last_scan.elapsed() >= SCAN_INTERVAL {
            self.scan();
        }
    }

    pub fn hit(&self, px: f32, py: f32) -> Option<usize> {
        for (i, icon) in self.icons.iter().enumerate().rev() {
            if icon.contains(px, py) {
                return Some(i);
            }
        }
        None
    }

    pub fn clear_selection(&mut self) {
        for icon in &mut self.icons {
            icon.selected = false;
        }
    }

    pub fn select_by_rect(&mut self, rx: f32, ry: f32, rw: f32, rh: f32) {
        for icon in &mut self.icons {
            let (ix, iy, iw, ih) = icon.cell_rect();
            icon.selected = rects_intersect(rx, ry, rw, rh, ix, iy, iw, ih);
        }
    }

    /// Inicia tracking de possivel drag (ainda nao iniciou ate DRAG_THRESHOLD).
    pub fn press_icon(&mut self, idx: usize, mouse_x: f32, mouse_y: f32) {
        let icon = &self.icons[idx];
        let off_x = mouse_x - icon.screen_x;
        let off_y = mouse_y - icon.screen_y;
        self.drag = Some((idx, off_x, off_y, false, mouse_x, mouse_y));
    }

    /// Mouse move: se passou threshold, considera drag ativo.
    pub fn motion_drag(&mut self, mouse_x: f32, mouse_y: f32) -> bool {
        if let Some((idx, off_x, off_y, ref mut started, press_x, press_y)) = self.drag {
            let dx = (mouse_x - press_x).abs();
            let dy = (mouse_y - press_y).abs();
            if !*started && (dx > DRAG_THRESHOLD || dy > DRAG_THRESHOLD) {
                *started = true;
            }
            if *started {
                // A33.fix: off ja foi calculado no press_icon (estatico).
                // Bug anterior: off recalc cada motion contra screen_x mutado
                // -> aceleracao exponencial -> icon foge pra fora da tela.
                self.icons[idx].screen_x = mouse_x - off_x;
                self.icons[idx].screen_y = mouse_y - off_y;
                self.icons[idx].custom_pos =
                    Some((self.icons[idx].screen_x, self.icons[idx].screen_y));
                return true;
            }
        }
        false
    }

    /// Retorna true se drag estava ativo (devemos suprimir click).
    pub fn release_drag(&mut self) -> bool {
        if let Some((idx, _off_x, _off_y, started, _px, _py)) = self.drag.take() {
            if started {
                let icon = &self.icons[idx];
                self.layout
                    .positions
                    .insert(icon.name.clone(), (icon.screen_x, icon.screen_y));
                self.layout.save();
                return true;
            }
        }
        false
    }

    pub fn create_folder(&mut self) {
        let dir = desktop_dir();
        let base = "Nova Pasta";
        let target = if !dir.join(base).exists() {
            dir.join(base)
        } else {
            let mut n = 2u32;
            loop {
                let candidate = dir.join(format!("{} {}", base, n));
                if !candidate.exists() {
                    break candidate;
                }
                n += 1;
            }
        };
        if let Err(e) = std::fs::create_dir_all(&target) {
            eprintln!("[lumo-desktop] criar pasta falhou: {e}");
        } else {
            eprintln!("[lumo-desktop] pasta criada: {}", target.display());
        }
        self.scan();
    }

    /// A40: abre o primeiro icone selecionado via xdg-open.
    pub fn open_selected(&self) {
        for (idx, icon) in self.icons.iter().enumerate() {
            if icon.selected {
                self.open_icon(idx);
                return;
            }
        }
    }

    pub fn open_icon(&self, idx: usize) {
        if let Some(icon) = self.icons.get(idx) {
            let path = icon.path.to_string_lossy().to_string();
            // W34.3: dir -> lumo-appctl files <path>. Resto -> xdg-open.
            // W34.9 fix #7+#9: resolve lumo-appctl path + log spawn err.
            if icon.path.is_dir() {
                let bin = resolve_lumo_bin("lumo-appctl");
                let payload = format!("files:{}", path);
                match Command::new(&bin).arg(&payload).spawn() {
                    Ok(_) => eprintln!("[lumo-desktop] {} {}", bin, payload),
                    Err(e) => eprintln!("[lumo-desktop] ERR spawn {} {}: {}", bin, payload, e),
                }
            } else {
                match Command::new("xdg-open").arg(&path).spawn() {
                    Ok(_) => eprintln!("[lumo-desktop] xdg-open {}", path),
                    Err(e) => eprintln!("[lumo-desktop] ERR xdg-open {}: {}", path, e),
                }
            }
        }
    }
}

/// W34.9 fix #7: resolve lumo-* bin path. PATH primeiro, fallback ./target/release.
fn resolve_lumo_bin(name: &str) -> String {
    if Command::new(name).arg("--version").output().is_ok() {
        return name.to_string();
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let candidate = format!("{}/Projects/lumo-shell/target/release/{}", home, name);
    if std::path::Path::new(&candidate).exists() {
        return candidate;
    }
    name.to_string()
}

// ============================================================
// Render
// ============================================================

pub fn paint_icons(canvas: &mut PixmapMut, state: &IconsState, accent_hex: u32) {
    for icon in &state.icons {
        paint_icon(canvas, icon, accent_hex);
    }
}

fn paint_icon(canvas: &mut PixmapMut, icon: &DesktopIcon, accent_hex: u32) {
    let x = icon.screen_x;
    let y = icon.screen_y;

    if icon.selected {
        // Bug Luiz 2026-05-18 v2: selecao agora abraca exatamente icone+label.
        // icone em (ICON_PAD_X, ICON_PAD_Y), label abaixo. bbox visual:
        //   top    = y + ICON_PAD_Y (top do icone)
        //   bottom = y + ICON_PAD_Y + ICON_SIZE + 4 (label) + ~14 (label h)
        //   centro horizontal = x + CELL_W / 2 (icone e label centrados)
        let sel = selection_color(accent_hex);
        let sel_pad_x = 8.0;
        let sel_pad_y = 4.0;
        let sel_x = x + (CELL_W - ICON_SIZE) / 2.0 - sel_pad_x;
        let sel_y = y + ICON_PAD_Y - sel_pad_y;
        let sel_w = ICON_SIZE + sel_pad_x * 2.0;
        let sel_h = ICON_SIZE + 4.0 + 14.0 + sel_pad_y * 2.0;
        fill_rrect(canvas, sel_x, sel_y, sel_w, sel_h, 8.0, sel);
    }

    let ix = x + ICON_PAD_X;
    let iy = y + ICON_PAD_Y;

    let shad = shadow_color();
    if icon.is_dir {
        paint_folder_icon(canvas, ix + 2.0, iy + 2.0, shad);
        paint_folder_icon(canvas, ix, iy, folder_color());
    } else {
        paint_file_icon(canvas, ix + 2.0, iy + 2.0, shad);
        paint_file_icon(canvas, ix, iy, file_color());
    }

    let label = truncate_label(&icon.name, 14);
    let text_w = estimate_text_w(&label);
    let label_x = x + CELL_W / 2.0 - text_w / 2.0;
    let label_y = y + ICON_PAD_Y + ICON_SIZE + 4.0;
    let text_color = Color::from_rgba8(0xF5, 0xF5, 0xF7, 0xFF);
    draw_text(canvas, label_x, label_y, &label, 11.0, text_color);
}

fn paint_folder_icon(canvas: &mut PixmapMut, x: f32, y: f32, color: Color) {
    fill_rrect(canvas, x, y + 6.0, ICON_SIZE, ICON_SIZE - 6.0, 4.0, color);
    let tab_w = ICON_SIZE * 0.4;
    fill_rrect(canvas, x, y, tab_w, 14.0, 3.0, color);
}

fn paint_file_icon(canvas: &mut PixmapMut, x: f32, y: f32, color: Color) {
    let dog = 14.0;
    let mut pb = PathBuilder::new();
    pb.move_to(x, y);
    pb.line_to(x + ICON_SIZE - dog, y);
    pb.line_to(x + ICON_SIZE, y + dog);
    pb.line_to(x + ICON_SIZE, y + ICON_SIZE);
    pb.line_to(x, y + ICON_SIZE);
    pb.close();
    if let Some(path) = pb.finish() {
        let mut p = Paint::default();
        p.set_color(color);
        p.anti_alias = true;
        canvas.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
    }

    let mut pb2 = PathBuilder::new();
    pb2.move_to(x + ICON_SIZE - dog, y);
    pb2.line_to(x + ICON_SIZE - dog, y + dog);
    pb2.line_to(x + ICON_SIZE, y + dog);
    pb2.close();
    if let Some(path) = pb2.finish() {
        let fold_color = Color::from_rgba8(0xCC, 0xCC, 0xCC, 0xEE);
        let mut p = Paint::default();
        p.set_color(fold_color);
        p.anti_alias = true;
        canvas.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
    }
}

fn estimate_text_w(s: &str) -> f32 {
    s.len() as f32 * 6.0
}

fn truncate_label(name: &str, max_chars: usize) -> String {
    if name.chars().count() <= max_chars {
        name.to_string()
    } else {
        let truncated: String = name.chars().take(max_chars - 3).collect();
        format!("{}...", truncated)
    }
}

pub fn paint_ctx_menu(
    canvas: &mut PixmapMut,
    menu_x: f32,
    menu_y: f32,
    hover_idx: usize,
    accent_hex: u32,
) {
    let h = ctx_menu_h();
    let bg = Color::from_rgba8(0x28, 0x28, 0x2C, 0xFF);  // W37: alpha solido (0xF2 deixava icons vazarem por baixo)
    fill_rrect(canvas, menu_x, menu_y, CTX_MENU_W, h, 8.0, bg);

    let border_color = Color::from_rgba8(0x60, 0x60, 0x68, 0x80);
    if let Some(path) = rrect_stroke_path(menu_x, menu_y, CTX_MENU_W, h, 8.0) {
        let mut stroke = Stroke::default();
        stroke.width = 1.0;
        let mut p = Paint::default();
        p.set_color(border_color);
        canvas.stroke_path(&path, &p, &stroke, Transform::identity(), None);
    }

    for (i, &label) in CTX_ITEMS.iter().enumerate() {
        let item_y = menu_y + CTX_PADDING + i as f32 * CTX_ITEM_H;

        if i == hover_idx {
            let r = ((accent_hex >> 16) & 0xFF) as u8;
            let g = ((accent_hex >> 8) & 0xFF) as u8;
            let b = (accent_hex & 0xFF) as u8;
            let accent = Color::from_rgba8(r, g, b, 0xFF);
            fill_rrect(
                canvas,
                menu_x + 4.0,
                item_y,
                CTX_MENU_W - 8.0,
                CTX_ITEM_H,
                4.0,
                accent,
            );
        }

        let text_color = Color::from_rgba8(0xF5, 0xF5, 0xF7, 0xFF);
        draw_text(canvas, menu_x + 12.0, item_y + 7.0, label, 12.0, text_color);
    }
}

fn rrect_stroke_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<tiny_skia::Path> {
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

fn rects_intersect(ax: f32, ay: f32, aw: f32, ah: f32, bx: f32, by: f32, bw: f32, bh: f32) -> bool {
    ax < bx + bw && ax + aw > bx && ay < by + bh && ay + ah > by
}

// ============================================================
// Unit tests -- logica pura sem I/O de disco.
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_icon(name: &str, x: f32, y: f32) -> DesktopIcon {
        DesktopIcon {
            name: name.to_string(),
            path: PathBuf::from(format!("/tmp/{name}")),
            is_dir: false,
            custom_pos: None,
            screen_x: x,
            screen_y: y,
            selected: false,
        }
    }

    fn make_state(icons: Vec<DesktopIcon>) -> IconsState {
        IconsState {
            icons,
            layout: DesktopLayout::default(),
            last_scan: std::time::Instant::now(),
            drag: None,
            last_click: None,
            ctx_menu: None,
            ctx_hover: usize::MAX,
        }
    }

    // T01: contains returns true when point is inside cell.
    #[test]
    fn contains_inside_cell() {
        let icon = make_icon("test.txt", 100.0, 200.0);
        assert!(icon.contains(150.0, 250.0));
    }

    // T02: contains returns false when point is outside cell (left).
    #[test]
    fn contains_outside_cell_left() {
        let icon = make_icon("test.txt", 100.0, 200.0);
        assert!(!icon.contains(50.0, 250.0));
    }

    // T03: contains returns false at right edge (exclusive).
    #[test]
    fn contains_at_right_edge_exclusive() {
        let icon = make_icon("test.txt", 100.0, 200.0);
        assert!(!icon.contains(100.0 + CELL_W, 200.0 + CELL_H / 2.0));
    }

    // T04: hit returns None when no icons.
    #[test]
    fn hit_empty_returns_none() {
        let state = make_state(vec![]);
        assert!(state.hit(0.0, 0.0).is_none());
    }

    // T05: hit returns index of icon containing point.
    #[test]
    fn hit_returns_correct_index() {
        let state = make_state(vec![
            make_icon("a.txt", 0.0, 0.0),
            make_icon("b.txt", 200.0, 0.0),
        ]);
        assert_eq!(state.hit(210.0, 10.0), Some(1));
    }

    // T06: press_icon sets drag offset relative to icon position.
    #[test]
    fn press_icon_sets_drag_offset() {
        let mut state = make_state(vec![make_icon("test.txt", 50.0, 60.0)]);
        state.press_icon(0, 70.0, 80.0);
        let drag = state.drag.unwrap();
        assert_eq!(drag.0, 0);
        assert!((drag.1 - 20.0).abs() < 1e-5); // off_x = 70 - 50
        assert!((drag.2 - 20.0).abs() < 1e-5); // off_y = 80 - 60
        assert!(!drag.3); // not started
    }

    // T07: motion_drag returns false before DRAG_THRESHOLD.
    #[test]
    fn motion_drag_false_below_threshold() {
        let mut state = make_state(vec![make_icon("test.txt", 50.0, 60.0)]);
        state.press_icon(0, 70.0, 80.0);
        let moved = state.motion_drag(71.0, 80.5); // dx=1 < DRAG_THRESHOLD=4
        assert!(!moved);
    }

    // T08: motion_drag returns true after DRAG_THRESHOLD crossed.
    #[test]
    fn motion_drag_true_after_threshold() {
        let mut state = make_state(vec![make_icon("test.txt", 50.0, 60.0)]);
        state.press_icon(0, 70.0, 80.0);
        let moved = state.motion_drag(80.0, 80.0); // dx=10 > DRAG_THRESHOLD
        assert!(moved);
    }

    // T09: release_drag returns false when drag was not started.
    #[test]
    fn release_drag_not_started_returns_false() {
        let mut state = make_state(vec![make_icon("test.txt", 50.0, 60.0)]);
        state.press_icon(0, 70.0, 80.0);
        state.motion_drag(70.5, 80.0); // below threshold
        let was_drag = state.release_drag();
        assert!(!was_drag);
    }

    // T10: release_drag returns true when drag was active.
    #[test]
    fn release_drag_active_returns_true() {
        let mut state = make_state(vec![make_icon("test.txt", 50.0, 60.0)]);
        state.press_icon(0, 70.0, 80.0);
        state.motion_drag(80.0, 80.0);
        let was_drag = state.release_drag();
        assert!(was_drag);
    }

    // T11: release_drag clears drag state.
    #[test]
    fn release_drag_clears_state() {
        let mut state = make_state(vec![make_icon("test.txt", 50.0, 60.0)]);
        state.press_icon(0, 70.0, 80.0);
        state.motion_drag(80.0, 80.0);
        state.release_drag();
        assert!(state.drag.is_none());
    }

    // T12: ctx_menu_hit returns None when point is outside menu.
    #[test]
    fn ctx_menu_hit_outside_returns_none() {
        let result = ctx_menu_hit(0.0, 0.0, 500.0, 500.0);
        assert!(result.is_none());
    }

    // T13: ctx_menu_hit returns first item index.
    #[test]
    fn ctx_menu_hit_first_item() {
        let menu_x = 100.0;
        let menu_y = 100.0;
        let click_y = menu_y + CTX_PADDING + CTX_ITEM_H * 0.5;
        let result = ctx_menu_hit(menu_x + 10.0, click_y, menu_x, menu_y);
        assert_eq!(result, Some(0));
    }

    // T14: truncate_label does not truncate short names.
    #[test]
    fn truncate_label_short_name() {
        let result = super::truncate_label("hello.txt", 14);
        assert_eq!(result, "hello.txt");
    }

    // T15: truncate_label truncates and adds ellipsis for long names.
    #[test]
    fn truncate_label_long_name_has_ellipsis() {
        let result = super::truncate_label("very_long_filename_here.txt", 14);
        assert!(result.ends_with("..."));
        assert!(result.chars().count() <= 14);
    }

    // T16: recalc_positions sets screen_x/y in valid grid area.
    #[test]
    fn recalc_positions_sets_coords() {
        let mut state = make_state(vec![make_icon("test.txt", 0.0, 0.0)]);
        state.recalc_positions(1920, 1080);
        assert!(state.icons[0].screen_x > 0.0);
        assert!(state.icons[0].screen_y >= GRID_MARGIN_TOP);
    }

    // T17: clear_selection deselects all icons.
    #[test]
    fn clear_selection_deselects_all() {
        let mut state = make_state(vec![
            make_icon("a.txt", 0.0, 0.0),
            make_icon("b.txt", 200.0, 0.0),
        ]);
        state.icons[0].selected = true;
        state.icons[1].selected = true;
        state.clear_selection();
        assert!(!state.icons[0].selected);
        assert!(!state.icons[1].selected);
    }

    // T18: cell_rect returns correct position and dimensions.
    #[test]
    fn cell_rect_correct() {
        let icon = make_icon("test.txt", 100.0, 200.0);
        let (x, y, w, h) = icon.cell_rect();
        assert!((x - 100.0).abs() < 1e-5);
        assert!((y - 200.0).abs() < 1e-5);
        assert!((w - CELL_W).abs() < 1e-5);
        assert!((h - CELL_H).abs() < 1e-5);
    }
}
