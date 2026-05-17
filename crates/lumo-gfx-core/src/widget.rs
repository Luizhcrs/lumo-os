//! Widgets (Layer 4.1.8 + 4.1.9).
//!
//! Primeiro widget Lumo de verdade: `Button`. Composto por:
//!   - 1 quad (background + corner radius + opcional border + drop shadow)
//!   - 1 run de texto centralizada no padding
//!
//! Layer 4.1.8: stateless (queue / measure / render direto).
//! Layer 4.1.9: state machine externa via `ButtonHandle` + spring animation
//! no `press_progress`. Caller mantem o handle entre frames; o `Button`
//! continua stateless e e renderizado via `queue` (sem state) ou
//! `queue_stateful(handle)` (com modulacao de cor/scale por estado).
//!
//! Convencao de cor: todas as cores aqui sao **linear** (consistente com
//! `color::*`). Se vier sRGB de um theme runtime, converter via
//! `color::srgb_to_linear` antes.

use std::time::Instant;

use cosmic_text::{Attrs, Buffer, Family, Metrics, Shaping, Weight};

use crate::{
    anim::Spring,
    color,
    input::{LumoEvent, MouseButton},
    px_center_to_ndc, px_offset_to_ndc, px_size_to_ndc, px_to_ndc_radius,
    text::{TextRenderer, TextStyle},
    QuadInstance,
};

// ============================================================================
// Rect + hit testing (Layer 4.1.9)
// ============================================================================

/// Retangulo em pixels (origem top-left). Usado como hit area dos widgets.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    /// `true` se `p` (pixels, top-left) cai dentro do rect.
    pub fn contains(&self, p: [f32; 2]) -> bool {
        p[0] >= self.x && p[0] <= self.x + self.w && p[1] >= self.y && p[1] <= self.y + self.h
    }
}

// ============================================================================
// Widget state machine (Layer 4.1.9)
// ============================================================================

/// Estados visuais de um widget interativo. Transicoes:
///   Idle -> Hover    : pointer entra no rect
///   Hover -> Idle    : pointer sai do rect
///   Hover -> Pressed : pointer press dentro do rect
///   Pressed -> Hover : pointer release dentro do rect (gera click)
///   Pressed -> Idle  : pointer release fora OU pointer sai durante press
///   * -> Disabled    : setado externamente; bloqueia hover/press
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetState {
    Idle,
    Hover,
    Pressed,
    Disabled,
}

impl Default for WidgetState {
    fn default() -> Self {
        WidgetState::Idle
    }
}

/// Handle mantido pelo caller entre frames. Guarda state + rect (atualizado
/// no `queue_stateful`) + spring de animacao + timestamp da ultima transicao.
#[derive(Clone, Debug)]
pub struct ButtonHandle {
    pub state: WidgetState,
    pub rect: Rect,
    /// 0 = visual idle, 1 = visual pressed. Animado via spring.
    pub press_progress: f32,
    pub last_state_change: Instant,
    /// Spring que dirige o `press_progress`.
    spring: Spring,
}

impl Default for ButtonHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl ButtonHandle {
    pub fn new() -> Self {
        let mut spring = Spring::snappy();
        spring.set_value(0.0);
        spring.set_target(0.0);
        Self {
            state: WidgetState::Idle,
            rect: Rect::default(),
            press_progress: 0.0,
            last_state_change: Instant::now(),
            spring,
        }
    }

    pub fn disabled() -> Self {
        let mut h = Self::new();
        h.state = WidgetState::Disabled;
        h
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        if !enabled {
            self.transition(WidgetState::Disabled);
        } else if self.state == WidgetState::Disabled {
            self.transition(WidgetState::Idle);
        }
    }

    fn transition(&mut self, next: WidgetState) {
        if self.state == next {
            return;
        }
        self.state = next;
        self.last_state_change = Instant::now();
        // Spring target: 1.0 quando Pressed, 0.0 caso contrario.
        let target = if next == WidgetState::Pressed { 1.0 } else { 0.0 };
        self.spring.set_target(target);
    }

