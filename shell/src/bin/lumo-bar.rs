//! luiz-bar — top bar layer-shell Wayland.
//! Sticky top, full width, exclusive zone reserva 32px pro compositor.

use gpui::{
    div, point, prelude::*, px, rgb, rgba, size, App, Bounds, Context, MouseButton, SharedString,
    Window, WindowBounds, WindowKind, WindowOptions,
};
use gpui::layer_shell::{Anchor, KeyboardInteractivity, Layer, LayerShellOptions};
use gpui_platform::application;
use std::time::Duration;

// ============================================================
// Tokens (duplicados aqui pra independência do bin)
// ============================================================
const C_BG_TOPBAR: u32 = 0x0a0a0c;
const C_TEXT: u32      = 0xf5f5f7;
const C_MUTED: u32     = 0x9596a0;
const C_ACCENT: u32    = 0x059669;
const C_BORDER: u32    = 0xffffff14;

const BAR_HEIGHT: f32 = 32.0;

// ============================================================
// State
// ============================================================
#[derive(Clone, Copy, PartialEq)]
enum MenuKind { App, Wifi, Battery, Clock }

struct TopBar {
    clock: SharedString,
    active_app: SharedString,
    battery_pct: u8,
    wifi_on: bool,
    menu: Option<MenuKind>,
}

impl TopBar {
    fn new() -> Self {
        Self {
            clock: format_now(),
            active_app: "Finder".into(),
            battery_pct: 100,
            wifi_on: true,
            menu: None,
        }
    }

    fn toggle_menu(&mut self, kind: MenuKind) {
        self.menu = if self.menu == Some(kind) { None } else { Some(kind) };
    }
}

fn format_now() -> SharedString {
    use std::time::SystemTime;
    let secs_since_epoch = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Offset Recife: UTC-3 = -3 * 3600 = -10800s. Sub direto.
    let local = secs_since_epoch.wrapping_sub(3 * 3600);
    let h = (local / 3600) % 24;
    let m = (local / 60) % 60;
    format!("{:02}:{:02}", h, m).into()
}

// ============================================================
// Render
// ============================================================
impl Render for TopBar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let menu_open = self.menu;

        // === Esquerda ===
        let brand_dot = div().size_2().bg(rgb(C_ACCENT)).rounded_full();
        let app_btn = div()
            .id("app-menu")
            .flex().flex_row().items_center().gap_2()
            .px_2().py_1().rounded_md()
            .cursor_pointer()
            .bg(if menu_open == Some(MenuKind::App) { rgba(C_BORDER) } else { rgba(0) })
            .hover(|s| s.bg(rgba(C_BORDER)))
            .child(brand_dot)
            .child(div().text_xs().text_color(rgb(C_TEXT)).child(self.active_app.clone()))
            .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
                t.toggle_menu(MenuKind::App);
                cx.stop_propagation();
                cx.notify();
            }));

        // === Direita ===
        let wifi_btn = div()
            .id("wifi-menu")
            .px_2().py_1().rounded_md()
            .cursor_pointer()
            .bg(if menu_open == Some(MenuKind::Wifi) { rgba(C_BORDER) } else { rgba(0) })
            .hover(|s| s.bg(rgba(C_BORDER)))
            .child(wifi_glyph(self.wifi_on))
            .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
                t.toggle_menu(MenuKind::Wifi);
                cx.stop_propagation();
                cx.notify();
            }));

        let battery_btn = div()
            .id("battery-menu")
            .px_2().py_1().rounded_md()
            .cursor_pointer()
            .bg(if menu_open == Some(MenuKind::Battery) { rgba(C_BORDER) } else { rgba(0) })
            .hover(|s| s.bg(rgba(C_BORDER)))
            .child(battery_glyph(self.battery_pct))
            .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
                t.toggle_menu(MenuKind::Battery);
                cx.stop_propagation();
                cx.notify();
            }));

        let clock_btn = div()
            .id("clock-menu")
            .px_2().py_1().rounded_md()
            .cursor_pointer()
            .bg(if menu_open == Some(MenuKind::Clock) { rgba(C_BORDER) } else { rgba(0) })
            .hover(|s| s.bg(rgba(C_BORDER)))
            .child(div().text_xs().text_color(rgb(C_TEXT)).child(self.clock.clone()))
            .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
                t.toggle_menu(MenuKind::Clock);
                cx.stop_propagation();
                cx.notify();
            }));

        let right_cluster = div()
            .flex().flex_row().items_center().gap_2()
            .child(wifi_btn)
            .child(battery_btn)
            .child(clock_btn);

        let bar_row = div()
            .w_full().h(px(BAR_HEIGHT))
            .px_3()
            .flex().flex_row().items_center().justify_between()
            .child(app_btn)
            .child(div().flex().items_center().justify_center().w_full().h_full())
            .child(right_cluster);

        // === Menus condicionais ===
        let mut root = div()
            .w_full()
            .bg(rgb(C_BG_TOPBAR))
            .border_b_1().border_color(rgba(C_BORDER))
            .flex().flex_col()
            .child(bar_row)
            .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
                if t.menu.is_some() { t.menu = None; cx.notify(); }
            }));

        if let Some(kind) = menu_open {
            root = root.child(render_menu(kind, self, cx));
        }

        root
    }
}

