//! lumo-gfx-core
//!
//! Framework grafico do Lumo OS (Layer 4.1, 4.1.5, 4.1.6, 4.1.7 e 4.1.8).
//! Backend: wgpu (cross-platform sobre Vulkan/Metal/DX12).
//! Em Layer 4.2 trocamos wgpu por Vulkan raw via `ash`.
//!
//! Sub-fases entregues:
//! - 4.1   (this lib.rs): Renderer + Vertex + triangle primitive
//! - 4.1.5 (this lib.rs): QuadRenderer com SDF rounded corners + border instanced
//! - 4.1.6 (this lib.rs): viewport uniform + drop shadow + AA pixel-precise via `fwidth`
//! - 4.1.7 (text.rs):     cosmic-text + atlas R8 + glyph instanced
//! - 4.1.8 (widget.rs):   Button widget primitive (quad + text composto)
//!
//! # Color space gotcha (Layer 4.1.8)
//!
//! O surface do wgpu usa `Bgra8UnormSrgb` (sRGB). Isso significa que o
//! hardware aplica `linear -> sRGB` automaticamente ao escrever o fragment
//! color no framebuffer. Portanto **as cores enviadas ao shader devem estar
//! em linear space**, nao em sRGB. Antes do A5.5 estavamos passando cores
//! sRGB (`0xRR / 255.0`), o que causava double-gamma e tornava o texto
//! emerald-600 quase branco.
//!
//! As constantes em `color::*` ja saem **linearizadas** (pre-computadas
//! offline). Para casos de runtime (e.g. parsing de hex do usuario), use
//! `color::srgb_to_linear`.

use bytemuck::{Pod, Zeroable};
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::{dpi::PhysicalSize, window::Window};

// Text rendering (Layer 4.1.7, paralelo A5.4). Modulo separado pra nao
// poluir o lib.rs principal; a API publica esta em `text::*`.
pub mod text;

// Widgets (Layer 4.1.8). Primeiro widget: Button. Composto por quad +
// label de texto. API publica em `widget::*`.
pub mod widget;

// Input (Layer 4.1.9): event types + winit bridge.
pub mod input;

// Animation (Layer 4.1.9): Spring physics.
pub mod anim;

// ---------------------------------------------------------------------------
// Color tokens (single source of truth, Layer 4.1.5 + 4.1.6 expansion)
// ---------------------------------------------------------------------------

/// Tokens de cor do Lumo OS.
///
/// **Importante (Layer 4.1.8)**: os valores em `[f32; 4]` aqui ja estao em
/// **linear space**, prontos para serem escritos por um shader cuja saida
/// alimenta um surface sRGB (que aplica a curva inversa no hardware).
///
/// As versoes `_SRGB` preservam os valores nominais do design system
/// (`hex / 255.0`) para debug / referencia / parsing humano. Use
/// `srgb_to_linear` quando precisar converter um valor sRGB runtime.
pub mod color {
    // -- sRGB references (design system originals; debug / reference) -----------
    /// `#0a0a0c` ink deep — sRGB normalizado.
    pub const INK_DEEP_SRGB:    [f32; 4] = [0.039_215_688, 0.039_215_688, 0.047_058_82, 1.0];
    /// `#1a1a21` panel-hi — sRGB normalizado.
    pub const PANEL_HI_SRGB:    [f32; 4] = [0.101_960_786, 0.101_960_786, 0.129_411_77, 1.0];
    /// `#131318` panel — sRGB normalizado.
    pub const PANEL_SRGB:       [f32; 4] = [0.074_509_805, 0.074_509_805, 0.094_117_65, 1.0];
    /// `#059669` emerald-600 — sRGB normalizado.
    pub const EMERALD_600_SRGB: [f32; 4] = [0.019_607_844, 0.588_235_3, 0.411_764_7, 1.0];
    /// `#10b981` emerald-500 — sRGB normalizado.
    pub const EMERALD_500_SRGB: [f32; 4] = [0.062_745_1, 0.725_490_2, 0.505_882_36, 1.0];
    /// `#f5f5f7` pearl — sRGB normalizado.
    pub const PEARL_SRGB:       [f32; 4] = [0.960_784_3, 0.960_784_3, 0.968_627_5, 1.0];
    /// `#9596a0` muted — sRGB normalizado.
    pub const MUTED_SRGB:       [f32; 4] = [0.585_0, 0.586_0, 0.627_0, 1.0];
    /// `#f87171` danger (red-400) — sRGB normalizado.
    pub const DANGER_SRGB:      [f32; 4] = [0.972_549, 0.443_137, 0.443_137, 1.0];