    /// Processa um `LumoEvent`. Retorna `true` se o botao foi **clicado**
    /// (release de left button dentro do rect estando em Pressed).
    pub fn handle_event(&mut self, event: &LumoEvent) -> bool {
        if self.state == WidgetState::Disabled {
            return false;
        }

        match event {
            LumoEvent::PointerMove { position } => {
                let inside = self.rect.contains(*position);
                match (self.state, inside) {
                    (WidgetState::Idle, true) => self.transition(WidgetState::Hover),
                    (WidgetState::Hover, false) => self.transition(WidgetState::Idle),
                    (WidgetState::Pressed, false) => {
                        // Arrastou pra fora durante press: cancela o press
                        // mas NAO emite click. Volta pra Idle (sem hover, ja
                        // que esta fora).
                        self.transition(WidgetState::Idle);
                    }
                    _ => {}
                }
                false
            }
            LumoEvent::PointerPress {
                position,
                button: MouseButton::Left,
            } => {
                if self.rect.contains(*position) {
                    self.transition(WidgetState::Pressed);
                }
                false
            }
            LumoEvent::PointerRelease {
                position,
                button: MouseButton::Left,
            } => {
                let was_pressed = self.state == WidgetState::Pressed;
                let inside = self.rect.contains(*position);
                if was_pressed {
                    if inside {
                        self.transition(WidgetState::Hover);
                        return true;
                    } else {
                        self.transition(WidgetState::Idle);
                    }
                }
                false
            }
            _ => false,
        }
    }

    /// Integra a animacao com `dt` em segundos. Caller deve chamar a cada
    /// frame antes de `queue_stateful`.
    pub fn update(&mut self, dt: f32) {
        if !self.spring.settled() || (self.spring.value - self.spring.target).abs() > 0.0001 {
            self.spring.tick(dt);
            self.press_progress = self.spring.value.clamp(-0.2, 1.2);
        } else {
            // Snap pro target final pra evitar drift.
            self.press_progress = self.spring.target;
            self.spring.set_value(self.spring.target);
        }
    }

    /// Util pra setar o rect manualmente (testes ou layout custom). O
    /// `queue_stateful` ja faz isso automaticamente baseado em `measure`.
    pub fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }
}

// ============================================================================
// Button (Layer 4.1.8) — stateless visual config
// ============================================================================

/// Botao stateless: quad + label.
///
/// Construa via `Button::primary()` / `Button::ghost()` / `Button::danger()`
/// ou customize com builder methods. Chame `queue` (stateless) ou
/// `queue_stateful(handle)` (com modulacao por estado + spring anim).
#[derive(Clone, Debug)]
pub struct Button {
    pub label: String,

    /// Background fill (linear). `TRANSPARENT` desabilita.
    pub bg: [f32; 4],
    /// Cor do texto (linear).
    pub label_color: [f32; 4],

    /// Border (linear). Alpha 0 desabilita.
    pub border_color: [f32; 4],
    /// Border width em pixels.
    pub border_px: f32,

    /// Corner radius em pixels.
    pub corner_radius_px: f32,
    /// Padding interno (horizontal, vertical) em pixels.
    pub padding: [f32; 2],

    /// Tamanho da fonte em pixels.
    pub font_size: f32,
    /// Familia da fonte (cosmic-text resolve via fontdb).
    pub font_family: String,
    /// Peso (cosmic-text Weight).
    pub font_weight: Weight,

    /// Cor da sombra (linear). Alpha 0 desabilita.
    pub shadow_color: [f32; 4],
    /// Offset CSS-style (positivo = direita/baixo) em pixels.
    pub shadow_offset_px: [f32; 2],
    /// Spread da sombra em pixels.
    pub shadow_radius_px: f32,
}

