//! text-demo: demo da Layer 4.1.7 (`lumo-gfx-core`).
//!
//! Renderiza 3 linhas de texto em estilos / pesos diferentes sobre um
//! background INK_DEEP. Valida o pipeline cosmic-text + atlas R8 + shader
//! de glyph instanced.

use std::sync::Arc;

use cosmic_text::Weight;
use lumo_gfx_core::{color, text::TextRenderer, text::TextStyle, INK_DEEP};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalSize},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

// ----------------------------------------------------------------------------
// Renderer leve dedicado ao demo. Sem QuadRenderer: foco e texto puro.
// ----------------------------------------------------------------------------
struct TextDemo {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    text: TextRenderer,
    _window: Arc<Window>,
}

impl TextDemo {
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
                    label: Some("text-demo::device"),
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

        let text = TextRenderer::new(&device, &queue, surface_format);

        Self {
            surface,
            device,
            queue,
            config,
            text,
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
                label: Some("text-demo::encoder"),
            });

        // Pass 1: clear ink-deep.
        {
            let _ = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("text-demo::clear-pass"),
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
        }

        // Texto. Geist nao esta no sistema do Galaxy; cosmic-text faz fallback
        // automatico pra SansSerif. Para "Geist Mono" caimos no monospace.
        let viewport = [self.config.width as f32, self.config.height as f32];

        // Linha 1: titulo grande emerald
        self.text.queue_text(
            &self.device,
            &self.queue,
            "Lumo",
            &TextStyle {
                size: 64.0,
                color: color::EMERALD_600,
                family: "Geist".to_string(),
                weight: Weight::BOLD,
            },
            [50.0, 50.0],
        );

        // Linha 2: subtitulo pearl
        self.text.queue_text(
            &self.device,
            &self.queue,
            "lumo-gfx-core",
            &TextStyle {
                size: 18.0,
                color: color::PEARL,
                family: "Geist".to_string(),
                weight: Weight::NORMAL,
            },
            [50.0, 130.0],
        );

        // Linha 3: hint mono muted
        self.text.queue_text(
            &self.device,
            &self.queue,
            "Press emerald button below",
            &TextStyle {
                size: 14.0,
                color: color::MUTED,
                family: "monospace".to_string(),
                weight: Weight::NORMAL,
            },
            [50.0, 170.0],
        );

        // Flush em um pass que carrega o conteudo previo (LoadOp::Load).
        self.text
            .flush(&self.device, &self.queue, &mut encoder, &view, viewport);

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(())
    }
}

// ----------------------------------------------------------------------------
// winit ApplicationHandler boilerplate (mesmo padrao dos outros bins).
// ----------------------------------------------------------------------------
struct App {
    window: Option<Arc<Window>>,
    renderer: Option<TextDemo>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("lumo-gfx-core text-demo")
            .with_inner_size(LogicalSize::new(800, 400));
        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("create_window"),
        );
        let renderer = pollster::block_on(TextDemo::new(window.clone()));
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