    // -- Linear (GPU-ready) -----------------------------------------------------
    // Pre-computado offline via `srgb_to_linear(c)` para evitar runtime cost
    // por frame e manter `const`. Se algum hex mudar, recalcular e atualizar.

    /// `#0a0a0c` ink deep (background do shell) — linear.
    pub const INK_DEEP:    [f32; 4] = [0.003_035_3, 0.003_035_3, 0.003_676_5, 1.0];
    /// `#1a1a21` panel-hi (cards, surfaces elevadas) — linear.
    pub const PANEL_HI:    [f32; 4] = [0.010_329_8, 0.010_329_8, 0.015_208_5, 1.0];
    /// `#131318` panel base (surface neutra) — linear.
    pub const PANEL:       [f32; 4] = [0.006_512_1, 0.006_512_1, 0.009_134_1, 1.0];
    /// `#059669` emerald-600 (accent primario) — linear.
    pub const EMERALD_600: [f32; 4] = [0.001_517_6, 0.304_987_3, 0.141_263_3, 1.0];
    /// `#10b981` emerald-500 (accent secundario / hover) — linear.
    pub const EMERALD_500: [f32; 4] = [0.005_181_5, 0.485_149_9, 0.219_526_2, 1.0];
    /// `#f5f5f7` quasi-white (texto, borders fortes) — linear.
    pub const PEARL:       [f32; 4] = [0.913_098_6, 0.913_098_6, 0.930_111_0, 1.0];
    /// `#9596a0` muted (text de baixa enfase) — linear.
    pub const MUTED:       [f32; 4] = [0.301_318_7, 0.302_449_8, 0.350_975_3, 1.0];
    /// `#f87171` danger / red-400 — linear.
    pub const DANGER:      [f32; 4] = [0.938_685_7, 0.165_132_2, 0.165_132_2, 1.0];

    /// Transparente puro.
    pub const TRANSPARENT: [f32; 4] = [0.0, 0.0, 0.0, 0.0];

    // -- shadow tokens (Layer 4.1.6) ------------------------------------------
    /// Sombra preta neutra (drop shadow padrao de cards). RGB linear (0,0,0)
    /// dispensa conversao; alpha controla intensidade.
    pub const SHADOW_BLACK: [f32; 4] = [0.0, 0.0, 0.0, 0.4];
    /// Sombra accent translucida (cards emerald, glow controlado) — RGB linear.
    pub const SHADOW_ACCENT: [f32; 4] = [0.001_517_0, 0.304_947_2, 0.141_288_9, 0.3];
    /// Sombra danger leve (botoes destrutivos) — RGB linear.
    pub const SHADOW_DANGER: [f32; 4] = [0.938_685_7, 0.165_132_2, 0.165_132_2, 0.25];