// ============================================================
// Menus dropdown
// ============================================================
fn render_menu(kind: MenuKind, t: &TopBar, cx: &mut Context<TopBar>) -> impl IntoElement {
    let panel = div()
        .flex().flex_col()
        .w(px(280.))
        .p_4().gap_3()
        .bg(rgb(C_BG_TOPBAR))
        .border_1().border_color(rgba(C_BORDER))
        .rounded_md()
        .shadow_lg()
        .on_mouse_down(MouseButton::Left, |_: &gpui::MouseDownEvent, _, cx| { cx.stop_propagation(); });

    let body = match kind {
        MenuKind::App => panel
            .child(div().text_xs().text_color(rgb(C_MUTED)).child("App"))
            .child(div().text_sm().text_color(rgb(C_TEXT)).child(t.active_app.clone()))
            .child(menu_action("Sobre", cx))
            .child(menu_action("Sair", cx)),
        MenuKind::Wifi => panel
            .child(div().text_xs().text_color(rgb(C_MUTED)).child(if t.wifi_on { "Wi-Fi ligado" } else { "Wi-Fi desligado" }))
            .child(menu_action("VirgaNetwork", cx))
            .child(menu_action("Outra rede", cx))
            .child(menu_action("Preferencias de rede", cx)),
        MenuKind::Battery => panel
            .child(div().text_xs().text_color(rgb(C_MUTED)).child("Bateria"))
            .child(div().text_2xl().text_color(rgb(C_TEXT)).child(format!("{}%", t.battery_pct)))
            .child(div().text_xs().text_color(rgb(C_MUTED)).child("Conectado"))
            .child(menu_action("Travar tela", cx))
            .child(menu_action("Reiniciar", cx))
            .child(menu_action("Desligar", cx)),
        MenuKind::Clock => panel
            .child(div().text_xs().text_color(rgb(C_MUTED)).child("Hora"))
            .child(div().text_2xl().text_color(rgb(C_TEXT)).child(t.clock.clone()))
            .child(div().text_xs().text_color(rgb(C_MUTED)).child("Recife · UTC-3")),
    };

    let align_right = matches!(kind, MenuKind::Wifi | MenuKind::Battery | MenuKind::Clock);

    div()
        .w_full()
        .flex().flex_row()
        .px_3().pt_1()
        .justify_start()
        .when(align_right, |row| row.justify_end())
        .child(body)
}

fn menu_action(label: &'static str, cx: &mut Context<TopBar>) -> impl IntoElement {
    div()
        .id(gpui::ElementId::Name(label.into()))
        .px_2().py_1()
        .rounded_sm()
        .text_xs().text_color(rgb(C_TEXT))
        .cursor_pointer()
        .hover(|s| s.bg(rgba(C_BORDER)))
        .child(label)
        .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
            t.menu = None;
            cx.stop_propagation();
            cx.notify();
        }))
}

// ============================================================
// Icon glyphs (SVG-like via div shapes, sem emojis)
// ============================================================
fn wifi_glyph(on: bool) -> impl IntoElement {
    let color = if on { rgb(C_TEXT) } else { rgb(C_MUTED) };
    // 3 arcos crescentes simulados com divs rounded
    div()
        .flex().flex_col_reverse().items_center().gap(px(2.))
        .w(px(14.)).h(px(12.))
        .child(div().w(px(3.)).h(px(3.)).rounded_full().bg(color))
        .child(div().w(px(8.)).h(px(2.5)).rounded_full().bg(color))
        .child(div().w(px(13.)).h(px(2.5)).rounded_full().bg(color))
}

fn battery_glyph(pct: u8) -> impl IntoElement {
    let fill_w = (pct as f32 / 100.0) * 16.0;
    let color = if pct <= 20 { rgb(0xf87171) } else { rgb(C_TEXT) };
    div()
        .flex().flex_row().items_center().gap(px(1.))
        .child(
            div()
                .w(px(20.)).h(px(10.))
                .rounded(px(2.))
                .border_1().border_color(color)
                .p(px(1.))
                .child(div().w(px(fill_w)).h_full().rounded(px(1.)).bg(color)),
        )
        .child(div().w(px(2.)).h(px(4.)).rounded_r(px(1.)).bg(color))
}

// ============================================================
// Main
// ============================================================
fn main() {
    application().run(|cx: &mut App| {
        // Clock tick a cada 30s via background timer
        let opts = LayerShellOptions {
            namespace: "lumo-bar".to_string(),
            layer: Layer::Top,
            anchor: Anchor::TOP | Anchor::LEFT | Anchor::RIGHT,
            exclusive_zone: Some(px(BAR_HEIGHT)),
            exclusive_edge: Some(Anchor::TOP),
            margin: Some((px(0.), px(0.), px(0.), px(0.))),
            keyboard_interactivity: KeyboardInteractivity::None,
        };

        // Width 0 = layer-shell auto-calc via anchors LEFT+RIGHT
        let bounds = Bounds {
            origin: point(px(0.), px(0.)),
            size: size(px(0.), px(BAR_HEIGHT)),
        };

        cx.open_window(
            WindowOptions {
                kind: WindowKind::LayerShell(opts),
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                let bar = cx.new(|_| TopBar::new());
                let weak = bar.downgrade();
                cx.spawn(async move |cx| {
                    loop {
                        cx.background_executor()
                            .timer(Duration::from_secs(30))
                            .await;
                        let _ = weak.update(cx, |b, cx| {
                            b.clock = format_now();
                            cx.notify();
                        });
                    }
                }).detach();
                bar
            },
        )
        .expect("falha ao abrir top bar layer-shell");
        cx.activate(false);
    });
}
