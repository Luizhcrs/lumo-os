//! W12.B: Mission Control overview.
//!
//! Triggered by: SUPER+UP keybind or 3-finger swipe up.
//! Shows all toplevels of the current workspace as a mini-grid (max 3x3).
//! Click cell = activate that toplevel. Esc / click outside = dismiss.
//!
//! Rendering: pure SolidColorRenderElement cells (no texture capture).
//! Thumbnails are colored rectangles with a highlight for the focused window.
//! A11y: when reduced_motion, zoom transition is skipped (instant enter/exit).
//!
//! OverviewState is stored in LumoState; render path checks it to draw overlay.

use smithay::backend::renderer::element::solid::SolidColorRenderElement;
use smithay::backend::renderer::element::{Id, Kind};
use smithay::backend::renderer::Color32F;
use smithay::desktop::{Space, Window};
use smithay::utils::{Physical, Point, Rectangle};
use smithay::wayland::seat::WaylandFocus;

// Overview grid constants.
const MAX_COLS: usize = 3;
const CELL_GAP: i32 = 16;
const CELL_BORDER: i32 = 2;
const HEADER_H: i32 = 40;

// Colors (no neon/glow per memory feedback_zero_neon_glow).
const BG_COLOR:       [f32; 4] = [0.05, 0.05, 0.06, 0.90];
const CELL_COLOR:     [f32; 4] = [0.15, 0.15, 0.17, 1.0];
const CELL_FOCUS:     [f32; 4] = [0.22, 0.22, 0.25, 1.0];
const CELL_BORDER_C:  [f32; 4] = [0.35, 0.35, 0.40, 1.0];

/// Animation state for overview zoom-out/in.
#[derive(Debug, Clone)]
pub enum OverviewAnim {
    /// Animating in (progress 0..1).
    In  { progress: f32, velocity: f32 },
    /// Fully visible.
    Visible,
    /// Animating out (progress 1..0).
    Out { progress: f32, velocity: f32 },
    /// Done closing -- caller should clear OverviewState.
    Closed,
}

impl OverviewAnim {
    const STIFFNESS: f32 = 180.0;
    const DAMPING:   f32 = 22.0;

    pub fn new_in(reduced_motion: bool) -> Self {
        if reduced_motion {
            return OverviewAnim::Visible;
        }
        OverviewAnim::In { progress: 0.0, velocity: 0.0 }
    }

    pub fn new_out(reduced_motion: bool) -> Self {
        if reduced_motion {
            return OverviewAnim::Closed;
        }
        OverviewAnim::Out { progress: 1.0, velocity: 0.0 }
    }

    /// Advance spring by dt seconds. Returns true when animation is done.
    pub fn tick(&mut self, dt: f32) -> bool {
        match self {
            OverviewAnim::In { progress, velocity } => {
                Self::spring_step(progress, velocity, 1.0, dt);
                if *progress >= 0.998 {
                    *self = OverviewAnim::Visible;
                    return true;
                }
            }
            OverviewAnim::Out { progress, velocity } => {
                Self::spring_step(progress, velocity, 0.0, dt);
                if *progress <= 0.002 {
                    *self = OverviewAnim::Closed;
                    return true;
                }
            }
            OverviewAnim::Visible | OverviewAnim::Closed => return true,
        }
        false
    }

    pub fn visual_progress(&self) -> f32 {
        match self {
            OverviewAnim::In  { progress, .. } => *progress,
            OverviewAnim::Out { progress, .. } => *progress,
            OverviewAnim::Visible => 1.0,
            OverviewAnim::Closed  => 0.0,
        }
    }

    pub fn is_closed(&self) -> bool {
        matches!(self, OverviewAnim::Closed)
    }

    fn spring_step(pos: &mut f32, vel: &mut f32, target: f32, dt: f32) {
        let dt = dt.min(0.05);
        let disp = *pos - target;
        let accel = (-Self::STIFFNESS * disp - Self::DAMPING * *vel);
        *vel += accel * dt;
        *pos += *vel * dt;
        *pos = pos.clamp(0.0, 1.2);
    }
}

/// Per-cell geometry computed for hit-testing and rendering.
#[derive(Debug, Clone)]
pub struct OverviewCell {
    /// Index into the windows vec.
    pub window_idx: usize,
    /// Cell rect in logical pixels.
    pub rect: Rectangle<i32, smithay::utils::Logical>,
}