    // -- conversion helpers ---------------------------------------------------
    /// Converte 1 canal sRGB normalizado (0..1) para linear (0..1).
    /// Curva oficial IEC 61966-2-1. Usar em runtime quando carregar cor
    /// de usuario (hex picker, theme override, etc.).
    pub fn srgb_to_linear_channel(c: f32) -> f32 {
        if c <= 0.040_45 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    /// Converte uma cor sRGB `[f32; 4]` para linear (alpha passa intacto).
    /// As constantes acima ja sao o resultado de `srgb_to_linear(*_SRGB)`.
    pub fn srgb_to_linear(s: [f32; 4]) -> [f32; 4] {
        [
            srgb_to_linear_channel(s[0]),
            srgb_to_linear_channel(s[1]),
            srgb_to_linear_channel(s[2]),
            s[3],
        ]
    }
}

/// Clear color do compositor (INK_DEEP em escala wgpu::Color, linear).
/// `LoadOp::Clear` pula a curva sRGB do surface (o clear vai direto pro
/// framebuffer sRGB), entao precisamos passar linear igual ao que o shader
/// passa em `color::INK_DEEP`.
pub const INK_DEEP: wgpu::Color = wgpu::Color {
    r: 0.003_035_3,
    g: 0.003_035_3,
    b: 0.003_676_5,
    a: 1.0,
};

/// Clear color pearl (`#f5f5f7`) em linear — usado em demos de fundo claro
/// como `quad-shadow` e `button-demo`.
pub const PEARL_CLEAR: wgpu::Color = wgpu::Color {
    r: 0.913_098_6,
    g: 0.913_098_6,
    b: 0.930_111_0,
    a: 1.0,
};

/// Compat shim para Layer 4.1 (callers antigos importam de raiz). Linear.
pub const EMERALD_600: [f32; 3] = [0.001_517_6, 0.304_987_3, 0.141_263_3];

// ---------------------------------------------------------------------------
// Pixel <-> NDC helpers (Layer 4.1.8)
// ---------------------------------------------------------------------------
//
// O QuadRenderer trabalha em NDC `[-1..+1]`. Widgets / demos pensam em
// pixels top-left. Estas funcoes encapsulam a conversao para nao ser
// re-implementada em cada bin. `viewport` e o tamanho do canvas em pixels.

/// Converte um tamanho (largura, altura) em pixels para **half-size** em NDC.
/// `QuadInstance::new` aceita size completo, entao multiplique por 2 ao usar.
pub fn px_size_to_ndc(w_px: f32, h_px: f32, viewport: [f32; 2]) -> [f32; 2] {
    [w_px / (viewport[0] * 0.5), h_px / (viewport[1] * 0.5)]
}

/// Converte um centro em pixels (origem top-left, y cresce pra baixo) em NDC.
pub fn px_center_to_ndc(cx_px: f32, cy_px: f32, viewport: [f32; 2]) -> [f32; 2] {
    let x = (cx_px / (viewport[0] * 0.5)) - 1.0;
    let y = 1.0 - (cy_px / (viewport[1] * 0.5));
    [x, y]
}

/// Converte um offset CSS-style (positivo = direita/baixo) para NDC.
pub fn px_offset_to_ndc(dx_px: f32, dy_px: f32, viewport: [f32; 2]) -> [f32; 2] {
    [dx_px / (viewport[0] * 0.5), dy_px / (viewport[1] * 0.5)]
}

/// Converte um raio (border / corner / shadow) em pixels para NDC. Usa o
/// eixo Y como base — eixo curto em telas 4:3 / 16:9 — mantendo a "feel"
/// consistente com unidades CSS px.
pub fn px_to_ndc_radius(px: f32, viewport_height: f32) -> f32 {
    px / (viewport_height * 0.5)
}

// ---------------------------------------------------------------------------
// Triangle primitive (Layer 4.1) -- mantido para regressao visual e A/B
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
    pub color: [f32; 3],
}

impl Vertex {
    pub const ATTRIBS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![
        0 => Float32x2,
        1 => Float32x3,
    ];

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

pub const TRIANGLE_VERTICES: &[Vertex] = &[
    Vertex { position: [0.0, 0.5], color: EMERALD_600 },
    Vertex { position: [-0.5, -0.5], color: EMERALD_600 },
    Vertex { position: [0.5, -0.5], color: EMERALD_600 },
];

const TRIANGLE_SHADER_SRC: &str = r#"
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(in.position, 0.0, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}
"#;

// ---------------------------------------------------------------------------
// Quad primitive (Layer 4.1.5 + 4.1.6)
// ---------------------------------------------------------------------------

/// Vertice de quad unitario. 4 vertices descrevem o retangulo `[-0.5..+0.5]`
/// em coordenadas locais; o shader expande para size/center via instance.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct QuadVertex {
    /// Posicao local `[-0.5..+0.5]` em ambos eixos.
    pub local_pos: [f32; 2],
    /// UV `[0..1]` correspondente.
    pub uv: [f32; 2],
}

