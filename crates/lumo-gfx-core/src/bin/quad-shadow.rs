//! quad-shadow: demo da Layer 4.1.6 (`lumo-gfx-core`).
//!
//! Renderiza 4 cards 200x150px no canvas 800x600 com drop shadow estilo Apple:
//!
//!   [1] panel-hi   shadow preto leve   offset (0,4)  radius 16
//!   [2] emerald    shadow accent       offset (0,6)  radius 16
//!   [3] panel      shadow forte        offset (0,12) radius 20
//!   [4] panel-hi   sem shadow (controle)
//!
//! Conversao px -> NDC: dx = (px / 400), dy = (px / 300). Eixo Y do shadow
//! offset segue convencao CSS (positivo = baixo); o shader inverte o sinal
//! pra alinhar com NDC.

use std::sync::Arc;
use std::time::Instant;

use lumo_gfx_core::{
    color, GlobalUniforms, QuadInstance, QuadRenderer, INK_DEEP,
};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalSize},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

const CANVAS_W: f32 = 800.0;
const CANVAS_H: f32 = 600.0;

/// Converte largura/altura em pixels para half-size NDC.
fn px_size_to_ndc(w_px: f32, h_px: f32) -> [f32; 2] {
    [w_px / (CANVAS_W * 0.5), h_px / (CANVAS_H * 0.5)]
}

/// Converte centro em pixels (origem top-left) para NDC.
fn px_center_to_ndc(cx_px: f32, cy_px: f32) -> [f32; 2] {
    let x = (cx_px / (CANVAS_W * 0.5)) - 1.0;
    let y = 1.0 - (cy_px / (CANVAS_H * 0.5));
    [x, y]
}

/// Converte offset em pixels (CSS-style: positivo = direita/baixo) para NDC.
fn px_offset_to_ndc(dx_px: f32, dy_px: f32) -> [f32; 2] {
    [dx_px / (CANVAS_W * 0.5), dy_px / (CANVAS_H * 0.5)]
}

/// Converte raio em pixels para NDC (usa eixo Y como base — eixo curto em 4:3).
fn px_to_ndc_radius(px: f32) -> f32 {
    px / (CANVAS_H * 0.5)
}

fn build_scene() -> Vec<QuadInstance> {
    // Grid 2x2 com gap 40px:
    //   col_left = 200, col_right = 600  (centros X)
    //   row_top  = 200, row_bot   = 400  (centros Y)
    // Cards 200x150, radius 16, sem border (foco em shadow).
    let card_size = px_size_to_ndc(200.0, 150.0);
    let r = px_to_ndc_radius(16.0);
    let r_big = px_to_ndc_radius(20.0);

    vec![
        // [1] panel-hi, shadow preto leve.
        QuadInstance::new(
            px_center_to_ndc(200.0, 200.0),
            [card_size[0] * 2.0, card_size[1] * 2.0],
            color::PANEL_HI,
            color::TRANSPARENT,
            0.0,
            r,
        )
        .with_shadow(
            color::SHADOW_BLACK,
            px_offset_to_ndc(0.0, 4.0),
            px_to_ndc_radius(12.0),
        ),

        // [2] emerald-600, shadow accent translucido.
        QuadInstance::new(
            px_center_to_ndc(600.0, 200.0),
            [card_size[0] * 2.0, card_size[1] * 2.0],
            color::EMERALD_600,
            color::TRANSPARENT,
            0.0,
            r,
        )
        .with_shadow(
            color::SHADOW_ACCENT,
            px_offset_to_ndc(0.0, 6.0),
            px_to_ndc_radius(16.0),
        ),

        // [3] panel, shadow forte (card "elevado").
        QuadInstance::new(
            px_center_to_ndc(200.0, 400.0),
            [card_size[0] * 2.0, card_size[1] * 2.0],
            color::PANEL,
            color::TRANSPARENT,
            0.0,
            r_big,
        )
        .with_shadow(
            [0.0, 0.0, 0.0, 0.6],
            px_offset_to_ndc(0.0, 12.0),
            px_to_ndc_radius(24.0),
        ),

        // [4] panel-hi com border accent leve, sem shadow (controle visual).
        QuadInstance::new(
            px_center_to_ndc(600.0, 400.0),
            [card_size[0] * 2.0, card_size[1] * 2.0],
            color::PANEL_HI,
            color::EMERALD_600,
            px_to_ndc_radius(1.0),
            r,
        ),
    ]
}

// ----------------------------------------------------------------------------
// Renderer leve dedicado ao demo.
// ----------------------------------------------------------------------------
struct ShadowRenderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    quads: QuadRenderer,
    start: Instant,
    _window: Arc<Window>,
}

impl ShadowRenderer {
    async fn new(window: Arc<Window>) -> Self {
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
                    label: Some("quad-shadow::device"),
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

        let mut quads = QuadRenderer::new(&device, surface_format, 16);
        let scene = build_scene();
        quads.update_instances(&device, &queue, &scene);

        Self {
            surface,
            device,
            queue,
            config,
            quads,
            start: Instant::now(),
            _window: window,
        }
    }

    fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        // Atualiza globals com viewport size atual + tempo decorrido.
        let globals = GlobalUniforms {
            viewport_size: [self.config.width as f32, self.config.height as f32],
            time: self.start.elapsed().as_secs_f32(),
            _pad: 0.0,
        };
        self.quads.update_globals(&self.queue, globals);

        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("quad-shadow::encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("quad-shadow::main-pass"),
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

            self.quads.draw(&mut pass);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(())
    }
}

// ----------------------------------------------------------------------------
// winit ApplicationHandler boilerplate.
// ----------------------------------------------------------------------------
struct App {
    window: Option<Arc<Window>>,
    renderer: Option<ShadowRenderer>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("lumo-gfx-core quad-shadow")
            .with_inner_size(LogicalSize::new(CANVAS_W, CANVAS_H));
        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("create_window"),
        );
        let renderer = pollster::block_on(ShadowRenderer::new(window.clone()));
        self.window = Some(window);
        self.renderer = Some(renderer);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        let (Some(renderer), Some(window)) =
            (self.renderer.as_mut(), self.window.as_ref())
        else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                renderer.resize(size);
                window.request_redraw();
            }
            WindowEvent::RedrawRequested => match renderer.render() {
                Ok(()) => {}
                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                    renderer.resize(window.inner_size());
                }
                Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
                Err(e) => log::warn!("render error: {e:?}"),
            },
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(w) = self.window.as_ref() {
            w.request_redraw();
        }
    }
}

fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info"),
    )
    .init();
    let event_loop = EventLoop::new().expect("event_loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App {
        window: None,
        renderer: None,
    };
    event_loop.run_app(&mut app).expect("run_app");
}