/// Active overview state stored in LumoState.
pub struct OverviewState {
    /// Captured windows (cloned for stable indexing).
    pub windows: Vec<Window>,
    /// Which cell the cursor is hovering (None = none).
    pub hovered: Option<usize>,
    /// Currently focused window index (used for highlight).
    pub focused_idx: Option<usize>,
    /// Animation state.
    pub anim: OverviewAnim,
}

impl OverviewState {
    /// Create overview from current space contents.
    pub fn new(
        space: &Space<Window>,
        focused: Option<&smithay::reexports::wayland_server::protocol::wl_surface::WlSurface>,
        reduced_motion: bool,
    ) -> Self {
        let windows: Vec<Window> = space.elements().cloned().collect();
        let focused_idx = focused.and_then(|s| {
            windows
                .iter()
                .position(|w| w.wl_surface().map(|ws| *ws == *s).unwrap_or(false))
        });
        Self {
            windows,
            hovered: None,
            focused_idx,
            anim: OverviewAnim::new_in(reduced_motion),
        }
    }

    /// Compute cell layout for the current window list.
    pub fn compute_cells(&self, output_w: i32, output_h: i32) -> Vec<OverviewCell> {
        compute_overview_cells(self.windows.len(), output_w, output_h)
    }

    /// Returns the window index at logical position `pos`, if any.
    pub fn hit_test(
        &self,
        pos: Point<i32, smithay::utils::Logical>,
        output_w: i32,
        output_h: i32,
    ) -> Option<usize> {
        let cells = self.compute_cells(output_w, output_h);
        cells
            .iter()
            .find(|c| c.rect.contains(pos))
            .map(|c| c.window_idx)
    }

    /// Begin close animation.
    pub fn close(&mut self, reduced_motion: bool) {
        self.anim = OverviewAnim::new_out(reduced_motion);
    }

    pub fn tick(&mut self, dt: f32) {
        self.anim.tick(dt);
    }

    pub fn is_closed(&self) -> bool {
        self.anim.is_closed()
    }
}

/// Pure cell geometry computation -- unit-testable.
pub fn compute_overview_cells(count: usize, output_w: i32, output_h: i32) -> Vec<OverviewCell> {
    if count == 0 {
        return Vec::new();
    }
    let cols = MAX_COLS.min(count);
    let rows = (count + cols - 1) / cols;

    // Grid area: centered in output below header.
    let grid_w = output_w - 2 * CELL_GAP;
    let grid_h = output_h - HEADER_H - 2 * CELL_GAP;
    let cell_w = (grid_w - (cols as i32 - 1) * CELL_GAP) / cols as i32;
    let cell_h = (grid_h - (rows as i32 - 1) * CELL_GAP) / rows as i32;

    // Center grid horizontally/vertically.
    let total_grid_w = cols as i32 * cell_w + (cols as i32 - 1) * CELL_GAP;
    let total_grid_h = rows as i32 * cell_h + (rows as i32 - 1) * CELL_GAP;
    let origin_x = (output_w - total_grid_w) / 2;
    let origin_y = HEADER_H + (output_h - HEADER_H - total_grid_h) / 2;

    (0..count)
        .map(|i| {
            let col = (i % cols) as i32;
            let row = (i / cols) as i32;
            let x = origin_x + col * (cell_w + CELL_GAP);
            let y = origin_y + row * (cell_h + CELL_GAP);
            OverviewCell {
                window_idx: i,
                rect: Rectangle::new(
                    smithay::utils::Point::from((x, y)),
                    smithay::utils::Size::from((cell_w, cell_h)),
                ),
            }
        })
        .collect()
}