impl QuadVertex {
    pub const ATTRIBS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![
        0 => Float32x2,
        1 => Float32x2,
    ];

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Quad unitario em torno da origem. O bounding e expandido no vertex shader
/// para acomodar a sombra (offset + radius), entao mantemos 4 vertices puros.
pub const QUAD_VERTICES: &[QuadVertex] = &[
    QuadVertex { local_pos: [-0.5, -0.5], uv: [0.0, 1.0] },
    QuadVertex { local_pos: [ 0.5, -0.5], uv: [1.0, 1.0] },
    QuadVertex { local_pos: [ 0.5,  0.5], uv: [1.0, 0.0] },
    QuadVertex { local_pos: [-0.5,  0.5], uv: [0.0, 0.0] },
];

/// Indices triangle-list para o quad unitario (2 tris).
pub const QUAD_INDICES: &[u16] = &[0, 1, 2, 0, 2, 3];

/// Uniforms globais por frame (bind group 0 binding 0).
///
/// Em Layer 4.1.6 o shader le `viewport_size` para derivar AA pixel-precise
/// quando `fwidth` nao bastar, e `time` esta reservado para animacoes futuras
/// (e.g. hover pulse, shimmer em loading).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable, Default)]
pub struct GlobalUniforms {
    /// Tamanho do viewport em pixels fisicos (width, height).
    pub viewport_size: [f32; 2],
    /// Segundos desde o start do app (anim source).
    pub time: f32,
    /// Padding para alinhar a 16 bytes (wgpu requer).
    pub _pad: f32,
}

/// Uma instancia de quad: vai como `VertexStepMode::Instance` no pipeline.
/// Tamanho 96 bytes, alinhado a 16 (requisito wgpu).
///
/// Layer 4.1.6 estende com 4 campos extras (`shadow_*`) mantendo compat
/// binaria via `QuadInstance::new` (zera shadow) e `with_shadow` (helper).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct QuadInstance {
    /// Centro em NDC `[-1..+1]`.
    pub center: [f32; 2],
    /// Half-size (largura/2, altura/2) em NDC.
    pub half_size: [f32; 2],
    /// Cor de fill RGBA.
    pub bg: [f32; 4],
    /// Cor de borda RGBA. Alpha 0 desabilita borda.
    pub border: [f32; 4],
    /// Largura da borda em NDC.
    pub border_width: f32,
    /// Raio das quinas em NDC.
    pub corner_radius: f32,
    /// Padding para alinhar bloco intermediario.
    pub _pad0: [f32; 2],
    /// Cor da sombra RGBA. Alpha 0 desabilita sombra.
    pub shadow_color: [f32; 4],
    /// Offset da sombra em NDC (positivo = direita/baixo no eixo NDC).
    pub shadow_offset: [f32; 2],
    /// Spread/blur da sombra em NDC.
    pub shadow_radius: f32,
    /// Padding final 16-byte alignment.
    pub _pad1: f32,
}

impl Default for QuadInstance {
    fn default() -> Self {
        Self {
            center: [0.0, 0.0],
            half_size: [0.0, 0.0],
            bg: color::TRANSPARENT,
            border: color::TRANSPARENT,
            border_width: 0.0,
            corner_radius: 0.0,
            _pad0: [0.0, 0.0],
            shadow_color: color::TRANSPARENT,
            shadow_offset: [0.0, 0.0],
            shadow_radius: 0.0,
            _pad1: 0.0,
        }
    }
}

impl QuadInstance {
    pub const ATTRIBS: [wgpu::VertexAttribute; 10] = wgpu::vertex_attr_array![
        2  => Float32x2, // center
        3  => Float32x2, // half_size
        4  => Float32x4, // bg
        5  => Float32x4, // border
        6  => Float32,   // border_width
        7  => Float32,   // corner_radius
        8  => Float32x2, // _pad0 (ignored by shader)
        9  => Float32x4, // shadow_color
        10 => Float32x2, // shadow_offset
        11 => Float32,   // shadow_radius
    ];

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }

    /// Construtor compacto sem sombra (compat com call-sites Layer 4.1.5).
    pub fn new(
        center: [f32; 2],
        size: [f32; 2],
        bg: [f32; 4],
        border: [f32; 4],
        border_width: f32,
        corner_radius: f32,
    ) -> Self {
        Self {
            center,
            half_size: [size[0] * 0.5, size[1] * 0.5],
            bg,
            border,
            border_width,
            corner_radius,
            _pad0: [0.0, 0.0],
            shadow_color: color::TRANSPARENT,
            shadow_offset: [0.0, 0.0],
            shadow_radius: 0.0,
            _pad1: 0.0,
        }
    }

    /// Builder: adiciona drop shadow sobre uma instancia existente.
    pub fn with_shadow(
        mut self,
        color: [f32; 4],
        offset: [f32; 2],
        radius: f32,
    ) -> Self {
        self.shadow_color = color;
        self.shadow_offset = offset;
        self.shadow_radius = radius;
        self
    }
}

const QUAD_SHADER_SRC: &str = r#"
struct Globals {
    viewport_size: vec2<f32>,
    time: f32,
    _pad: f32,
};

@group(0) @binding(0) var<uniform> globals: Globals;

