//! quad-gallery: demo da Layer 4.1.5 (`lumo-gfx-core`).
//!
//! Renderiza um grid 2x2 de quads testando: solid fill, border, corner radius,
//! e contraste de cores do design system Lumo. Clear color INK_DEEP.

use std::sync::Arc;

use lumo_gfx_core::{
    color, INK_DEEP, QuadInstance, QuadRenderer,
};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalSize},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

// ----------------------------------------------------------------------------
// Cena: 4 quads num grid 2x2.
//
//   [1] solid emerald, sem border, radius 0
//   [2] emerald, border pearl 2px, radius 0.05
//   [3] panel-hi, border emerald 1px, radius 0.10
//   [4] ink-deep, border emerald-500 2px, radius 0.18
//
// Coordenadas em NDC. Cada quad ocupa 0.4 x 0.4 (cabe folgado no quadrante).
// ----------------------------------------------------------------------------
fn build_scene() -> Vec<QuadInstance> {
    vec![
        QuadInstance::new(
            [-0.5,  0.5], [0.4, 0.4],
            color::EMERALD_600,
            color::TRANSPARENT,
            0.0,
            0.0,
        ),
        QuadInstance::new(
            [ 0.5,  0.5], [0.4, 0.4],
            color::EMERALD_600,
            color::PEARL,
            0.008,
            0.05,
        ),
        QuadInstance::new(
            [-0.5, -0.5], [0.4, 0.4],
            color::PANEL_HI,
            color::EMERALD_600,
            0.004,
            0.10,
        ),
        QuadInstance::new(
            [ 0.5, -0.5], [0.4, 0.4],
            color::INK_DEEP,
            color::EMERALD_500,
            0.008,
            0.18,
        ),
    ]
}

// ----------------------------------------------------------------------------
// Renderer leve dedicado ao demo. Configura wgpu manualmente para nao
// arrastar o pipeline de triangle do `lumo_gfx_core::Renderer`.
// ----------------------------------------------------------------------------
struct GalleryRenderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    quads: QuadRenderer,
    _window: Arc<Window>,
}

impl GalleryRenderer {
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
                    label: Some("quad-gallery::device"),
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
        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("quad-gallery::encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("quad-gallery::main-pass"),
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
// winit ApplicationHandler boilerplate (mesmo padrao do binario `triangle`).
// ----------------------------------------------------------------------------
struct App {
    window: Option<Arc<Window>>,
    renderer: Option<GalleryRenderer>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("lumo-gfx-core quad-gallery")
            .with_inner_size(LogicalSize::new(800, 600));
        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("create_window"),
        );
        let renderer = pollster::block_on(GalleryRenderer::new(window.clone()));
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
