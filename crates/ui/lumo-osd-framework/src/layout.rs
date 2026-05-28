//! layout.rs — geometry calc puro pra OSDs. Sem Wayland, sem render. Testavel.

use crate::tokens;

/// Geometry de slider em coordenadas pixmap-local.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SliderGeom {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// X final do fill bar (proporcional ao valor 0-1).
    pub fill_x_end: f32,
}

/// Geometry de toggle (dot ON/OFF).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToggleGeom {
    pub icon_cx: f32,
    pub icon_cy: f32,
    pub icon_radius: f32,
    pub label_x: f32,
    pub label_y: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct OsdLayout {
    pub width: u32,
    pub height: u32,
}

impl OsdLayout {
    pub fn default_size() -> Self {
        Self {
            width: tokens::OSD_WIDTH,
            height: tokens::OSD_HEIGHT,
        }
    }

    pub fn custom(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Slider preenche bottom half do OSD apos icon.
    /// icon_box = quadrado lado esquerdo. Slider lado direito.
    /// value 0.0-1.0 controla fill.
    pub fn slider(self, value: f32) -> SliderGeom {
        let value = value.clamp(0.0, 1.0);
        let icon_w = tokens::OSD_HEIGHT as f32 - tokens::OSD_PAD_Y * 2.0;
        let x = tokens::OSD_PAD_X + icon_w + tokens::OSD_GAP_ICON;
        let y = self.height as f32 / 2.0 + 6.0;
        let total_w = self.width as f32 - x - tokens::OSD_PAD_X;
        SliderGeom {
            x,
            y,
            w: total_w,
            h: tokens::SLIDER_H,
            fill_x_end: x + total_w * value,
        }
    }

    /// Toggle = icon + label "ON"/"OFF" lado direito.
    pub fn toggle(self) -> ToggleGeom {
        let icon_radius = 10.0;
        let icon_cx = tokens::OSD_PAD_X + icon_radius;
        let icon_cy = self.height as f32 / 2.0;
        let label_x = icon_cx + icon_radius + tokens::OSD_GAP_ICON;
        let label_y = self.height as f32 / 2.0 - 8.0;
        ToggleGeom {
            icon_cx,
            icon_cy,
            icon_radius,
            label_x,
            label_y,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slider_at_zero_fills_nothing() {
        let l = OsdLayout::default_size();
        let s = l.slider(0.0);
        assert!((s.fill_x_end - s.x).abs() < 0.01);
    }

    #[test]
    fn slider_at_one_fills_full() {
        let l = OsdLayout::default_size();
        let s = l.slider(1.0);
        assert!((s.fill_x_end - (s.x + s.w)).abs() < 0.01);
    }

    #[test]
    fn slider_at_half_fills_half() {
        let l = OsdLayout::default_size();
        let s = l.slider(0.5);
        assert!((s.fill_x_end - (s.x + s.w / 2.0)).abs() < 0.5);
    }

    #[test]
    fn slider_value_clamped_above_one() {
        let l = OsdLayout::default_size();
        let s = l.slider(1.5);
        assert!((s.fill_x_end - (s.x + s.w)).abs() < 0.01);
    }

    #[test]
    fn slider_value_clamped_below_zero() {
        let l = OsdLayout::default_size();
        let s = l.slider(-0.3);
        assert!((s.fill_x_end - s.x).abs() < 0.01);
    }

    #[test]
    fn slider_height_matches_token() {
        let l = OsdLayout::default_size();
        let s = l.slider(0.5);
        assert_eq!(s.h, tokens::SLIDER_H);
    }

    #[test]
    fn slider_x_after_icon_and_gap() {
        let l = OsdLayout::default_size();
        let s = l.slider(0.5);
        // x deve estar apos icon + gap, pelo menos pad_x + algo
        assert!(s.x > tokens::OSD_PAD_X + tokens::OSD_GAP_ICON);
    }

    #[test]
    fn toggle_icon_centered_vertical() {
        let l = OsdLayout::default_size();
        let t = l.toggle();
        assert!((t.icon_cy - l.height as f32 / 2.0).abs() < 0.01);
    }

    #[test]
    fn toggle_label_after_icon() {
        let l = OsdLayout::default_size();
        let t = l.toggle();
        assert!(t.label_x > t.icon_cx + t.icon_radius);
    }

    #[test]
    fn custom_size_changes_slider_position() {
        let small = OsdLayout::custom(200, 60);
        let large = OsdLayout::custom(400, 100);
        assert!(small.slider(1.0).w < large.slider(1.0).w);
    }

    #[test]
    fn slider_geom_eq() {
        let l = OsdLayout::default_size();
        let a = l.slider(0.5);
        let b = l.slider(0.5);
        assert_eq!(a, b);
    }
}
