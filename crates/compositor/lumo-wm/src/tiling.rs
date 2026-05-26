//! W12.A: Tiling layouts manager.
//!
//! TilingMode enum + pure layout math.
//! Modes: Floating (default), MasterStack (60/40), Spiral (recursive H/V), Columns (equal).
//! A11y: callers check reduced_motion before animating transitions.

use smithay::desktop::{Space, Window};
use smithay::utils::{Point, Size};

const MASTER_RATIO: f64 = 0.60;
const TILE_GAP: i32 = 8;
const BAR_HEIGHT: i32 = 40;

/// Tiling layout mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TilingMode {
    #[default]
    Floating,
    MasterStack,
    Spiral,
    Columns,
}

impl TilingMode {
    pub fn next(self) -> Self {
        match self {
            TilingMode::Floating => TilingMode::MasterStack,
            TilingMode::MasterStack => TilingMode::Spiral,
            TilingMode::Spiral => TilingMode::Columns,
            TilingMode::Columns => TilingMode::Floating,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            TilingMode::Floating => "floating",
            TilingMode::MasterStack => "master-stack",
            TilingMode::Spiral => "spiral",
            TilingMode::Columns => "columns",
        }
    }
}

/// A computed tile: logical-pixel position and size.
#[derive(Debug, Clone, PartialEq)]
pub struct Tile {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Pure layout computation -- no smithay mutation, fully unit-testable.
pub fn compute_tiles(count: usize, output_w: i32, output_h: i32, mode: TilingMode) -> Vec<Tile> {
    if count == 0 {
        return Vec::new();
    }
    let usable_y = BAR_HEIGHT;
    let usable_h = output_h - BAR_HEIGHT;
    match mode {
        TilingMode::Floating => Vec::new(),
        TilingMode::MasterStack => compute_master_stack(count, output_w, usable_y, usable_h),
        TilingMode::Spiral => compute_spiral(count, output_w, usable_y, usable_h),
        TilingMode::Columns => compute_columns(count, output_w, usable_y, usable_h),
    }
}

fn compute_master_stack(count: usize, out_w: i32, usable_y: i32, usable_h: i32) -> Vec<Tile> {
    if count == 1 {
        return vec![Tile {
            x: TILE_GAP,
            y: usable_y + TILE_GAP,
            w: out_w - 2 * TILE_GAP,
            h: usable_h - 2 * TILE_GAP,
        }];
    }
    let master_w = ((out_w as f64 * MASTER_RATIO) as i32) - TILE_GAP - TILE_GAP / 2;
    let slave_x = TILE_GAP / 2 + master_w + TILE_GAP;
    let slave_w = out_w - slave_x - TILE_GAP;
    let slaves = count - 1;
    let slave_h = (usable_h - 2 * TILE_GAP - (slaves as i32 - 1) * TILE_GAP) / slaves as i32;

    let mut tiles = Vec::with_capacity(count);
    tiles.push(Tile {
        x: TILE_GAP,
        y: usable_y + TILE_GAP,
        w: master_w,
        h: usable_h - 2 * TILE_GAP,
    });
    for i in 0..slaves {
        tiles.push(Tile {
            x: slave_x,
            y: usable_y + TILE_GAP + i as i32 * (slave_h + TILE_GAP),
            w: slave_w,
            h: slave_h,
        });
    }
    tiles
}

fn compute_columns(count: usize, out_w: i32, usable_y: i32, usable_h: i32) -> Vec<Tile> {
    let total_gap = TILE_GAP * (count as i32 + 1);
    let col_w = (out_w - total_gap) / count as i32;
    (0..count)
        .map(|i| Tile {
            x: TILE_GAP + i as i32 * (col_w + TILE_GAP),
            y: usable_y + TILE_GAP,
            w: col_w,
            h: usable_h - 2 * TILE_GAP,
        })
        .collect()
}

fn compute_spiral(count: usize, out_w: i32, usable_y: i32, usable_h: i32) -> Vec<Tile> {
    let mut tiles = Vec::with_capacity(count);
    if count == 0 {
        return tiles;
    }
    let mut rx = TILE_GAP;
    let mut ry = usable_y + TILE_GAP;
    let mut rw = out_w - 2 * TILE_GAP;
    let mut rh = usable_h - 2 * TILE_GAP;

    for i in 0..count {
        if i == count - 1 {
            tiles.push(Tile {
                x: rx,
                y: ry,
                w: rw,
                h: rh,
            });
        } else if i % 2 == 0 {
            // Horizontal split: place in top half, recurse bottom.
            let half_h = (rh - TILE_GAP) / 2;
            tiles.push(Tile {
                x: rx,
                y: ry,
                w: rw,
                h: half_h,
            });
            ry += half_h + TILE_GAP;
            rh -= half_h + TILE_GAP;
        } else {
            // Vertical split: place in left half, recurse right.
            let half_w = (rw - TILE_GAP) / 2;
            tiles.push(Tile {
                x: rx,
                y: ry,
                w: half_w,
                h: rh,
            });
            rx += half_w + TILE_GAP;
            rw -= half_w + TILE_GAP;
        }
    }
    tiles
}

/// Apply tiling to all elements in space. Noop for Floating.
pub fn apply_tiling(space: &mut Space<Window>, mode: TilingMode, output_w: i32, output_h: i32) {
    if mode == TilingMode::Floating {
        return;
    }
    let windows: Vec<Window> = space.elements().cloned().collect();
    let tiles = compute_tiles(windows.len(), output_w, output_h, mode);
    for (win, tile) in windows.iter().zip(tiles.iter()) {
        space.map_element(win.clone(), Point::from((tile.x, tile.y)), false);
        if let Some(tl) = win.toplevel() {
            tl.with_pending_state(|st| {
                st.size = Some(Size::from((tile.w, tile.h)));
            });
            tl.send_configure();
        }
    }
}

/// Return previous window in z-order relative to current focus.
pub fn focus_prev<'a>(
    windows: &'a [Window],
    current: Option<&smithay::reexports::wayland_server::protocol::wl_surface::WlSurface>,
) -> Option<&'a Window> {
    use smithay::wayland::seat::WaylandFocus;
    if windows.is_empty() {
        return None;
    }
    let idx = current.and_then(|s| {
        windows
            .iter()
            .position(|w| w.wl_surface().map(|ws| *ws == *s).unwrap_or(false))
    });
    let prev_idx = match idx {
        Some(i) => {
            if i == 0 {
                windows.len() - 1
            } else {
                i - 1
            }
        }
        None => 0,
    };
    windows.get(prev_idx)
}

