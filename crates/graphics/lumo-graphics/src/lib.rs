//! # lumo-graphics
//!
//! Proposito: QuadRenderer instanced com SDF rounded corners, border e drop shadow.
//!
//! ## Invariantes
//! - Instancias enviadas por frame sao acumuladas e flushed em draw(); nao chamar draw() multiplas vezes sem flush.
//! - Sombras usam preto neutro (sem neon) — ver feedback_zero_neon_glow.
//!
//! ## Memory refs
//! - [[feedback-design-lapidado]]
//! - [[project-lumo-os]]

use bytemuck::{Pod, Zeroable};
use lumo_beam::LBGlobalUniforms;
use lumo_foundation::LFTokens;
use wgpu::util::DeviceExt;

// ---------------------------------------------------------------------------
// LGQuadVertex -- vertex local unitario
// ---------------------------------------------------------------------------

/// Vertice de quad unitario. 4 vertices descrevem o retangulo `[-0.5..+0.5]`
/// em coordenadas locais; o shader expande para size/center via instance.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct LGQuadVertex {
    /// Posicao local `[-0.5..+0.5]` em ambos eixos.
    pub local_pos: [f32; 2],
    /// UV `[0..1]` correspondente.
    pub uv: [f32; 2],
}

impl LGQuadVertex {
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

/// Alias retro-compat. Prefira `LGQuadVertex`.
pub type QuadVertex = LGQuadVertex;

/// Quad unitario em torno da origem. O bounding e expandido no vertex shader
/// para acomodar a sombra (offset + radius), entao mantemos 4 vertices puros.
pub const QUAD_VERTICES: &[LGQuadVertex] = &[
    LGQuadVertex { local_pos: [-0.5, -0.5], uv: [0.0, 1.0] },
    LGQuadVertex { local_pos: [ 0.5, -0.5], uv: [1.0, 1.0] },
    LGQuadVertex { local_pos: [ 0.5,  0.5], uv: [1.0, 0.0] },
    LGQuadVertex { local_pos: [-0.5,  0.5], uv: [0.0, 0.0] },
];

/// Indices triangle-list para o quad unitario (2 tris).
pub const QUAD_INDICES: &[u16] = &[0, 1, 2, 0, 2, 3];

// ---------------------------------------------------------------------------
// LGQuadInstance -- 1 quad descrito como instancia
// ---------------------------------------------------------------------------

/// Uma instancia de quad: vai como `VertexStepMode::Instance` no pipeline.
/// Tamanho 96 bytes, alinhado a 16 (requisito wgpu).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct LGQuadInstance {
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

/// Alias retro-compat. Prefira `LGQuadInstance`.
pub type QuadInstance = LGQuadInstance;

impl Default for LGQuadInstance {
    fn default() -> Self {
        Self {
            center: [0.0, 0.0],
            half_size: [0.0, 0.0],
            bg: LFTokens::TRANSPARENT,
            border: LFTokens::TRANSPARENT,
            border_width: 0.0,
            corner_radius: 0.0,
            _pad0: [0.0, 0.0],
            shadow_color: LFTokens::TRANSPARENT,
            shadow_offset: [0.0, 0.0],
            shadow_radius: 0.0,
            _pad1: 0.0,
        }
    }
}

impl LGQuadInstance {
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
            shadow_color: LFTokens::TRANSPARENT,
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

// ---------------------------------------------------------------------------
// LGQuadRenderer -- pipeline instanced de quad com SDF + shadow
// ---------------------------------------------------------------------------

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
    let extra_x = abs(in.shadow_offset.x) + in.shadow_radius;
    let extra_y = abs(in.shadow_offset.y) + in.shadow_radius;
    let expanded_half = in.half_size + vec2<f32>(extra_x, extra_y);

    var out: VsOut;
    let world = in.center + in.local_pos * (expanded_half * 2.0);
    out.clip = vec4<f32>(world, 0.0, 1.0);
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

fn sdf_rounded_box(p: vec2<f32>, half: vec2<f32>, radius: f32) -> f32 {
    let q = abs(p) - half + vec2<f32>(radius, radius);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0, 0.0))) - radius;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let r = clamp(in.corner_radius, 0.0, min(in.half_size.x, in.half_size.y));

    let shadow_p = in.local - vec2<f32>(in.shadow_offset.x, -in.shadow_offset.y);
    let d_shadow = sdf_rounded_box(shadow_p, in.half_size, r);
    let shadow_falloff = 1.0 - smoothstep(0.0, max(in.shadow_radius, 0.0001), d_shadow);
    let shadow_alpha = clamp(shadow_falloff, 0.0, 1.0) * in.shadow_color.a;

    let d_quad = sdf_rounded_box(in.local, in.half_size, r);
    let aa = max(fwidth(d_quad) * 0.7, 0.0001);
    let outside = smoothstep(0.0, aa, d_quad);
    let shape_alpha = 1.0 - outside;

    let bw = max(in.border_width, 0.0);
    let border_t = smoothstep(-bw - aa, -bw + aa, d_quad);

    let bg_premul = in.bg * (1.0 - border_t);
    let border_premul = in.border * border_t;
    var quad_color = bg_premul + border_premul;
    let quad_alpha = quad_color.a * shape_alpha;

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
/// - `vertex_buffer`   : 4 `LGQuadVertex` (constante)
/// - `index_buffer`    : 6 `u16` (constante)
/// - `instance_buffer` : `capacity` * `LGQuadInstance` (resize sob demanda)
/// - `globals_buffer`  : 1 `LBGlobalUniforms` (atualizado por frame)
pub struct LGQuadRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    globals_buffer: wgpu::Buffer,
    globals_bind_group: wgpu::BindGroup,
    capacity: usize,
    pub instance_count: u32,
}

/// Alias retro-compat. Prefira `LGQuadRenderer`.
pub type QuadRenderer = LGQuadRenderer;

impl LGQuadRenderer {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        max_instances: usize,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("lumo-graphics::quad-shader"),
            source: wgpu::ShaderSource::Wgsl(QUAD_SHADER_SRC.into()),
        });

        let globals_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("lumo-graphics::quad-globals-bgl"),
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
            label: Some("lumo-graphics::quad-pipeline-layout"),
            bind_group_layouts: &[&globals_bgl],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("lumo-graphics::quad-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[LGQuadVertex::layout(), LGQuadInstance::layout()],
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
            label: Some("lumo-graphics::quad-vbo"),
            contents: bytemuck::cast_slice(QUAD_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("lumo-graphics::quad-ibo"),
            contents: bytemuck::cast_slice(QUAD_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        let capacity = max_instances.max(1);
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lumo-graphics::quad-instances"),
            size: (capacity * std::mem::size_of::<LGQuadInstance>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lumo-graphics::quad-globals"),
            size: std::mem::size_of::<LBGlobalUniforms>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lumo-graphics::quad-globals-bg"),
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
        instances: &[LGQuadInstance],
    ) {
        if instances.len() > self.capacity {
            let new_cap = instances.len().next_power_of_two();
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("lumo-graphics::quad-instances"),
                size: (new_cap * std::mem::size_of::<LGQuadInstance>()) as wgpu::BufferAddress,
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
    pub fn update_globals(&self, queue: &wgpu::Queue, globals: LBGlobalUniforms) {
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