impl Button {
    /// Botao "vazio". Use `primary()` / `ghost()` / `danger()` na pratica.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            bg: color::TRANSPARENT,
            label_color: color::PEARL,
            border_color: color::TRANSPARENT,
            border_px: 0.0,
            corner_radius_px: 10.0,
            padding: [20.0, 12.0],
            font_size: 16.0,
            font_family: "Geist".to_string(),
            font_weight: Weight::SEMIBOLD,
            shadow_color: color::TRANSPARENT,
            shadow_offset_px: [0.0, 0.0],
            shadow_radius_px: 0.0,
        }
    }

    // -- presets --------------------------------------------------------------

    pub fn primary() -> Self {
        Self {
            label: "Continuar".to_string(),
            bg: color::EMERALD_600,
            label_color: color::PEARL,
            border_color: color::TRANSPARENT,
            border_px: 0.0,
            corner_radius_px: 10.0,
            padding: [22.0, 12.0],
            font_size: 16.0,
            font_family: "Geist".to_string(),
            font_weight: Weight::SEMIBOLD,
            shadow_color: color::SHADOW_BLACK,
            shadow_offset_px: [0.0, 3.0],
            shadow_radius_px: 8.0,
        }
    }

    pub fn ghost() -> Self {
        Self {
            label: "Cancelar".to_string(),
            bg: color::TRANSPARENT,
            label_color: color::EMERALD_500,
            border_color: color::EMERALD_500,
            border_px: 1.5,
            corner_radius_px: 10.0,
            padding: [22.0, 12.0],
            font_size: 16.0,
            font_family: "Geist".to_string(),
            font_weight: Weight::SEMIBOLD,
            shadow_color: color::TRANSPARENT,
            shadow_offset_px: [0.0, 0.0],
            shadow_radius_px: 0.0,
        }
    }

    pub fn danger() -> Self {
        Self {
            label: "Apagar".to_string(),
            bg: color::DANGER,
            label_color: color::PEARL,
            border_color: color::TRANSPARENT,
            border_px: 0.0,
            corner_radius_px: 10.0,
            padding: [22.0, 12.0],
            font_size: 16.0,
            font_family: "Geist".to_string(),
            font_weight: Weight::SEMIBOLD,
            shadow_color: color::SHADOW_DANGER,
            shadow_offset_px: [0.0, 3.0],
            shadow_radius_px: 10.0,
        }
    }

    // -- builders -------------------------------------------------------------

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }
    pub fn with_bg(mut self, bg: [f32; 4]) -> Self {
        self.bg = bg;
        self
    }
    pub fn with_label_color(mut self, c: [f32; 4]) -> Self {
        self.label_color = c;
        self
    }
    pub fn with_radius(mut self, r: f32) -> Self {
        self.corner_radius_px = r;
        self
    }
    pub fn with_border(mut self, color: [f32; 4], width: f32) -> Self {
        self.border_color = color;
        self.border_px = width;
        self
    }
    pub fn with_padding(mut self, h: f32, v: f32) -> Self {
        self.padding = [h, v];
        self
    }
    pub fn with_font(mut self, family: impl Into<String>, size: f32, weight: Weight) -> Self {
        self.font_family = family.into();
        self.font_size = size;
        self.font_weight = weight;
        self
    }
    pub fn with_shadow(mut self, color: [f32; 4], offset: [f32; 2], radius: f32) -> Self {
        self.shadow_color = color;
        self.shadow_offset_px = offset;
        self.shadow_radius_px = radius;
        self
    }

    // -- measure / layout -----------------------------------------------------

    /// Mede o tamanho total do botao em pixels (label + padding).
    pub fn measure(&self, text_r: &mut TextRenderer) -> [f32; 2] {
        let metrics = Metrics::new(self.font_size, self.font_size * 1.25);
        let mut buf = Buffer::new(text_r.font_system_mut(), metrics);
        let family = match self.font_family.to_lowercase().as_str() {
            "monospace" | "mono" => Family::Monospace,
            "serif" => Family::Serif,
            _ => Family::Name(&self.font_family),
        };
        let attrs = Attrs::new().family(family).weight(self.font_weight);
        buf.set_size(
            text_r.font_system_mut(),
            Some(4096.0),
            Some(self.font_size * 2.0),
        );
        buf.set_text(
            text_r.font_system_mut(),
            &self.label,
            attrs,
            Shaping::Advanced,
        );
        buf.shape_until_scroll(text_r.font_system_mut(), false);

        let mut w_px: f32 = 0.0;
        let mut h_px: f32 = self.font_size;
        for run in buf.layout_runs() {
            if run.line_w > w_px {
                w_px = run.line_w;
            }
            h_px = h_px.max(run.line_height);
        }
        [
            w_px + self.padding[0] * 2.0,
            h_px + self.padding[1] * 2.0,
        ]
    }

    // -- render ---------------------------------------------------------------

    /// Render stateless (Layer 4.1.8 compat). Veja `queue_stateful` para
    /// versao reativa.
    pub fn queue(
        &self,
        quads: &mut Vec<QuadInstance>,
        text_r: &mut TextRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        position: [f32; 2],
        viewport: [f32; 2],
    ) -> [f32; 2] {
        self.queue_internal(quads, text_r, device, queue, position, viewport, None)
    }

    /// Render com state machine. Atualiza `handle.rect` com a hit area
    /// medida e modula visual (brightness / scale / shadow) baseado em
    /// `handle.state` + `handle.press_progress`.
    pub fn queue_stateful(
        &self,
        quads: &mut Vec<QuadInstance>,
        text_r: &mut TextRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        position: [f32; 2],
        viewport: [f32; 2],
        handle: &mut ButtonHandle,
    ) -> [f32; 2] {
        self.queue_internal(
            quads,
            text_r,
            device,
            queue,
            position,
            viewport,
            Some(handle),
        )
    }

    fn queue_internal(
        &self,
        quads: &mut Vec<QuadInstance>,
        text_r: &mut TextRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        position: [f32; 2],
        viewport: [f32; 2],
        handle: Option<&mut ButtonHandle>,
    ) -> [f32; 2] {
        let size = self.measure(text_r);

        // ---- visual modulation por state ---------------------------------
        let (bg_mod, label_mod, scale, shadow_alpha_mod, opacity) = if let Some(h) = &handle {
            visual_for_state(h.state, h.press_progress, self.bg, self.label_color)
        } else {
            (self.bg, self.label_color, 1.0, 1.0, 1.0)
        };

        // Atualiza rect no handle (hit area inalterada por scale visual —
        // usuario clica no tamanho original pra evitar jitter de hover).
        if let Some(h) = handle {
            h.rect = Rect::new(position[0], position[1], size[0], size[1]);
        }

        // Scale visual: aplica no half_size em NDC. Rect logico permanece
        // o original (acima).
        let visual_size = [size[0] * scale, size[1] * scale];
        let visual_offset = [
            (size[0] - visual_size[0]) * 0.5,
            (size[1] - visual_size[1]) * 0.5,
        ];
        let center_px = [
            position[0] + size[0] * 0.5 + visual_offset[0] - visual_offset[0],
            position[1] + size[1] * 0.5 + visual_offset[1] - visual_offset[1],
        ];
        let center_ndc = px_center_to_ndc(center_px[0], center_px[1], viewport);
        let size_ndc = px_size_to_ndc(visual_size[0], visual_size[1], viewport);

        // Corner radius escala proporcionalmente ao scale visual; mantem
        // ratio raio/size constante.
        let effective_radius_px = self.corner_radius_px * scale;
        let radius_ndc = px_to_ndc_radius(effective_radius_px, viewport[1]);
        let border_ndc = if self.border_px > 0.0 {
            px_to_ndc_radius(self.border_px, viewport[1])
        } else {
            0.0
        };

        // Aplica opacity sobre bg/label/border via alpha multiplicacao.
        let bg_final = apply_alpha(bg_mod, opacity);
        let border_final = apply_alpha(self.border_color, opacity);
        let label_final = apply_alpha(label_mod, opacity);

        let mut inst = QuadInstance::new(
            center_ndc,
            size_ndc,
            bg_final,
            border_final,
            border_ndc,
            radius_ndc,
        );

        if self.shadow_color[3] > 0.0 {
            let offset_ndc = px_offset_to_ndc(
                self.shadow_offset_px[0],
                self.shadow_offset_px[1] * scale,
                viewport,
            );
            let shadow_r_ndc = px_to_ndc_radius(self.shadow_radius_px * scale, viewport[1]);
            let mut sc = self.shadow_color;
            sc[3] *= shadow_alpha_mod * opacity;
            inst = inst.with_shadow(sc, offset_ndc, shadow_r_ndc);
        }

        quads.push(inst);

        // Label: posicionado no centro visual (compensa scale). Hit area
        // usa o size original, mas o label desenha no rect visual.
        let label_x = position[0] + visual_offset[0] + self.padding[0] * scale;
        let label_y =
            position[1] + visual_offset[1] + self.padding[1] * scale - self.font_size * 0.1 * scale;

        text_r.queue_text(
            device,
            queue,
            &self.label,
            &TextStyle {
                size: self.font_size,
                color: label_final,
                family: self.font_family.clone(),
                weight: self.font_weight,
            },
            [label_x, label_y],
        );

        size
    }
}

