//! # lumo-text
//!
//! Proposito: Text rendering via cosmic-text + atlas GPU 1024x1024.
//!
//! ## Invariantes
//! - FontSystem locked ANTES de SwashCache em qualquer callsite — ver I-03.
//! - Atlas overflow descarta glyph com log::warn (sem panic); crescimento de atlas nao implementado.
//!
//! ## Memory refs
//! - [[feedback-design-lapidado]]
//! - [[project-lumo-os]]

use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use cosmic_text::{
    Attrs, Buffer, CacheKey, Color as CtColor, Family, FontSystem, Metrics, Shaping, SwashCache,
    SwashContent, Weight,
};
use etagere::{size2, AllocId, Allocation, BucketedAtlasAllocator};
use wgpu::util::DeviceExt;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Estilo de uma run de texto. `family` aceita nomes do fontdb; se nao
/// existir cai no fallback do `cosmic-text` (SansSerif).
#[derive(Clone, Debug)]
pub struct TextStyle {
    pub size: f32,
    pub color: [f32; 4],
    pub family: String,
    pub weight: Weight,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            size: 16.0,
            color: [1.0, 1.0, 1.0, 1.0],
            family: "sans-serif".to_string(),
            weight: Weight::NORMAL,
        }
    }
}

// ---------------------------------------------------------------------------
// Internal types
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
struct AtlasEntry {
    /// UV top-left em [0..1].
    uv_origin: [f32; 2],
    /// UV size em [0..1].
    uv_size: [f32; 2],
    /// Tamanho do bitmap em pixels.
    px_size: [f32; 2],
    /// Offset do bitmap em pixels (do baseline / origem do glyph).
    px_offset: [f32; 2],
    /// Allocation id do etagere (para um futuro free; nao usado por ora).
    _alloc: AllocId,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct GlyphInstance {
    pos: [f32; 2],
    size: [f32; 2],
    uv_origin: [f32; 2],
    uv_size: [f32; 2],
    color: [f32; 4],
}

impl GlyphInstance {
    const ATTRIBS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
        0 => Float32x2, // pos
        1 => Float32x2, // size
        2 => Float32x2, // uv_origin
        3 => Float32x2, // uv_size
        4 => Float32x4, // color
    ];

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Uniforms {
    viewport: [f32; 2],
    _pad: [f32; 2],
}

const ATLAS_SIZE: u32 = 1024;
const INITIAL_INSTANCE_CAPACITY: usize = 512;

const TEXT_SHADER_SRC: &str = r#"
struct Uniforms {
    viewport: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var atlas_tex: texture_2d<f32>;
@group(0) @binding(2) var atlas_samp: sampler;

struct VsIn {
    @builtin(vertex_index) vid: u32,
    @location(0) pos: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) uv_origin: vec2<f32>,
    @location(3) uv_size: vec2<f32>,
    @location(4) color: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    // Quad triangle-list: 0,1,2,2,1,3 -> (0,0)(1,0)(0,1)(0,1)(1,0)(1,1)
    // Usamos triangle-strip 4 verts: 0=(0,0) 1=(1,0) 2=(0,1) 3=(1,1)
    var corners = array<vec2<f32>, 4>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
    );
    let c = corners[in.vid];

    let px = in.pos + c * in.size;
    // Pixel space -> NDC. Origem top-left, y cresce pra baixo.
    let ndc = vec2<f32>(
        (px.x / u.viewport.x) * 2.0 - 1.0,
        1.0 - (px.y / u.viewport.y) * 2.0,
    );

    var out: VsOut;
    out.clip = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = in.uv_origin + c * in.uv_size;
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let a = textureSample(atlas_tex, atlas_samp, in.uv).r;
    if (a <= 0.0) {
        discard;
    }
    return vec4<f32>(in.color.rgb, in.color.a * a);
}
"#;

// ---------------------------------------------------------------------------
// TextRenderer
// ---------------------------------------------------------------------------

pub struct TextRenderer {
    font_system: FontSystem,
    swash_cache: SwashCache,

    atlas_texture: wgpu::Texture,
    atlas_view: wgpu::TextureView,
    atlas_sampler: wgpu::Sampler,
    atlas_packer: BucketedAtlasAllocator,
    glyph_cache: HashMap<CacheKey, Option<AtlasEntry>>,

    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,

    uniforms_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,

    /// Instancias acumuladas no frame corrente. `render_text` faz push,
    /// `flush` envia ao GPU + draw.
    pending: Vec<GlyphInstance>,
    last_viewport: [f32; 2],
}

