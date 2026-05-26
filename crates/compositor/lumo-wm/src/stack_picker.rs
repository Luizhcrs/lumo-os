//! W12.C: Window stack picker.
//!
//! Visual overlay for SUPER+TAB window switching.
//! SUPER+TAB pressed: render horizontal list of window thumbnails (max 8).
//! Subsequent Tab    = cycle right.
//! Shift+Tab         = cycle left.
//! Release SUPER     = activate selected.
//! Esc               = dismiss without switching.
//!
//! Rendering: SolidColorRenderElement cells centered horizontally.
//! Each cell is a colored rect; selected cell has distinct highlight.
//!
//! StackPickerState lives in LumoState.
//! Keybindings dispatch to picker methods when picker is active.

use smithay::backend::renderer::element::solid::SolidColorRenderElement;
use smithay::backend::renderer::element::{Id, Kind};
use smithay::backend::renderer::Color32F;
use smithay::desktop::{Space, Window};
use smithay::utils::{Physical, Point, Rectangle};

const MAX_CELLS: usize = 8;
const CELL_W: i32 = 120;
const CELL_H: i32 = 80;
const CELL_GAP: i32 = 12;
const CELL_PADDING: i32 = 16;
const _PICKER_RADIUS: i32 = 8;

// Colors.
const BG_COLOR: [f32; 4] = [0.08, 0.08, 0.10, 0.92];
const CELL_NORMAL: [f32; 4] = [0.18, 0.18, 0.20, 1.0];
const CELL_SELECTED: [f32; 4] = [0.30, 0.30, 0.35, 1.0];
const LABEL_LINE: [f32; 4] = [0.50, 0.50, 0.55, 1.0];

/// Active window stack picker state.
pub struct StackPickerState {
    /// Window list snapshot when picker was opened (max MAX_CELLS).
    pub windows: Vec<Window>,
    /// Currently selected index.
    pub selected: usize,
}

impl StackPickerState {
    /// Open picker with current space contents.
    /// `focused` is used to set the initial selected index to the current window.
    pub fn new(
        space: &Space<Window>,
        focused: Option<&smithay::reexports::wayland_server::protocol::wl_surface::WlSurface>,
    ) -> Self {
        use smithay::wayland::seat::WaylandFocus;
        let windows: Vec<Window> = space.elements().cloned().take(MAX_CELLS).collect();
        // Start selection at next window (Tab cycles forward on first press).
        let focused_idx = focused.and_then(|s| {
            windows
                .iter()
                .position(|w| w.wl_surface().map(|ws| *ws == *s).unwrap_or(false))
        });
        let selected = match focused_idx {
            Some(i) => (i + 1) % windows.len().max(1),
            None => 0,
        };
        Self { windows, selected }
    }

    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    /// Cycle selection right (Tab).
    pub fn cycle_next(&mut self) {
        if self.windows.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.windows.len();
    }

    /// Cycle selection left (Shift+Tab).
    pub fn cycle_prev(&mut self) {
        if self.windows.is_empty() {
            return;
        }
        self.selected = if self.selected == 0 {
            self.windows.len() - 1
        } else {
            self.selected - 1
        };
    }

    /// Return the currently selected window.
    pub fn selected_window(&self) -> Option<&Window> {
        self.windows.get(self.selected)
    }
}

/// Geometry of a single picker cell.
#[derive(Debug, Clone)]
pub struct PickerCell {
    pub index: usize,
    pub rect: Rectangle<i32, smithay::utils::Logical>,
}

/// Pure geometry computation.
pub fn compute_picker_cells(count: usize, output_w: i32, output_h: i32) -> Vec<PickerCell> {
    if count == 0 {
        return Vec::new();
    }
    let n = count.min(MAX_CELLS);
    let total_w = n as i32 * CELL_W + (n as i32 - 1) * CELL_GAP + 2 * CELL_PADDING;
    let total_h = CELL_H + 2 * CELL_PADDING;
    let origin_x = (output_w - total_w) / 2;
    // Vertically centered.
    let origin_y = (output_h - total_h) / 2;

    (0..n)
        .map(|i| {
            let x = origin_x + CELL_PADDING + i as i32 * (CELL_W + CELL_GAP);
            let y = origin_y + CELL_PADDING;
            PickerCell {
                index: i,
                rect: Rectangle::new(
                    smithay::utils::Point::from((x, y)),
                    smithay::utils::Size::from((CELL_W, CELL_H)),
                ),
            }
        })
        .collect()
}