// ============================================================================
// Helpers de modulacao visual por state
// ============================================================================

/// Aplica opacity multiplicando o canal alpha.
fn apply_alpha(c: [f32; 4], alpha: f32) -> [f32; 4] {
    [c[0], c[1], c[2], c[3] * alpha]
}

/// Multiplica canais RGB por `factor` (em linear space, ja que cores aqui
/// sao linear). Clampa a 1.0. Alpha intacto.
fn brighten(c: [f32; 4], factor: f32) -> [f32; 4] {
    [
        (c[0] * factor).min(1.0),
        (c[1] * factor).min(1.0),
        (c[2] * factor).min(1.0),
        c[3],
    ]
}

/// Calcula visual modulado por estado + progresso de anim.
///
/// Retorna `(bg, label, scale, shadow_alpha_mod, opacity)`.
///
/// Regras:
/// - Idle      : bg / label originais, scale 1.0, shadow 1.0
/// - Hover     : bg * 1.10 (brightness), scale 1.0, shadow 1.1 (leve eleva)
/// - Pressed   : bg * 1.05 (entre idle/hover), scale 0.96, shadow 0.6 (afunda)
/// - Disabled  : opacity 0.5
///
/// Interpola entre Hover e Pressed via `press_progress` (0..1, Spring driven).
fn visual_for_state(
    state: WidgetState,
    press_progress: f32,
    bg: [f32; 4],
    label: [f32; 4],
) -> ([f32; 4], [f32; 4], f32, f32, f32) {
    if state == WidgetState::Disabled {
        return (bg, label, 1.0, 1.0, 0.5);
    }

    // Hover bg = bg * 1.10. Pressed bg = bg * 1.05. Interpolacao por progress.
    let hover_factor = if state == WidgetState::Idle && press_progress < 0.01 {
        1.0
    } else {
        // Anti-flash: quando spring volta pra Idle, mantem leve hover ate
        // chegar perto de 0.
        1.0 + 0.10 * (1.0 - press_progress)
    };
    let pressed_factor = 1.05;
    let bg_factor = hover_factor * (1.0 - press_progress) + pressed_factor * press_progress;

    let bg_mod = brighten(bg, bg_factor);

    // Scale: 1.0 -> 0.96 conforme press_progress sobe.
    let scale = 1.0 - 0.04 * press_progress;

    // Shadow: 1.0 (Idle/Hover) -> 0.6 (Pressed). Botao "afunda".
    let shadow_mod = 1.0 - 0.4 * press_progress;

    // Label: mantem mesmo a cor (so muda em Disabled, ja tratado acima).
    (bg_mod, label, scale, shadow_mod, 1.0)
}

