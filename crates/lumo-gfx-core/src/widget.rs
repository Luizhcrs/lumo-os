//! Widgets (Layer 4.1.8).
//!
//! Primeiro widget Lumo de verdade: `Button`. Composto por:
//!   - 1 quad (background + corner radius + opcional border + drop shadow)
//!   - 1 run de texto centralizada no padding
//!
//! Stateless por enquanto: nao tem hover / press / focus (depende de input
//! handling que entra na Layer 4.1.9). Cada `render` recalcula tudo.
//!
//! Convencao de cor: todas as cores aqui sao **linear** (consistente com
//! `color::*`). Se vier sRGB de um theme runtime, converter via
//! `color::srgb_to_linear` antes.

use cosmic_text::{Attrs, Buffer, Family, Metrics, Shaping, Weight};

use crate::{
    color, px_center_to_ndc, px_offset_to_ndc, px_size_to_ndc, px_to_ndc_radius,
    text::{TextRenderer, TextStyle},
    QuadInstance,
};

/// Botao stateless: quad + label.
///
/// Construa via `Button::primary()` / `Button::ghost()` / `Button::danger()`
/// ou customize com builder methods. Chame `render` dentro do frame
/// (queue text + push quad instance via callbacks).
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

    /// Botao primario: bg emerald-600, label pearl (texto branco-quase em
    /// fundo verde escuro, contraste WCAG AA). Drop shadow preto leve.
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

    /// Botao ghost: bg transparente, border emerald-500 1px, label emerald.
    /// Sem shadow (visual minimalista).
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

    /// Botao destrutivo: bg danger, label pearl, shadow vermelho leve.
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

    pub fn with_shadow(
        mut self,
        color: [f32; 4],
        offset: [f32; 2],
        radius: f32,
    ) -> Self {
        self.shadow_color = color;
        self.shadow_offset_px = offset;
        self.shadow_radius_px = radius;
        self
    }

    // -- measure / layout -----------------------------------------------------

    /// Mede o tamanho total do botao em pixels (label + padding).
    ///
    /// Usa cosmic-text shaping de uma vez. Custo: 1 shape por chamada;
    /// para layouts estaticos faca cache no caller.
    pub fn measure(&self, text_r: &mut TextRenderer) -> [f32; 2] {
        let metrics = Metrics::new(self.font_size, self.font_size * 1.25);
        let mut buf = Buffer::new(text_r.font_system_mut(), metrics);
        let family = match self.font_family.to_lowercase().as_str() {
            "monospace" | "mono" => Family::Monospace,
            "serif" => Family::Serif,
            _ => Family::Name(&self.font_family),
        };
        let attrs = Attrs::new()
            .family(family)
            .weight(self.font_weight);
        buf.set_size(text_r.font_system_mut(), Some(4096.0), Some(self.font_size * 2.0));
        buf.set_text(text_r.font_system_mut(), &self.label, attrs, Shaping::Advanced);
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

    /// Acumula 1 `QuadInstance` (background+shadow+border) no `quads` e
    /// queia o label no `text_r`. O caller faz draw/flush no frame.
    ///
    /// `position` em pixels top-left. `viewport` em pixels (canvas size).
    ///
    /// Retorna o `[width, height]` calculado para o caller fazer layout.
    pub fn queue(
        &self,
        quads: &mut Vec<QuadInstance>,
        text_r: &mut TextRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        position: [f32; 2],
        viewport: [f32; 2],
    ) -> [f32; 2] {
        let size = self.measure(text_r);

        // Centro em pixels (top-left + half_size). Convertido pra NDC pelo
        // shader do QuadRenderer.
        let center_px = [
            position[0] + size[0] * 0.5,
            position[1] + size[1] * 0.5,
        ];
        let center_ndc = px_center_to_ndc(center_px[0], center_px[1], viewport);
        let size_ndc = px_size_to_ndc(size[0], size[1], viewport);
        // QuadInstance::new espera `size` (nao half_size) em NDC.
        let size_ndc_full = [size_ndc[0] * 2.0, size_ndc[1] * 2.0];

        let radius_ndc = px_to_ndc_radius(self.corner_radius_px, viewport[1]);
        let border_ndc = if self.border_px > 0.0 {
            px_to_ndc_radius(self.border_px, viewport[1])
        } else {
            0.0
        };

        let mut inst = QuadInstance::new(
            center_ndc,
            size_ndc_full,
            self.bg,
            self.border_color,
            border_ndc,
            radius_ndc,
        );

        if self.shadow_color[3] > 0.0 {
            let offset_ndc = px_offset_to_ndc(
                self.shadow_offset_px[0],
                self.shadow_offset_px[1],
                viewport,
            );
            let shadow_r_ndc = px_to_ndc_radius(self.shadow_radius_px, viewport[1]);
            inst = inst.with_shadow(self.shadow_color, offset_ndc, shadow_r_ndc);
        }

        quads.push(inst);

        // Label: posicionado no centro do botao. cosmic-text usa origem
        // top-left, com baseline derivado da linha. Aproximamos:
        // y = top + padding_v (cosmic ja inclui o ascent na primeira linha).
        let label_x = position[0] + self.padding[0];
        let label_y = position[1] + self.padding[1] - self.font_size * 0.1;

        text_r.queue_text(
            device,
            queue,
            &self.label,
            &TextStyle {
                size: self.font_size,
                color: self.label_color,
                family: self.font_family.clone(),
                weight: self.font_weight,
            },
            [label_x, label_y],
        );

        size
    }
}
