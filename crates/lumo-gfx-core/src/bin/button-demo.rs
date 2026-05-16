//! button-demo: Layer 4.1.8 — primeiro widget Lumo (Button).
//!
//! 3 botoes empilhados verticalmente sobre fundo pearl:
//!   - primary "Pressione" (bg emerald-600, label pearl)
//!   - ghost   "Cancelar"  (border emerald-500, bg transparente, label emerald)
//!   - danger  "Apagar"    (bg danger, label pearl, shadow vermelho)
//!
//! Cada botao usa `widget::Button` que compoe quad + text. Renderer
//! orquestra: QuadRenderer draw -> TextRenderer flush (LoadOp::Load) no
//! mesmo frame.

use std::sync::Arc;
use std::time::Instant;

use lumo_gfx_core::{
    text::TextRenderer, widget::Button, GlobalUniforms, QuadInstance, QuadRenderer, PEARL_CLEAR,
};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalSize},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

const CANVAS_W: f32 = 480.0;
const CANVAS_H: f32 = 360.0;

// ----------------------------------------------------------------------------
// Renderer leve dedicado ao demo.
// ----------------------------------------------------------------------------
struct ButtonDemo {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    quads: QuadRenderer,
    text: TextRenderer,
    start: Instant,
    _window: Arc<Window>,
}

impl ButtonDemo {
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
                    label: Some("button-demo::device"),
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

        let quads = QuadRenderer::new(&device, surface_format, 16);
        let text = TextRenderer::new(&device, &queue, surface_format);

        Self {
            surface,
            device,
            queue,
            config,
            quads,
            text,
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
        let viewport = [self.config.width as f32, self.config.height as f32];

        // --- monta a UI ---------------------------------------------------------
        let primary = Button::primary().with_label("Pressione");
        let ghost = Button::ghost().with_label("Cancelar");
        let danger = Button::danger().with_label("Apagar");

        // Layout: 3 botoes empilhados, centro horizontal, gap 16px.
        let buttons = [&primary, &ghost, &danger];
        let measured: Vec<[f32; 2]> =
            buttons.iter().map(|b| b.measure(&mut self.text)).collect();
        let total_h: f32 = measured.iter().map(|s| s[1]).sum::<f32>() + 16.0 * 2.0;
        let start_y = (viewport[1] - total_h) * 0.5;

        let mut quad_instances: Vec<QuadInstance> = Vec::with_capacity(3);
        let mut y = start_y;
        for (i, b) in buttons.iter().enumerate() {
            let w = measured[i][0];
            let x = (viewport[0] - w) * 0.5;
            b.queue(
                &mut quad_instances,
                &mut self.text,
                &self.device,
                &self.queue,
                [x, y],
                viewport,
            );
            y += measured[i][1] + 16.0;
        }

        // --- atualiza GPU buffers ----------------------------------------------
        self.quads.update_globals(
            &self.queue,
            GlobalUniforms {
                viewport_size: viewport,
                time: self.start.elapsed().as_secs_f32(),
                _pad: 0.0,
            },
        );
        self.quads
            .update_instances(&self.device, &self.queue, &quad_instances);

        // --- draw --------------------------------------------------------------
        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("button-demo::encoder"),
            });

        // Pass 1: clear pearl + draw quads.
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("button-demo::quads-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(PEARL_CLEAR),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.quads.draw(&mut pass);
        }

        // Pass 2: text flush por cima (LoadOp::Load preserva os quads).
        self.text
            .flush(&self.device, &self.queue, &mut encoder, &view, viewport);

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
    renderer: Option<ButtonDemo>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("lumo-gfx-core button-demo")
            .with_inner_size(LogicalSize::new(CANVAS_W, CANVAS_H));
        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("create_window"),
        );
        let renderer = pollster::block_on(ButtonDemo::new(window.clone()));
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