impl TextRenderer {
    pub fn new(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let font_system = FontSystem::new();
        let swash_cache = SwashCache::new();

        // --- atlas texture R8 ---
        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("lumo-text-atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("lumo-text-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let atlas_packer = BucketedAtlasAllocator::new(size2(ATLAS_SIZE as i32, ATLAS_SIZE as i32));

        // --- bind group layout (uniforms + atlas + sampler) ---
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("lumo-text-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let uniforms_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("lumo-text-uniforms"),
            contents: bytemuck::bytes_of(&Uniforms {
                viewport: [1.0, 1.0],
                _pad: [0.0, 0.0],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lumo-text-bg"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniforms_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });

        // --- pipeline ---
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("lumo-text-shader"),
            source: wgpu::ShaderSource::Wgsl(TEXT_SHADER_SRC.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lumo-text-pl"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("lumo-text-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[GlyphInstance::layout()],
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
                topology: wgpu::PrimitiveTopology::TriangleStrip,
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

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lumo-text-instances"),
            size: (INITIAL_INSTANCE_CAPACITY * std::mem::size_of::<GlyphInstance>())
                as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            font_system,
            swash_cache,
            atlas_texture,
            atlas_view,
            atlas_sampler,
            atlas_packer,
            glyph_cache: HashMap::new(),
            pipeline,
            bind_group_layout,
            bind_group,
            uniforms_buffer,
            instance_buffer,
            instance_capacity: INITIAL_INSTANCE_CAPACITY,
            pending: Vec::with_capacity(INITIAL_INSTANCE_CAPACITY),
            last_viewport: [1.0, 1.0],
        }
    }

    /// Acessor mutavel ao `FontSystem` interno. Usado por widgets (e.g.
    /// `widget::Button::measure`) que precisam fazer shaping antes de
    /// posicionar elementos. Mantemos privado o atlas / cache / pending
    /// e expomos so o shaping engine.
    pub fn font_system_mut(&mut self) -> &mut cosmic_text::FontSystem {
        &mut self.font_system
    }

    /// Shape a string com `cosmic-text` e empilha as instancias de glyph
    /// no buffer pendente. Chame `flush` para fazer o draw.
    pub fn queue_text(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        text: &str,
        style: &TextStyle,
        position: [f32; 2],
    ) {
        let metrics = Metrics::new(style.size, style.size * 1.25);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);

        let family = match style.family.to_lowercase().as_str() {
            "monospace" | "mono" => Family::Monospace,
            "serif" => Family::Serif,
            _ => {
                // Tenta nome especifico, cosmic-text faz fallback se nao achar.
                Family::Name(&style.family)
            }
        };
        // Cor NAO entra no Attrs (faz parte do CacheKey, causaria miss
        // a cada mudanca de cor). Cor vai via GlyphInstance::color no shader.
        let attrs = Attrs::new().family(family).weight(style.weight);

        buffer.set_size(&mut self.font_system, Some(4096.0), Some(style.size * 2.0));
        buffer.set_text(&mut self.font_system, text, attrs, Shaping::Advanced);
        buffer.shape_until_scroll(&mut self.font_system, false);

        // Baseline = position.y + ascent. cosmic-text ja entrega line_y como
        // top da linha; somamos para chegar na origem do shape.
        for run in buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                let physical = glyph.physical((0.0, 0.0), 1.0);
                let off_x = position[0].round();
                let off_y = position[1].round();

                let entry = self.cache_glyph(device, queue, physical.cache_key);
                let Some(entry) = entry else { continue };

                // physical.x/y e o "pen position" da origin do glyph em pixels.
                // Atlas offset corrige o canto top-left do bitmap.
                let px = off_x + physical.x as f32 + entry.px_offset[0];
                // run.line_y e o baseline da linha em coordenadas top-left
                // (cosmic-text), entao somamos.
                let py = off_y + physical.y as f32 + run.line_y + entry.px_offset[1];

                self.pending.push(GlyphInstance {
                    pos: [px, py],
                    size: entry.px_size,
                    uv_origin: entry.uv_origin,
                    uv_size: entry.uv_size,
                    color: style.color,
                });
            }
        }
    }

    /// Variante mais ergonomica: queue + flush num unico call.
    /// Util quando a UI desenha texto fora de um batch maior.
    #[allow(clippy::too_many_arguments)]
    pub fn render_text(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        text: &str,
        style: &TextStyle,
        position: [f32; 2],
        viewport: [f32; 2],
    ) {
        self.queue_text(device, queue, text, style, position);
        self.flush(device, queue, encoder, target, viewport);
    }

    /// Faz o draw das instancias acumuladas dentro de um render pass que
    /// **carrega** o conteudo anterior (LoadOp::Load). Use isto quando ja
    /// existe outro pass que limpou / desenhou o background.
    pub fn flush(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        viewport: [f32; 2],
    ) {
        if self.pending.is_empty() {
            return;
        }
        self.upload_uniforms(queue, viewport);
        self.upload_instances(device, queue);

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("lumo-text-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
            // 4 vertices (triangle-strip) x N instancias
            pass.draw(0..4, 0..self.pending.len() as u32);
        }

        self.pending.clear();
    }

    /// Mesmo que `flush` mas grava num pass ja aberto pelo chamador.
    /// O pass precisa estar com pipeline disponivel � esta funcao seta
    /// o seu proprio pipeline / bind group / vertex buffer.
    pub fn draw_into_pass<'a>(
        &'a mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pass: &mut wgpu::RenderPass<'a>,
        viewport: [f32; 2],
    ) {
        if self.pending.is_empty() {
            return;
        }
        self.upload_uniforms(queue, viewport);
        self.upload_instances(device, queue);

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
        pass.draw(0..4, 0..self.pending.len() as u32);

        self.pending.clear();
    }