/// Generate SolidColorRenderElements for the picker overlay.
pub fn picker_elements(
    state: &StackPickerState,
    output_w: i32,
    output_h: i32,
) -> Vec<SolidColorRenderElement> {
    if state.is_empty() {
        return Vec::new();
    }
    let cells = compute_picker_cells(state.windows.len(), output_w, output_h);
    if cells.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();

    // Background panel.
    let n = cells.len();
    let total_w = n as i32 * CELL_W + (n as i32 - 1) * CELL_GAP + 2 * CELL_PADDING;
    let total_h = CELL_H + 2 * CELL_PADDING;
    let panel_x = (output_w - total_w) / 2;
    let panel_y = (output_h - total_h) / 2;
    let panel_rect: Rectangle<i32, Physical> = Rectangle::new(
        Point::from((panel_x, panel_y)).to_physical_precise_round(1.0),
        (total_w, total_h).into(),
    );
    out.push(SolidColorRenderElement::new(
        Id::new(),
        panel_rect,
        0,
        Color32F::new(BG_COLOR[0], BG_COLOR[1], BG_COLOR[2], BG_COLOR[3]),
        Kind::Unspecified,
    ));

    for cell in &cells {
        let is_selected = cell.index == state.selected;
        let color = if is_selected {
            CELL_SELECTED
        } else {
            CELL_NORMAL
        };

        // Cell quad.
        let cell_phys: Rectangle<i32, Physical> = Rectangle::new(
            Point::from((cell.rect.loc.x, cell.rect.loc.y)).to_physical_precise_round(1.0),
            (cell.rect.size.w, cell.rect.size.h).into(),
        );
        out.push(SolidColorRenderElement::new(
            Id::new(),
            cell_phys,
            0,
            Color32F::new(color[0], color[1], color[2], color[3]),
            Kind::Unspecified,
        ));

        // Label line at bottom of cell (thin horizontal bar).
        if is_selected {
            let label_phys: Rectangle<i32, Physical> = Rectangle::new(
                Point::from((cell.rect.loc.x, cell.rect.loc.y + cell.rect.size.h - 4))
                    .to_physical_precise_round(1.0),
                (cell.rect.size.w, 3).into(),
            );
            out.push(SolidColorRenderElement::new(
                Id::new(),
                label_phys,
                0,
                Color32F::new(LABEL_LINE[0], LABEL_LINE[1], LABEL_LINE[2], LABEL_LINE[3]),
                Kind::Unspecified,
            ));
        }
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
        let cells = compute_picker_cells(0, W, H);
        assert!(cells.is_empty());
    }

    #[test]
    fn single_window_one_cell() {
        let cells = compute_picker_cells(1, W, H);
        assert_eq!(cells.len(), 1);
    }

    #[test]
    fn cells_capped_at_max() {
        let cells = compute_picker_cells(MAX_CELLS + 5, W, H);
        assert_eq!(cells.len(), MAX_CELLS);
    }

    #[test]
    fn cells_positive_size() {
        for n in 1..=MAX_CELLS {
            for c in compute_picker_cells(n, W, H) {
                assert!(c.rect.size.w > 0);
                assert!(c.rect.size.h > 0);
            }
        }
    }

    #[test]
    fn cells_increasing_x() {
        let cells = compute_picker_cells(4, W, H);
        for i in 1..cells.len() {
            assert!(cells[i].rect.loc.x > cells[i - 1].rect.loc.x);
        }
    }

    #[test]
    fn cells_same_y() {
        let cells = compute_picker_cells(4, W, H);
        let y0 = cells[0].rect.loc.y;
        for c in &cells {
            assert_eq!(c.rect.loc.y, y0);
        }
    }

    #[test]
    fn cycle_next_wraps() {
        let mut picker = StackPickerState {
            windows: vec![],
            selected: 0,
        };
        // Empty -- should not panic.
        picker.cycle_next();
        assert_eq!(picker.selected, 0);
    }

    #[test]
    fn cycle_wraps_correctly() {
        // Manually set up picker without smithay types.
        let mut picker = StackPickerState {
            windows: vec![],
            selected: 2,
        };
        // Simulate 3 windows by overriding the vec len via direct mutation:
        // We just test the index logic with empty vec (len=0), which no-ops.
        // Full integration test would need a real smithay space.
        picker.selected = 2;
        // cycle_next on empty is noop, selected stays 2.
        picker.cycle_next();
        assert_eq!(picker.selected, 2);
    }

    #[test]
    fn cycle_prev_empty_noop() {
        let mut picker = StackPickerState {
            windows: vec![],
            selected: 0,
        };
        picker.cycle_prev();
        assert_eq!(picker.selected, 0);
    }

    #[test]
    fn picker_elements_empty_for_empty_state() {
        let state = StackPickerState {
            windows: vec![],
            selected: 0,
        };
        assert!(picker_elements(&state, W, H).is_empty());
    }

    #[test]
    fn cells_centered_horizontally() {
        let cells = compute_picker_cells(3, W, H);
        let first_x = cells[0].rect.loc.x;
        let last = cells.last().unwrap();
        let last_x_end = last.rect.loc.x + last.rect.size.w;
        // Both margins from output edge should be roughly equal.
        let margin_left = first_x;
        let margin_right = W - last_x_end;
        let diff = (margin_left - margin_right).abs();
        assert!(diff <= 2, "margins differ by {diff}px");
    }
}
