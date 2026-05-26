//! desktop/rubber_band.rs - Selecao por retangulo A34.
//!
//! Mouse down em area vazia + drag = desenha rubber-band rect.
//! Fill: accent alpha 0x20. Stroke: 1px accent alpha 0x80. Sem glow.
//! Mouse up: rect some, selecao persiste ate click vazio.

use tiny_skia::{Color, Paint, PathBuilder, PixmapMut, Stroke, Transform};

/// Estado do rubber-band.
#[derive(Debug, Clone, Copy, Default)]
pub struct RubberBand {
    /// Ponto de origem do press (canto fixo).
    pub origin: Option<(f32, f32)>,
    /// Ponto atual do mouse durante drag.
    pub current: Option<(f32, f32)>,
    /// Rubber-band ativo (ainda esta sendo desenhado).
    pub active: bool,
}

impl RubberBand {
    pub fn new() -> Self {
        Self::default()
    }

    /// Inicia rubber-band na posicao do press.
    pub fn start(&mut self, x: f32, y: f32) {
        self.origin = Some((x, y));
        self.current = Some((x, y));
        self.active = true;
    }

    /// Atualiza ponto atual (motion).
    pub fn update(&mut self, x: f32, y: f32) {
        if self.active {
            self.current = Some((x, y));
        }
    }

    /// Finaliza rubber-band. Retorna rect normalizado (x,y,w,h) se valido (>2px).
    pub fn finish(&mut self) -> Option<(f32, f32, f32, f32)> {
        self.active = false;
        let rect = self.normalized_rect();
        self.origin = None;
        self.current = None;
        rect.filter(|(_, _, w, h)| *w > 2.0 && *h > 2.0)
    }

    /// Cancela sem retornar selecao.
    pub fn cancel(&mut self) {
        self.active = false;
        self.origin = None;
        self.current = None;
    }

    /// Retorna rect normalizado (x,y,w,h) com w,h sempre positivos.
    pub fn normalized_rect(&self) -> Option<(f32, f32, f32, f32)> {
        let (ox, oy) = self.origin?;
        let (cx, cy) = self.current?;
        let x = ox.min(cx);
        let y = oy.min(cy);
        let w = (cx - ox).abs();
        let h = (cy - oy).abs();
        Some((x, y, w, h))
    }
}

/// Pinta o rubber-band rect. Deve ser chamado ANTES de paint_icons.
pub fn paint_rubber_band(canvas: &mut PixmapMut, rb: &RubberBand, accent_hex: u32) {
    if !rb.active {
        return;
    }
    let Some((x, y, w, h)) = rb.normalized_rect() else {
        return;
    };
    if w < 1.0 || h < 1.0 {
        return;
    }

    let r = ((accent_hex >> 16) & 0xFF) as u8;
    let g = ((accent_hex >> 8) & 0xFF) as u8;
    let b = (accent_hex & 0xFF) as u8;

    // Fill: accent alpha 0x20.
    let fill_color = Color::from_rgba8(r, g, b, 0x20);
    let mut pb = PathBuilder::new();
    pb.move_to(x, y);
    pb.line_to(x + w, y);
    pb.line_to(x + w, y + h);
    pb.line_to(x, y + h);
    pb.close();
    if let Some(path) = pb.finish() {
        let mut p = Paint::default();
        p.set_color(fill_color);
        p.anti_alias = false;
        canvas.fill_path(
            &path,
            &p,
            tiny_skia::FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    // Stroke: 1px accent alpha 0x80.
    let stroke_color = Color::from_rgba8(r, g, b, 0x80);
    let mut pb2 = PathBuilder::new();
    pb2.move_to(x, y);
    pb2.line_to(x + w, y);
    pb2.line_to(x + w, y + h);
    pb2.line_to(x, y + h);
    pb2.close();
    if let Some(path) = pb2.finish() {
        let mut stroke = Stroke::default();
        stroke.width = 1.0;
        let mut p = Paint::default();
        p.set_color(stroke_color);
        canvas.stroke_path(&path, &p, &stroke, Transform::identity(), None);
    }
}