    fn upload_uniforms(&mut self, queue: &wgpu::Queue, viewport: [f32; 2]) {
        if viewport != self.last_viewport {
            let u = Uniforms {
                viewport,
                _pad: [0.0, 0.0],
            };
            queue.write_buffer(&self.uniforms_buffer, 0, bytemuck::bytes_of(&u));
            self.last_viewport = viewport;
        }
    }

    fn upload_instances(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if self.pending.len() > self.instance_capacity {
            let new_cap = self.pending.len().next_power_of_two();
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("lumo-text-instances"),
                size: (new_cap * std::mem::size_of::<GlyphInstance>()) as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.instance_capacity = new_cap;
        }
        queue.write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(&self.pending),
        );
    }

    fn cache_glyph(
        &mut self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        key: CacheKey,
    ) -> Option<AtlasEntry> {
        if let Some(entry) = self.glyph_cache.get(&key) {
            return *entry;
        }

        let image = match self.swash_cache.get_image(&mut self.font_system, key) {
            Some(img) => img,
            None => {
                self.glyph_cache.insert(key, None);
                return None;
            }
        };

        let w = image.placement.width;
        let h = image.placement.height;
        if w == 0 || h == 0 {
            // Glyph sem area (espaco). Nao precisa rasterizar mas e valido.
            self.glyph_cache.insert(key, None);
            return None;
        }

        // etagere prefere allocations com padding pequeno para evitar bleed.
        let alloc = self
            .atlas_packer
            .allocate(size2(w as i32 + 2, h as i32 + 2));
        let Some(Allocation { id, rectangle }) = alloc else {
            log::warn!("text atlas full; dropping glyph {:?}", key);
            self.glyph_cache.insert(key, None);
            return None;
        };

        // Offset 1px dentro do bucket pra padding.
        let dst_x = rectangle.min.x as u32 + 1;
        let dst_y = rectangle.min.y as u32 + 1;

        // Upload do bitmap. cosmic-text expoe mask 8-bit ou subpixel; pegamos
        // SwashContent::Mask (alpha unico, padrao pra non-color glyphs).
        match image.content {
            SwashContent::Mask => {
                let bytes_per_row = w;
                queue.write_texture(
                    wgpu::ImageCopyTexture {
                        texture: &self.atlas_texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d {
                            x: dst_x,
                            y: dst_y,
                            z: 0,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &image.data,
                    wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(bytes_per_row),
                        rows_per_image: Some(h),
                    },
                    wgpu::Extent3d {
                        width: w,
                        height: h,
                        depth_or_array_layers: 1,
                    },
                );
            }
            SwashContent::SubpixelMask | SwashContent::Color => {
                // Caso color/subpx: caimos no fallback que so usa um canal.
                // Para emoji color teriamos que rasterizar em outro atlas RGBA;
                // adia para layer futura.
                log::debug!("unsupported swash content for glyph {:?}", key);
                self.glyph_cache.insert(key, None);
                return None;
            }
        }

        let atlas_f = ATLAS_SIZE as f32;
        let entry = AtlasEntry {
            uv_origin: [dst_x as f32 / atlas_f, dst_y as f32 / atlas_f],
            uv_size: [w as f32 / atlas_f, h as f32 / atlas_f],
            px_size: [w as f32, h as f32],
            px_offset: [image.placement.left as f32, -image.placement.top as f32],
            _alloc: id,
        };
        self.glyph_cache.insert(key, Some(entry));
        Some(entry)
    }

    /// Re-cria o bind group apos resize / recriacao de surface (opcional).
    pub fn rebuild_bind_group(&mut self, device: &wgpu::Device) {
        self.bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lumo-text-bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniforms_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.atlas_sampler),
                },
            ],
        });
    }
}

// ----------------------------------------------------------------------------
// LT* aliases (A9-rename) -- text rendering primitives.
// ----------------------------------------------------------------------------

/// Alias Lumo-style. Prefira `LTRenderer` em call sites novos.
pub type LTRenderer = TextRenderer;
