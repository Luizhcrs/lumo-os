//! # lumo-beam
//!
//! Proposito: wgpu device/surface/queue wrapper. Primitivo triangle e uniforms globais.
//!
//! ## Invariantes
//!     - LBDevice e o unico dono do wgpu Device/Queue — nunca clonar Arc pra escopo mais longo que o renderer.
//!     - LBGlobalUniforms e updated a cada frame antes de qualquer draw call.
//!
//! ## Memory refs
//!     - [[feedback-design-lapidado]]
//!     - [[project-lumo-os]]

use bytemuck::{Pod, Zeroable};
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::{dpi::PhysicalSize, window::Window};

// ---------------------------------------------------------------------------
// Clear colors -- linear-space wgpu::Color (drop-in pra LoadOp::Clear).
// ---------------------------------------------------------------------------

/// Clear color do compositor (INK_DEEP em escala wgpu::Color, linear).
pub const INK_DEEP: wgpu::Color = wgpu::Color {
    r: 0.003_035_3,
    g: 0.003_035_3,
    b: 0.003_676_5,
    a: 1.0,
};

/// Clear color pearl (`#f5f5f7`) em linear -- usado em demos de fundo claro.
pub const PEARL_CLEAR: wgpu::Color = wgpu::Color {
    r: 0.913_098_6,
    g: 0.913_098_6,
    b: 0.930_111_0,
    a: 1.0,
};

/// Compat shim Layer 4.1 (`EMERALD_600` rgb3 para o triangle vertex).
pub const EMERALD_600: [f32; 3] = [0.001_517_6, 0.304_987_3, 0.141_263_3];

// ---------------------------------------------------------------------------
// LBVertex -- triangle primitive vertex (position + color)
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct LBVertex {
    pub position: [f32; 2],
    pub color: [f32; 3],
}

impl LBVertex {
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

/// Alias retro-compat. Prefira `LBVertex`.
pub type Vertex = LBVertex;

pub const TRIANGLE_VERTICES: &[LBVertex] = &[
    LBVertex {
        position: [0.0, 0.5],
        color: EMERALD_600,
    },
    LBVertex {
        position: [-0.5, -0.5],
        color: EMERALD_600,
    },
    LBVertex {
        position: [0.5, -0.5],
        color: EMERALD_600,
    },
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
// LBGlobalUniforms -- uniform global por frame
// ---------------------------------------------------------------------------

/// Uniforms globais por frame (bind group 0 binding 0).
///
/// O shader le `viewport_size` para derivar AA pixel-precise, e `time` esta
/// reservado para animacoes futuras (e.g. hover pulse, shimmer em loading).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable, Default)]
pub struct LBGlobalUniforms {
    /// Tamanho do viewport em pixels fisicos (width, height).
    pub viewport_size: [f32; 2],
    /// Segundos desde o start do app (anim source).
    pub time: f32,
    /// Padding para alinhar a 16 bytes (wgpu requer).
    pub _pad: f32,
}

/// Alias retro-compat. Prefira `LBGlobalUniforms`.
pub type GlobalUniforms = LBGlobalUniforms;

// ---------------------------------------------------------------------------
// LBDevice -- wgpu device + surface + triangle pipeline
// ---------------------------------------------------------------------------

/// Renderer base do Lumo OS. Mantem instance/adapter/device/queue/surface
/// + um pipeline triangle pra smoke test. Apps acima compoe outros
/// renderers (quad, text, glyph) usando esse device.
pub struct LBDevice {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    num_vertices: u32,
    _window: Arc<Window>,
}

/// Alias retro-compat. Prefira `LBDevice`.
pub type Renderer = LBDevice;

impl LBDevice {
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
                    label: Some("lumo-beam::device"),
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
            label: Some("lumo-beam::triangle-shader"),
            source: wgpu::ShaderSource::Wgsl(TRIANGLE_SHADER_SRC.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lumo-beam::triangle-pipeline-layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("lumo-beam::triangle-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[LBVertex::layout()],
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
            label: Some("lumo-beam::triangle-vbo"),
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
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("lumo-beam::encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("lumo-beam::main-pass"),
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