/// Generate render elements for the overview overlay.
/// Called from render_common when overview is Some.
pub fn overview_elements(
    state: &OverviewState,
    output_w: i32,
    output_h: i32,
) -> Vec<SolidColorRenderElement> {
    let progress = state.anim.visual_progress();
    if progress < 0.001 {
        return Vec::new();
    }
    let alpha = progress;

    let mut out = Vec::new();

    // Full-screen dimmed background.
    let bg = Color32F::new(BG_COLOR[0], BG_COLOR[1], BG_COLOR[2], BG_COLOR[3] * alpha);
    out.push(SolidColorRenderElement::new(
        Id::new(),
        Rectangle::new(Point::from((0, 0)), (output_w, output_h).into()),
        0,
        bg,
        Kind::Unspecified,
    ));

    let cells = compute_overview_cells(state.windows.len(), output_w, output_h);
    for cell in &cells {
        let is_focused  = state.focused_idx == Some(cell.window_idx);
        let is_hovered  = state.hovered    == Some(cell.window_idx);
        let base_color  = if is_focused || is_hovered { CELL_FOCUS } else { CELL_COLOR };

        // Border quad.
        let border_rect: Rectangle<i32, Physical> = Rectangle::new(
            Point::from((cell.rect.loc.x, cell.rect.loc.y)).to_physical_precise_round(1.0),
            (cell.rect.size.w + CELL_BORDER * 2, cell.rect.size.h + CELL_BORDER * 2).into(),
        );
        out.push(SolidColorRenderElement::new(
            Id::new(),
            border_rect,
            0,
            Color32F::new(CELL_BORDER_C[0], CELL_BORDER_C[1], CELL_BORDER_C[2], alpha),
            Kind::Unspecified,
        ));

        // Cell fill quad.
        let cell_phys: Rectangle<i32, Physical> = Rectangle::new(
            Point::from((cell.rect.loc.x + CELL_BORDER, cell.rect.loc.y + CELL_BORDER))
                .to_physical_precise_round(1.0),
            (cell.rect.size.w, cell.rect.size.h).into(),
        );
        out.push(SolidColorRenderElement::new(
            Id::new(),
            cell_phys,
            0,
            Color32F::new(base_color[0], base_color[1], base_color[2], alpha),
            Kind::Unspecified,
        ));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: i32 = 1920;
    const H: i32 = 1080;

    #[test]
    fn no_windows_no_cells() {
        let cells = compute_overview_cells(0, W, H);
        assert!(cells.is_empty());
    }

    #[test]
    fn single_window_one_cell() {
        let cells = compute_overview_cells(1, W, H);
        assert_eq!(cells.len(), 1);
    }

    #[test]
    fn cells_count_matches_windows() {
        for n in 1..=9 {
            assert_eq!(compute_overview_cells(n, W, H).len(), n);
        }
    }

    #[test]
    fn cells_positive_size() {
        for n in 1..=6 {
            for c in compute_overview_cells(n, W, H) {
                assert!(c.rect.size.w > 0);
                assert!(c.rect.size.h > 0);
            }
        }
    }

    #[test]
    fn max_cols_three() {
        // 4 windows -> 2 rows x 3 cols (or adjusted). First 3 on row 0.
        let cells = compute_overview_cells(4, W, H);
        assert_eq!(cells.len(), 4);
        // Cells 0,1,2 on same row (y). Cell 3 on next row.
        assert_eq!(cells[0].rect.loc.y, cells[1].rect.loc.y);
        assert_eq!(cells[1].rect.loc.y, cells[2].rect.loc.y);
        assert!(cells[3].rect.loc.y > cells[0].rect.loc.y);
    }

    #[test]
    fn cells_increasing_x_in_row() {
        let cells = compute_overview_cells(3, W, H);
        assert!(cells[1].rect.loc.x > cells[0].rect.loc.x);
        assert!(cells[2].rect.loc.x > cells[1].rect.loc.x);
    }

    #[test]
    fn anim_reduced_motion_instant_visible() {
        let a = OverviewAnim::new_in(true);
        assert!(matches!(a, OverviewAnim::Visible));
        assert_eq!(a.visual_progress(), 1.0);
    }

    #[test]
    fn anim_out_reduced_motion_instant_closed() {
        let a = OverviewAnim::new_out(true);
        assert!(a.is_closed());
        assert_eq!(a.visual_progress(), 0.0);
    }

    #[test]
    fn anim_in_converges() {
        let mut a = OverviewAnim::new_in(false);
        for _ in 0..300 { a.tick(0.016); }
        assert!(matches!(a, OverviewAnim::Visible));
    }

    #[test]
    fn anim_out_converges() {
        let mut a = OverviewAnim::new_out(false);
        for _ in 0..300 { a.tick(0.016); }
        assert!(a.is_closed());
    }

    #[test]
    fn elements_empty_when_progress_zero() {
        let state = OverviewState {
            windows: Vec::new(),
            hovered: None,
            focused_idx: None,
            anim: OverviewAnim::Closed,
        };
        assert!(overview_elements(&state, W, H).is_empty());
    }
}