// ============================================================================
// Tests
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::MouseButton as Mb;

    fn ev_move(x: f32, y: f32) -> LumoEvent {
        LumoEvent::PointerMove { position: [x, y] }
    }
    fn ev_press(x: f32, y: f32) -> LumoEvent {
        LumoEvent::PointerPress {
            position: [x, y],
            button: Mb::Left,
        }
    }
    fn ev_release(x: f32, y: f32) -> LumoEvent {
        LumoEvent::PointerRelease {
            position: [x, y],
            button: Mb::Left,
        }
    }

    #[test]
    fn rect_contains_inclusive() {
        let r = Rect::new(10.0, 20.0, 100.0, 50.0);
        assert!(r.contains([10.0, 20.0]));
        assert!(r.contains([110.0, 70.0]));
        assert!(r.contains([50.0, 40.0]));
        assert!(!r.contains([9.9, 40.0]));
        assert!(!r.contains([50.0, 70.1]));
    }

    #[test]
    fn handle_idle_to_hover_on_enter() {
        let mut h = ButtonHandle::new();
        h.set_rect(Rect::new(0.0, 0.0, 100.0, 50.0));
        let clicked = h.handle_event(&ev_move(10.0, 10.0));
        assert!(!clicked);
        assert_eq!(h.state, WidgetState::Hover);
    }

    #[test]
    fn handle_click_cycle_emits_click() {
        let mut h = ButtonHandle::new();
        h.set_rect(Rect::new(0.0, 0.0, 100.0, 50.0));
        h.handle_event(&ev_move(10.0, 10.0));
        assert_eq!(h.state, WidgetState::Hover);
        h.handle_event(&ev_press(10.0, 10.0));
        assert_eq!(h.state, WidgetState::Pressed);
        let clicked = h.handle_event(&ev_release(10.0, 10.0));
        assert!(clicked);
        assert_eq!(h.state, WidgetState::Hover);
    }

    #[test]
    fn handle_release_outside_no_click() {
        let mut h = ButtonHandle::new();
        h.set_rect(Rect::new(0.0, 0.0, 100.0, 50.0));
        h.handle_event(&ev_move(10.0, 10.0));
        h.handle_event(&ev_press(10.0, 10.0));
        let clicked = h.handle_event(&ev_release(500.0, 500.0));
        assert!(!clicked);
        // release fora cancela o press: state vai pra Idle (sem hover, pois cursor fora).
        assert_eq!(h.state, WidgetState::Idle);
    }

    #[test]
    fn handle_drag_out_cancels_press() {
        let mut h = ButtonHandle::new();
        h.set_rect(Rect::new(0.0, 0.0, 100.0, 50.0));
        h.handle_event(&ev_move(10.0, 10.0));
        h.handle_event(&ev_press(10.0, 10.0));
        h.handle_event(&ev_move(500.0, 500.0));
        assert_eq!(h.state, WidgetState::Idle);
    }

    #[test]
    fn disabled_ignores_events() {
        let mut h = ButtonHandle::disabled();
        h.set_rect(Rect::new(0.0, 0.0, 100.0, 50.0));
        h.handle_event(&ev_move(10.0, 10.0));
        h.handle_event(&ev_press(10.0, 10.0));
        let clicked = h.handle_event(&ev_release(10.0, 10.0));
        assert!(!clicked);
        assert_eq!(h.state, WidgetState::Disabled);
    }

    #[test]
    fn press_progress_animates_toward_one() {
        let mut h = ButtonHandle::new();
        h.set_rect(Rect::new(0.0, 0.0, 100.0, 50.0));
        h.handle_event(&ev_move(10.0, 10.0));
        h.handle_event(&ev_press(10.0, 10.0));
        // Tick 120 frames (~2s).
        for _ in 0..120 {
            h.update(1.0 / 60.0);
        }
        assert!(
            (h.press_progress - 1.0).abs() < 0.05,
            "press_progress nao chegou em 1.0: {}",
            h.press_progress
        );
    }
}