struct VsIn {
    @location(0)  local_pos: vec2<f32>,
    @location(1)  uv: vec2<f32>,
    @location(2)  center: vec2<f32>,
    @location(3)  half_size: vec2<f32>,
    @location(4)  bg: vec4<f32>,
    @location(5)  border: vec4<f32>,
    @location(6)  border_width: f32,
    @location(7)  corner_radius: f32,
    @location(8)  _pad0: vec2<f32>,
    @location(9)  shadow_color: vec4<f32>,
    @location(10) shadow_offset: vec2<f32>,
    @location(11) shadow_radius: f32,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) half_size: vec2<f32>,
    @location(2) bg: vec4<f32>,
    @location(3) border: vec4<f32>,
    @location(4) border_width: f32,
    @location(5) corner_radius: f32,
    @location(6) shadow_color: vec4<f32>,
    @location(7) shadow_offset: vec2<f32>,
    @location(8) shadow_radius: f32,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    // Expansao do bounding pra acomodar shadow: max(|offset| + radius) em
    // cada eixo. Sem expandir, fragmentos da sombra que vazam fora do quad
    // original seriam clipados.
    let extra_x = abs(in.shadow_offset.x) + in.shadow_radius;
    let extra_y = abs(in.shadow_offset.y) + in.shadow_radius;
    let expanded_half = in.half_size + vec2<f32>(extra_x, extra_y);

    var out: VsOut;
    let world = in.center + in.local_pos * (expanded_half * 2.0);
    out.clip = vec4<f32>(world, 0.0, 1.0);
    // `local` representa a posicao em coords do quad original (centro 0,0)
    // mesmo quando o vertex foi expandido pra shadow.
    out.local = in.local_pos * (expanded_half * 2.0);
    out.half_size = in.half_size;
    out.bg = in.bg;
    out.border = in.border;
    out.border_width = in.border_width;
    out.corner_radius = in.corner_radius;
    out.shadow_color = in.shadow_color;
    out.shadow_offset = in.shadow_offset;
    out.shadow_radius = in.shadow_radius;
    return out;
}

// SDF de retangulo arredondado centrado na origem (Inigo Quilez).
fn sdf_rounded_box(p: vec2<f32>, half: vec2<f32>, radius: f32) -> f32 {
    let q = abs(p) - half + vec2<f32>(radius, radius);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0, 0.0))) - radius;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let r = clamp(in.corner_radius, 0.0, min(in.half_size.x, in.half_size.y));

    // --- shadow pass (drawn below the quad) ---------------------------------
    // Inverte o sinal do offset Y: NDC sobe -Y vai pra cima, mas a API expoe
    // shadow_offset "positivo = baixo" (convencao CSS). Aqui o eixo da
    // amostragem precisa subtrair offset com sinal invertido pra que valores
    // positivos de Y "empurrem" a sombra pra baixo da view.
    let shadow_p = in.local - vec2<f32>(in.shadow_offset.x, -in.shadow_offset.y);
    let d_shadow = sdf_rounded_box(shadow_p, in.half_size, r);
    // smoothstep do shadow_radius (fora) ate 0 (borda). Quanto maior o radius,
    // mais difusa a sombra.
    let shadow_falloff = 1.0 - smoothstep(0.0, max(in.shadow_radius, 0.0001), d_shadow);
    let shadow_alpha = clamp(shadow_falloff, 0.0, 1.0) * in.shadow_color.a;

    // --- quad pass ----------------------------------------------------------
    let d_quad = sdf_rounded_box(in.local, in.half_size, r);
    // AA pixel-precise: largura da transicao = 1 pixel (derivative). fwidth
    // varia com o resolution; quando o frag esta longe da borda, fwidth->0
    // e o smoothstep degenera pra step (sem AA penalidade). Multiplicador
    // 0.7 reduz "fuzziness" sem serrilhar.
    let aa = max(fwidth(d_quad) * 0.7, 0.0001);
    let outside = smoothstep(0.0, aa, d_quad);
    let shape_alpha = 1.0 - outside;

    let bw = max(in.border_width, 0.0);
    let border_t = smoothstep(-bw - aa, -bw + aa, d_quad);

    let bg_premul = in.bg * (1.0 - border_t);
    let border_premul = in.border * border_t;
    var quad_color = bg_premul + border_premul;
    let quad_alpha = quad_color.a * shape_alpha;

    // --- compose: sombra embaixo, quad em cima ------------------------------
    // Sombra so visivel onde o quad nao cobre. (1 - quad_alpha) e a janela
    // visivel da sombra. Multiplicado pela alpha do shadow_color.
    let shadow_visible = shadow_alpha * (1.0 - quad_alpha);
    let out_rgb = in.shadow_color.rgb * shadow_visible + quad_color.rgb * quad_alpha;
    let out_a = shadow_visible + quad_alpha;

    if (out_a <= 0.0) {
        discard;
    }
    return vec4<f32>(out_rgb, out_a);
}
"#;