/// Return next window in z-order relative to current focus.
pub fn focus_next<'a>(
    windows: &'a [Window],
    current: Option<&smithay::reexports::wayland_server::protocol::wl_surface::WlSurface>,
) -> Option<&'a Window> {
    use smithay::wayland::seat::WaylandFocus;
    if windows.is_empty() {
        return None;
    }
    let idx = current.and_then(|s| {
        windows
            .iter()
            .position(|w| w.wl_surface().map(|ws| *ws == *s).unwrap_or(false))
    });
    let next_idx = match idx {
        Some(i) => (i + 1) % windows.len(),
        None => 0,
    };
    windows.get(next_idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: i32 = 1920;
    const H: i32 = 1080;

    #[test]
    fn floating_returns_empty() {
        assert!(compute_tiles(3, W, H, TilingMode::Floating).is_empty());
    }

    #[test]
    fn master_stack_single_window_fills_usable() {
        let tiles = compute_tiles(1, W, H, TilingMode::MasterStack);
        assert_eq!(tiles.len(), 1);
        assert_eq!(tiles[0].x, TILE_GAP);
        assert_eq!(tiles[0].y, BAR_HEIGHT + TILE_GAP);
        assert!(tiles[0].w > 0 && tiles[0].h > 0);
    }

    #[test]
    fn master_stack_two_windows_master_left() {
        let tiles = compute_tiles(2, W, H, TilingMode::MasterStack);
        assert_eq!(tiles.len(), 2);
        assert!(tiles[0].x < tiles[1].x, "master must be left of slave");
        assert!(tiles[0].w > tiles[1].w, "master must be wider (60%)");
    }

    #[test]
    fn master_stack_three_windows_slaves_stacked() {
        let tiles = compute_tiles(3, W, H, TilingMode::MasterStack);
        assert_eq!(tiles.len(), 3);
        assert_eq!(tiles[1].x, tiles[2].x, "slaves same x");
        assert!(tiles[2].y > tiles[1].y, "slave 2 below slave 1");
    }

    #[test]
    fn columns_equal_widths() {
        let tiles = compute_tiles(3, W, H, TilingMode::Columns);
        assert_eq!(tiles.len(), 3);
        assert_eq!(tiles[0].w, tiles[1].w);
        assert_eq!(tiles[1].w, tiles[2].w);
    }

    #[test]
    fn columns_increasing_x() {
        let tiles = compute_tiles(4, W, H, TilingMode::Columns);
        for i in 1..tiles.len() {
            assert!(tiles[i].x > tiles[i - 1].x);
        }
    }

    #[test]
    fn spiral_count_matches() {
        for n in 1..=6 {
            assert_eq!(compute_tiles(n, W, H, TilingMode::Spiral).len(), n);
        }
    }

    #[test]
    fn all_tiles_positive_dimensions() {
        for mode in [
            TilingMode::MasterStack,
            TilingMode::Spiral,
            TilingMode::Columns,
        ] {
            for n in 1..=5 {
                for t in compute_tiles(n, W, H, mode) {
                    assert!(t.w > 0, "mode={:?} n={n} w={}", mode, t.w);
                    assert!(t.h > 0, "mode={:?} n={n} h={}", mode, t.h);
                }
            }
        }
    }

    #[test]
    fn tiling_mode_cycle_full() {
        assert_eq!(TilingMode::Floating.next(), TilingMode::MasterStack);
        assert_eq!(TilingMode::MasterStack.next(), TilingMode::Spiral);
        assert_eq!(TilingMode::Spiral.next(), TilingMode::Columns);
        assert_eq!(TilingMode::Columns.next(), TilingMode::Floating);
    }

    #[test]
    fn tiles_respect_bar_height() {
        for mode in [
            TilingMode::MasterStack,
            TilingMode::Spiral,
            TilingMode::Columns,
        ] {
            for t in compute_tiles(2, W, H, mode) {
                assert!(t.y >= BAR_HEIGHT, "y={} < BAR_HEIGHT={}", t.y, BAR_HEIGHT);
            }
        }
    }

    #[test]
    fn zero_count_empty() {
        assert!(compute_tiles(0, W, H, TilingMode::MasterStack).is_empty());
    }
}
