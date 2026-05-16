//! lumo-gfx-core
//!
//! Framework grafico do Lumo OS (Layer 4.1 e 4.1.5).
//! Backend: wgpu (cross-platform sobre Vulkan/Metal/DX12).
//! Em Layer 4.2 trocamos wgpu por Vulkan raw via `ash`.
//!
//! Sub-fases entregues:
//! - 4.1   (this lib.rs): Renderer + Vertex + triangle primitive
//! - 4.1.5 (this lib.rs): QuadRenderer com SDF rounded corners + border instanced

use bytemuck::{Pod, Zeroable};
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::{dpi::PhysicalSize, window::Window};

// ---------------------------------------------------------------------------
// Color tokens (single source of truth, Layer 4.1.5 expansion)
// ---------------------------------------------------------------------------

/// Tokens de cor do Lumo OS. Os valores estao em RGBA `[f32; 4]` ja prontos
/// para upload em buffers GPU. Hex de referencia entre parenteses; conversao
/// hex / 255 (sRGB nominal), o surface format sRGB do wgpu faz a curva final.
pub mod color {
    /// `#0a0a0c` ink deep (background do shell)
    pub const INK_DEEP: [f32; 4] = [0.039_215_688, 0.039_215_688, 0.047_058_82, 1.0];
    /// `#1a1a21` panel-hi (cards, surfaces elevadas)
    pub const PANEL_HI: [f32; 4] = [0.101_960_786, 0.101_960_786, 0.129_411_77, 1.0];
    /// `#059669` emerald-600 (accent primario)
    pub const EMERALD_600: [f32; 4] = [0.019_607_844, 0.588_235_3, 0.411_764_7, 1.0];
    /// `#10b981` emerald-500 (accent secundario / hover)
    pub const EMERALD_500: [f32; 4] = [0.062_745_1, 0.725_490_2, 0.505_882_36, 1.0];
    /// `#f5f5f7` quasi-white (texto, borders fortes)
    pub const PEARL: [f32; 4] = [0.960_784_3, 0.960_784_3, 0.968_627_5, 1.0];
    /// Transparente puro
    pub const TRANSPARENT: [f32; 4] = [0.0, 0.0, 0.0, 0.0];
}

/// Clear color do compositor (INK_DEEP em escala wgpu::Color).
/// Mantemos `wgpu::Color` separado de `color::INK_DEEP` porque o clear value
/// do attachment passa antes da curva sRGB do surface; estes numeros estao
/// pre-linearizados para combinar com o `color::INK_DEEP` no shader.
pub const INK_DEEP: wgpu::Color = wgpu::Color {
    r: 0.003_677,
    g: 0.003_677,
    b: 0.004_777,
    a: 1.0,
};

/// Compat shim para Layer 4.1 (callers antigos importam de raiz).
pub const EMERALD_600: [f32; 3] = [0.019_607_844, 0.588_235_3, 0.411_764_7];

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
// Quad primitive (Layer 4.1.5) -- instanced rounded rect via SDF
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

/// Quad unitario em torno da origem.
pub const QUAD_VERTICES: &[QuadVertex] = &[
    QuadVertex { local_pos: [-0.5, -0.5], uv: [0.0, 1.0] },
    QuadVertex { local_pos: [ 0.5, -0.5], uv: [1.0, 1.0] },
    QuadVertex { local_pos: [ 0.5,  0.5], uv: [1.0, 0.0] },
    QuadVertex { local_pos: [-0.5,  0.5], uv: [0.0, 0.0] },
];

/// Indices triangle-list para o quad unitario (2 tris).
pub const QUAD_INDICES: &[u16] = &[0, 1, 2, 0, 2, 3];

/// Uma instancia de quad: vai como `VertexStepMode::Instance` no pipeline.
/// Tamanho 64 bytes, alinhado a 16 (requisito wgpu).
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
    /// Padding para alinhar a 16 bytes.
    pub _pad: [f32; 2],
}

impl QuadInstance {
    pub const ATTRIBS: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
        2 => Float32x2, // center
        3 => Float32x2, // half_size
        4 => Float32x4, // bg
        5 => Float32x4, // border
        6 => Float32,   // border_width
        7 => Float32,   // corner_radius
    ];

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }

    /// Construtor curto para call-sites do demo.
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
            _pad: [0.0, 0.0],
        }
    }
}

const QUAD_SHADER_SRC: &str = r#"
struct VsIn {
    @location(0) local_pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) center: vec2<f32>,
    @location(3) half_size: vec2<f32>,
    @location(4) bg: vec4<f32>,
    @location(5) border: vec4<f32>,
    @location(6) border_width: f32,
    @location(7) corner_radius: f32,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) half_size: vec2<f32>,
    @location(2) bg: vec4<f32>,
    @location(3) border: vec4<f32>,
    @location(4) border_width: f32,
    @location(5) corner_radius: f32,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    let world = in.center + in.local_pos * (in.half_size * 2.0);
    out.clip = vec4<f32>(world, 0.0, 1.0);
    out.local = in.local_pos * (in.half_size * 2.0);
    out.half_size = in.half_size;
    out.bg = in.bg;
    out.border = in.border;
    out.border_width = in.border_width;
    out.corner_radius = in.corner_radius;
    return out;
}

// SDF de retangulo arredondado centrado na origem (Inigo Quilez).
fn sdf_rounded_box(p: vec2<f32>, half: vec2<f32>, radius: f32) -> f32 {
    let q = abs(p) - half + vec2<f32>(radius, radius);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0, 0.0))) - radius;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let aa = 0.0015;
    let r = clamp(in.corner_radius, 0.0, min(in.half_size.x, in.half_size.y));
    let d = sdf_rounded_box(in.local, in.half_size, r);

    let outside = smoothstep(0.0, aa, d);
    let shape_alpha = 1.0 - outside;

    let bw = max(in.border_width, 0.0);
    let border_t = smoothstep(-bw - aa, -bw + aa, d);

    let bg_premul = in.bg * (1.0 - border_t);
    let border_premul = in.border * border_t;
    var color = bg_premul + border_premul;
    color.a = color.a * shape_alpha;

    if (color.a <= 0.0) {
        discard;
    }
    return color;
}
"#;

/// Renderer instanced de quads com SDF rounded corners.
///
/// Buffer layout:
/// - `vertex_buffer`   : 4 `QuadVertex` (constante)
/// - `index_buffer`    : 6 `u16` (constante)
/// - `instance_buffer` : `capacity` * `QuadInstance` (resize sob demanda)
pub struct QuadRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lumo-gfx-core::quad-pipeline-layout"),
            bind_group_layouts: &[],
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

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            instance_buffer,
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

    /// Grava o draw call no render pass corrente.
    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if self.instance_count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
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