/// Renderer instanced de quads com SDF rounded corners + drop shadow.
///
/// Buffer layout:
/// - `vertex_buffer`   : 4 `QuadVertex` (constante)
/// - `index_buffer`    : 6 `u16` (constante)
/// - `instance_buffer` : `capacity` * `QuadInstance` (resize sob demanda)
/// - `globals_buffer`  : 1 `GlobalUniforms` (atualizado por frame)
pub struct QuadRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    globals_buffer: wgpu::Buffer,
    globals_bind_group: wgpu::BindGroup,
    capacity: usize,
    pub instance_count: u32,
}

impl QuadRenderer {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        max_instances: usize,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("lumo-gfx-core::quad-shader"),
            source: wgpu::ShaderSource::Wgsl(QUAD_SHADER_SRC.into()),
        });

        let globals_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("lumo-gfx-core::quad-globals-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lumo-gfx-core::quad-pipeline-layout"),
            bind_group_layouts: &[&globals_bgl],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("lumo-gfx-core::quad-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[QuadVertex::layout(), QuadInstance::layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("lumo-gfx-core::quad-vbo"),
            contents: bytemuck::cast_slice(QUAD_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("lumo-gfx-core::quad-ibo"),
            contents: bytemuck::cast_slice(QUAD_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        let capacity = max_instances.max(1);
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lumo-gfx-core::quad-instances"),
            size: (capacity * std::mem::size_of::<QuadInstance>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lumo-gfx-core::quad-globals"),
            size: std::mem::size_of::<GlobalUniforms>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lumo-gfx-core::quad-globals-bg"),
            layout: &globals_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buffer.as_entire_binding(),
            }],
        });

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            instance_buffer,
            globals_buffer,
            globals_bind_group,
            capacity,
            instance_count: 0,
        }
    }

    /// Atualiza os dados das instancias. Realoca o buffer se exceder capacity.
    pub fn update_instances(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        instances: &[QuadInstance],
    ) {
        if instances.len() > self.capacity {
            let new_cap = instances.len().next_power_of_two();
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("lumo-gfx-core::quad-instances"),
                size: (new_cap * std::mem::size_of::<QuadInstance>()) as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.capacity = new_cap;
        }
        if !instances.is_empty() {
            queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(instances));
        }
        self.instance_count = instances.len() as u32;
    }

    /// Atualiza o uniform global (chamar uma vez por frame antes de `draw`).
    pub fn update_globals(&self, queue: &wgpu::Queue, globals: GlobalUniforms) {
        queue.write_buffer(&self.globals_buffer, 0, bytemuck::bytes_of(&globals));
    }

    /// Grava o draw call no render pass corrente.
    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if self.instance_count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.globals_bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        pass.draw_indexed(0..QUAD_INDICES.len() as u32, 0, 0..self.instance_count);
    }
}

// ---------------------------------------------------------------------------
// Renderer (Layer 4.1) -- orquestra clear + triangle. QuadRenderer e exposto
// separadamente para o demo `quad-gallery` montar sua propria cena.
// ---------------------------------------------------------------------------

pub struct Renderer {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    num_vertices: u32,
    _window: Arc<Window>,
}

impl Renderer {
    pub async fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();
        let size = PhysicalSize {
            width: size.width.max(1),
            height: size.height.max(1),
        };

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance
            .create_surface(window.clone())
            .expect("create_surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("no compatible GPU adapter found");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("lumo-gfx-core::device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults()
                        .using_resolution(adapter.limits()),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .expect("request_device");

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("lumo-gfx-core::triangle-shader"),
            source: wgpu::ShaderSource::Wgsl(TRIANGLE_SHADER_SRC.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lumo-gfx-core::triangle-pipeline-layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("lumo-gfx-core::triangle-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("lumo-gfx-core::triangle-vbo"),
            contents: bytemuck::cast_slice(TRIANGLE_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self {
            surface,
            device,
            queue,
            config,
            pipeline,
            vertex_buffer,
            num_vertices: TRIANGLE_VERTICES.len() as u32,
            _window: window,
        }
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let frame = self.surface.get_current_texture()?;
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("lumo-gfx-core::encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("lumo-gfx-core::main-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(INK_DEEP),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_pipeline(&self.pipeline);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.draw(0..self.num_vertices, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(())
    }
}
