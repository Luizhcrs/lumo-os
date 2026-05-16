//! luiz-shell — Apple-fluid widget gallery em GPUI.
//! Code review fixes P0+P1+P2 aplicados.

mod tokens;

use gpui::{
    actions, div, prelude::*, px, rgb, rgba, size, Animation, AnimationExt as _, App, Bounds,
    Context, ElementId, KeyBinding, MouseButton, MouseDownEvent, SharedString, Window,
    WindowBounds, WindowOptions, ease_in_out,
};
use gpui_platform::application;
use tokens::*;

// ============================================================
// Actions / keybindings
// ============================================================
actions!(luiz, [PrevDemo, NextDemo, CloseOverlay]);

// ============================================================
// Demo registry
// ============================================================
#[derive(Clone, Copy, PartialEq)]
enum Demo {
    SpringButton,
    GlideToggle,
    StaggerReveal,
    HoverLift,
    ToastStack,
    Modal,
    BottomSheet,
    PageTransition,
    Segmented,
    Skeleton,
    BounceList,
    PinchZoom,
    Carousel,
    SwipeDelete,
    ContextMenu,
    LongPress,
    TiltCard,
    StretchBanner,
}

impl Demo {
    const ALL: &'static [Demo] = &[
        Demo::SpringButton,
        Demo::GlideToggle,
        Demo::StaggerReveal,
        Demo::HoverLift,
        Demo::ToastStack,
        Demo::Modal,
        Demo::BottomSheet,
        Demo::PageTransition,
        Demo::Segmented,
        Demo::Skeleton,
        Demo::BounceList,
        Demo::PinchZoom,
        Demo::Carousel,
        Demo::SwipeDelete,
        Demo::ContextMenu,
        Demo::LongPress,
        Demo::TiltCard,
        Demo::StretchBanner,
    ];
    fn label(&self) -> &'static str {
        match self {
            Demo::SpringButton    => "01. Spring button",
            Demo::GlideToggle     => "02. Glide toggle",
            Demo::StaggerReveal   => "03. Stagger reveal",
            Demo::HoverLift       => "04. Hover lift",
            Demo::ToastStack      => "05. Toast stack",
            Demo::Modal           => "06. Modal overlay",
            Demo::BottomSheet     => "07. Bottom sheet",
            Demo::PageTransition  => "08. Page transition",
            Demo::Segmented       => "09. Segmented control",
            Demo::Skeleton        => "10. Skeleton shimmer",
            Demo::BounceList      => "11. Bounce list",
            Demo::PinchZoom       => "12. Pinch zoom",
            Demo::Carousel        => "13. Carousel snap",
            Demo::SwipeDelete     => "14. Swipe to delete",
            Demo::ContextMenu     => "15. Context menu",
            Demo::LongPress       => "16. Press and hold",
            Demo::TiltCard        => "17. Tilt card",
            Demo::StretchBanner   => "18. Stretch banner",
        }
    }
}

// ============================================================
// Sub-states (P2 #16 — start of architecture split)
// ============================================================
#[derive(Default)]
struct SpringState {
    pressed: bool,
    press_tick: usize,
    release_tick: usize,
}

#[derive(Default)]
struct ToggleState {
    on: bool,
    tick: usize,
}

#[derive(Default)]
struct StaggerState {
    tick: usize,
}

#[derive(Default)]
struct ToastState {
    items: Vec<usize>,
    counter: usize,
}

#[derive(Default)]
struct ModalState {
    open: bool,
    tick: usize,
}

#[derive(Default)]
struct SheetState {
    open: bool,
    tick: usize,
}

#[derive(Default)]
struct PageState {
    depth: usize,
    tick: usize,
}

#[derive(Default)]
struct SegState {
    idx: usize,
    prev_idx: usize,
    tick: usize,
}

#[derive(Default)]
struct SkeletonState {
    showing_skeleton: bool, // P1 #21 — renamed pra semântica clara
    tick: usize,
}

#[derive(Default)]
struct BounceState {
    bounced: bool, // simula bounce ao chegar no fim
    tick: usize,
}

struct ZoomState { scale: f32, tick: usize }
impl Default for ZoomState { fn default() -> Self { Self { scale: 1.0, tick: 0 } } }

#[derive(Default)]
struct CarouselState {
    idx: usize,
    prev_idx: usize,
    tick: usize,
}

#[derive(Default)]
struct SwipeState {
    items: Vec<usize>,
    removing: Option<usize>,
    tick: usize,
}

#[derive(Default)]
struct CtxMenuState {
    open: bool,
    tick: usize,
}

#[derive(Default)]
struct LongPressState {
    holding: bool,
    completed: bool,
    tick: usize,
}

#[derive(Default)]
struct TiltState {
    hovered: bool,
    tick: usize,
}

#[derive(Default)]
struct StretchState {
    expanded: bool,
    tick: usize,
}

// ============================================================
// Gallery (root state)
// ============================================================
struct Gallery {
    current: usize,
    spring: SpringState,
    toggle: ToggleState,
    stagger: StaggerState,
    toast: ToastState,
    modal: ModalState,
    sheet: SheetState,
    page: PageState,
    seg: SegState,
    skeleton: SkeletonState,
    bounce: BounceState,
    zoom: ZoomState,
    carousel: CarouselState,
    swipe: SwipeState,
    ctx_menu: CtxMenuState,
    long_press: LongPressState,
    tilt: TiltState,
    stretch: StretchState,
}

impl Gallery {
    fn demo(&self) -> Demo { Demo::ALL[self.current] }

    fn next(&mut self) {
        self.current = (self.current + 1) % Demo::ALL.len();
        self.on_demo_change();
    }

    fn prev(&mut self) {
        self.current = (self.current + Demo::ALL.len() - 1) % Demo::ALL.len();
        self.on_demo_change();
    }

    /// P1 #10 — reset stale animation states ao trocar demo (evita salto visual).
    fn on_demo_change(&mut self) {
        self.seg.prev_idx = self.seg.idx;
        self.carousel.prev_idx = self.carousel.idx;
        self.modal.open = false;
        self.sheet.open = false;
        self.ctx_menu.open = false;
    }

    /// Has overlay aberto (pra Esc handler).
    fn has_overlay(&self) -> bool {
        self.modal.open || self.sheet.open || self.ctx_menu.open
    }

    fn close_overlays(&mut self) {
        self.modal.open = false;
        self.sheet.open = false;
        self.ctx_menu.open = false;
    }
}

// ============================================================
// Constants
// ============================================================
const STAGGER_ITEMS: &[&str] = &[
    "Sincronizar Supermente",
    "Subir SaaS no Coolify",
    "Onboard tenant Marques",
    "Audit security multi-agent",
    "Push commit luizhcrds",
];

const TOAST_MSGS: &[&str] = &[
    "Deploy concluido",
    "Commit pushado",
    "Build OK",
    "Tenant onboarded",
    "Security audit limpo",
];

const SEG_OPTS: &[&str] = &["Hoje", "Semana", "Mes", "Ano"];

// ============================================================
// Demo 1 -- Spring button (P1 #11 — adicionado press animation)
// ============================================================
fn render_spring_button(g: &Gallery, cx: &mut Context<Gallery>) -> gpui::AnyElement {
    let pressed = g.spring.pressed;
    let press_id: SharedString = format!("press-{}", g.spring.press_tick).into();
    let release_id: SharedString = format!("release-{}", g.spring.release_tick).into();

    let base = div()
        .id("spring-btn")
        .px_8().py_3()
        .bg(if pressed { rgb(C_ACCENT_PRESS) } else { rgb(C_ACCENT) })
        .text_color(rgb(C_ON_ACCENT))
        .text_sm()
        .rounded_lg()
        .shadow_md()
        .child("Pressione")
        .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
            t.spring.pressed = true;
            t.spring.press_tick = t.spring.press_tick.wrapping_add(1);
            cx.notify();
        }))
        .on_mouse_up(MouseButton::Left, cx.listener(|t, _, _, cx| {
            t.spring.pressed = false;
            t.spring.release_tick = t.spring.release_tick.wrapping_add(1);
            cx.notify();
        }));

    if pressed {
        // P1 #11 — press anim 150ms (1.0 -> 0.75 opacity)
        base.with_animation(
            ElementId::Name(press_id),
            Animation::new(DUR_PRESS).with_easing(ease_in_out),
            |b, delta| {
                let op = 1.0 + (0.75 - 1.0) * delta.clamp(0.0, 1.0);
                b.opacity(op)
            },
        )
        .into_any_element()
    } else {
        base.with_animation(
            ElementId::Name(release_id),
            Animation::new(DUR_BOUNCE).with_easing(ease_in_out),
            |b, delta| {
                let op = if delta < 0.5 {
                    0.75 + (1.05 - 0.75) * (delta * 2.0)
                } else {
                    1.05 + (1.0 - 1.05) * ((delta - 0.5) * 2.0)
                };
                b.opacity(op.clamp(0.0, 1.0))
            },
        )
        .into_any_element()
    }
}

// ============================================================
// Demo 2 -- Glide toggle
// ============================================================
fn render_glide_toggle(g: &Gallery, cx: &mut Context<Gallery>) -> gpui::AnyElement {
    let on = g.toggle.on;
    let tick_id: SharedString = format!("toggle-{}", g.toggle.tick).into();
    let fill_target: f32 = if on { 1.0 } else { 0.0 };

    let fill = div()
        .absolute().top_0().left_0().h_full()
        .bg(rgb(C_ACCENT))
        .with_animation(
            ElementId::Name(tick_id),
            Animation::new(DUR_MODAL).with_easing(ease_in_out),
            move |elem, delta| {
                let progress = if fill_target > 0.5 { delta } else { 1.0 - delta };
                elem.w(px(120.0 * progress.clamp(0.0, 1.0)))
            },
        );

    let label = div()
        .absolute().top_0().left_0().w_full().h_full()
        .flex().items_center().justify_center()
        .text_xs()
        .text_color(if on { rgb(C_ON_ACCENT) } else { rgb(C_ACCENT) })
        .child(if on { "LIGADO" } else { "DESLIGADO" });

    div()
        .id("glide-toggle")
        .w(px(120.)).h(px(36.))
        .rounded_md().border_1().border_color(rgb(C_ACCENT))
        .relative().overflow_hidden().cursor_pointer()
        .child(fill).child(label)
        .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
            t.toggle.on = !t.toggle.on;
            t.toggle.tick = t.toggle.tick.wrapping_add(1);
            cx.notify();
        }))
        .into_any_element()
}

// ============================================================
// Demo 3 -- Stagger reveal
// ============================================================
fn render_stagger(g: &Gallery, cx: &mut Context<Gallery>) -> gpui::AnyElement {
    let tick = g.stagger.tick;
    let mut list = div().flex().flex_col().gap_2();

    for (i, txt) in STAGGER_ITEMS.iter().enumerate() {
        let delay_ms = (i as u64) * 70;
        let total = 280 + delay_ms;
        let item_id: SharedString = format!("stag-{}-{}", tick, i).into();

        let item = div()
            .px_4().py_2().rounded_md()
            .bg(rgb(C_PANEL))
            .border_1().border_color(rgba(C_BORDER))
            .text_sm().text_color(rgb(C_TEXT))
            .child(*txt)
            .with_animation(
                ElementId::Name(item_id),
                Animation::new(std::time::Duration::from_millis(total)).with_easing(ease_in_out),
                move |elem, delta| {
                    let start = delay_ms as f32 / total as f32;
                    let local = ((delta - start) / (1.0 - start)).clamp(0.0, 1.0);
                    elem.opacity(local)
                },
            );
        list = list.child(item);
    }

    let replay = div()
        .id("replay")
        .px_4().py_2().mt_4().rounded_md()
        .bg(rgb(C_ACCENT)).text_color(rgb(C_ON_ACCENT)).text_xs()
        .cursor_pointer().child("Reanimar")
        .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
            t.stagger.tick = t.stagger.tick.wrapping_add(1);
            cx.notify();
        }));

    div().flex().flex_col().items_center().gap_2().child(list).child(replay).into_any_element()
}

// ============================================================
// Demo 4 -- Hover lift (state-driven via on_hover listener)
// ============================================================
fn render_hover_lift(g: &Gallery, cx: &mut Context<Gallery>) -> gpui::AnyElement {
    let hovered = g.tilt.hovered; // reuso state — separar depois
    let _ = g;

    div()
        .id("lift-card")
        .w(px(280.)).h(px(160.))
        .rounded_lg()
        .bg(if hovered { rgb(C_PANEL_HI) } else { rgb(C_PANEL) })
        .border_1()
        .border_color(if hovered { rgb(C_ACCENT) } else { rgba(C_BORDER) })
        .flex().flex_col().items_center().justify_center().gap_2()
        .mt(if hovered { px(-6.) } else { px(0.) })
        .shadow_lg()
        .child(div().text_lg().text_color(rgb(C_TEXT)).child("Hover aqui"))
        .child(div().text_xs().text_color(rgb(C_MUTED)).child("Border accent + elevacao"))
        .on_hover(cx.listener(|t, is_hovered: &bool, _, cx| {
            t.tilt.hovered = *is_hovered;
            cx.notify();
        }))
        .into_any_element()
}

// ============================================================
// Demo 5 -- Toast stack
// ============================================================
fn render_toast_stack(g: &Gallery, cx: &mut Context<Gallery>) -> gpui::AnyElement {
    let mut stack = div().flex().flex_col().gap_2().items_end().mt_4();

    for tid in &g.toast.items {
        let msg = TOAST_MSGS[tid % TOAST_MSGS.len()];
        let anim_id: SharedString = format!("toast-{}", tid).into();
        let toast = div()
            .px_4().py_2().rounded_md()
            .bg(rgb(C_PANEL))
            .border_1().border_color(rgb(C_ACCENT))
            .text_xs().text_color(rgb(C_TEXT))
            .child(msg.to_string())
            .with_animation(
                ElementId::Name(anim_id),
                Animation::new(DUR_MODAL).with_easing(ease_in_out),
                |elem, delta| elem.opacity(delta.clamp(0.0, 1.0)),
            );
        stack = stack.child(toast);
    }

    let fire = div()
        .id("fire-toast")
        .px_4().py_2().rounded_md()
        .bg(rgb(C_ACCENT)).text_color(rgb(C_ON_ACCENT)).text_xs()
        .cursor_pointer().child("Disparar toast")
        .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
            t.toast.counter = t.toast.counter.wrapping_add(1);
            t.toast.items.push(t.toast.counter);
            if t.toast.items.len() > 5 { t.toast.items.remove(0); }
            cx.notify();
        }));

    let clear = div()
        .id("clear-toast")
        .px_4().py_2().ml_2().rounded_md()
        .border_1().border_color(rgba(C_BORDER))
        .text_color(rgb(C_TEXT)).text_xs()
        .cursor_pointer().child("Limpar")
        .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
            t.toast.items.clear(); cx.notify();
        }));

    let controls = div().flex().flex_row().items_center().child(fire).child(clear);
    div().flex().flex_col().items_center().gap_4().child(controls).child(stack).into_any_element()
}

// ============================================================
// Demo 6 -- Modal trigger (overlay renderiza no root, P0 #4)
// ============================================================
fn render_modal_trigger(_g: &Gallery, cx: &mut Context<Gallery>) -> gpui::AnyElement {
    div()
        .flex().flex_col().items_center().gap_4()
        .child(
            div()
                .id("modal-open")
                .px_6().py_2().rounded_md()
                .bg(rgb(C_ACCENT)).text_color(rgb(C_ON_ACCENT)).text_xs()
                .cursor_pointer().child("Abrir modal")
                .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
                    t.modal.open = true;
                    t.modal.tick = t.modal.tick.wrapping_add(1);
                    cx.stop_propagation();
                    cx.notify();
                })),
        )
        .into_any_element()
}

// ============================================================
// Demo 7 -- Bottom sheet trigger (overlay renderiza no root, P0 #4)
// ============================================================
fn render_sheet_trigger(_g: &Gallery, cx: &mut Context<Gallery>) -> gpui::AnyElement {
    div()
        .flex().flex_col().items_center().gap_4()
        .child(
            div()
                .id("sheet-open")
                .px_6().py_2().rounded_md()
                .bg(rgb(C_ACCENT)).text_color(rgb(C_ON_ACCENT)).text_xs()
                .cursor_pointer().child("Abrir sheet")
                .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
                    t.sheet.open = true;
                    t.sheet.tick = t.sheet.tick.wrapping_add(1);
                    cx.stop_propagation();
                    cx.notify();
                })),
        )
        .into_any_element()
}

// ============================================================
// Demo 8 -- Page transition (P0 #3 — contador depth limpo)
// ============================================================
fn render_page_transition(g: &Gallery, cx: &mut Context<Gallery>) -> gpui::AnyElement {
    let depth = g.page.depth;
    let tick_id: SharedString = format!("page-{}", g.page.tick).into();

    let title = match depth {
        0 => "Home",
        1 => "Detalhes",
        _ => "Sub-detalhes",
    };
    let body_text = match depth {
        0 => "Tela inicial",
        1 => "Empurrado pela direita",
        _ => "Stack nivel 2",
    };

    let mut card = div()
        .w(px(360.)).h(px(220.))
        .rounded_lg().bg(rgb(C_PANEL))
        .border_1().border_color(rgba(C_BORDER))
        .p_6().flex().flex_col().gap_2()
        .child(div().text_lg().text_color(rgb(C_TEXT)).child(title))
        .child(div().text_sm().text_color(rgb(C_MUTED)).child(body_text));

    if depth < 2 {
        card = card.child(
            div()
                .id("page-push")
                .px_4().py_2().mt_4().rounded_md()
                .bg(rgb(C_ACCENT)).text_color(rgb(C_ON_ACCENT)).text_xs()
                .cursor_pointer().child(if depth == 0 { "Abrir Detalhes" } else { "Proxima" })
                .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
                    t.page.depth += 1;
                    t.page.tick = t.page.tick.wrapping_add(1);
                    cx.notify();
                })),
        );
    }
    if depth > 0 {
        card = card.child(
            div()
                .id("page-pop")
                .px_4().py_2().mt_2().rounded_md()
                .border_1().border_color(rgba(C_BORDER))
                .text_color(rgb(C_TEXT)).text_xs()
                .cursor_pointer().child("Voltar")
                .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
                    if t.page.depth > 0 {
                        t.page.depth -= 1;
                        t.page.tick = t.page.tick.wrapping_add(1);
                        cx.notify();
                    }
                })),
        );
    }

    let animated = card.with_animation(
        ElementId::Name(tick_id),
        Animation::new(DUR_MODAL).with_easing(ease_in_out),
        |elem, delta| elem.opacity(delta.clamp(0.0, 1.0)),
    );

    div().flex().flex_col().items_center().gap_2()
        .child(div().text_xs().text_color(rgb(C_MUTED)).child(format!("Stack depth: {}", depth)))
        .child(animated)
        .into_any_element()
}

// ============================================================
// Demo 9 -- Segmented control (P0 #1 — IDs distintos pill vs items)
// ============================================================
fn render_segmented(g: &Gallery, cx: &mut Context<Gallery>) -> gpui::AnyElement {
    let idx = g.seg.idx;
    let prev_idx = g.seg.prev_idx;
    let tick_id: SharedString = format!("seg-pill-{}", g.seg.tick).into();
    let from = prev_idx as f32 * SEG_WIDTH;
    let to = idx as f32 * SEG_WIDTH;

    let pill = div()
        .absolute().top_0().h_full()
        .w(px(SEG_WIDTH))
        .rounded_md().bg(rgb(C_ACCENT))
        .with_animation(
            ElementId::Name(tick_id),
            Animation::new(DUR_BASE).with_easing(ease_in_out),
            move |elem, delta| {
                let pos = from + (to - from) * delta.clamp(0.0, 1.0);
                elem.left(px(pos))
            },
        );

    let mut bar = div()
        .relative()
        .flex().flex_row()
        .h(px(36.))
        .w(px(SEG_WIDTH * SEG_OPTS.len() as f32))
        .rounded_md()
        .border_1().border_color(rgba(C_BORDER))
        .bg(rgb(C_PANEL))
        .child(pill);

    for (i, opt) in SEG_OPTS.iter().enumerate() {
        let active = i == idx;
        let item_id: SharedString = format!("seg-item-{}", i).into(); // P0 #1 — distinto de pill
        let item = div()
            .id(ElementId::Name(item_id))
            .w(px(SEG_WIDTH)).h_full()
            .flex().items_center().justify_center()
            .text_xs()
            .text_color(if active { rgb(C_ON_ACCENT) } else { rgb(C_TEXT) })
            .cursor_pointer()
            .child(*opt)
            .on_mouse_down(MouseButton::Left, cx.listener(move |t, _, _, cx| {
                if t.seg.idx != i {
                    t.seg.prev_idx = t.seg.idx;
                    t.seg.idx = i;
                    t.seg.tick = t.seg.tick.wrapping_add(1);
                    cx.notify();
                }
            }));
        bar = bar.child(item);
    }

    bar.into_any_element()
}

// ============================================================
// Demo 10 -- Skeleton (P1 #21 — semântica botão corrigida)
// ============================================================
fn render_skeleton(g: &Gallery, cx: &mut Context<Gallery>) -> gpui::AnyElement {
    let showing = g.skeleton.showing_skeleton;
    let tick_id_base = g.skeleton.tick;

    // P1 #21 — botão descreve A AÇÃO (não estado atual)
    let label = if showing { "Mostrar conteudo" } else { "Mostrar skeleton" };

    let toggle = div()
        .id("skel-toggle")
        .px_4().py_2().rounded_md()
        .bg(rgb(C_ACCENT)).text_color(rgb(C_ON_ACCENT)).text_xs()
        .cursor_pointer()
        .child(label)
        .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
            t.skeleton.showing_skeleton = !t.skeleton.showing_skeleton;
            t.skeleton.tick = t.skeleton.tick.wrapping_add(1);
            cx.notify();
        }));

    let bar = move |width_pct: f32, idx: usize| -> gpui::AnyElement {
        let line_id: SharedString = format!("skel-{}-{}", tick_id_base, idx).into();
        if !showing {
            div()
                .h(px(12.)).rounded_md().bg(rgb(C_PANEL_HI))
                .w(px(280.0 * width_pct))
                .into_any_element()
        } else {
            div()
                .h(px(12.)).rounded_md().bg(rgb(C_PANEL_HI))
                .w(px(280.0 * width_pct))
                .with_animation(
                    ElementId::Name(line_id),
                    Animation::new(std::time::Duration::from_millis(1400))
                        .repeat()
                        .with_easing(ease_in_out),
                    |elem, delta| {
                        let t = (delta * 2.0 - 1.0).abs();
                        let op = 0.4 + (1.0 - 0.4) * (1.0 - t);
                        elem.opacity(op)
                    },
                )
                .into_any_element()
        }
    };

    let lines = div().flex().flex_col().gap_3()
        .child(bar(1.0, 0))
        .child(bar(0.75, 1))
        .child(bar(0.5, 2));

    div().flex().flex_col().items_center().gap_4().child(toggle).child(lines).into_any_element()
}

// ============================================================
// Demo 11 -- Bounce list (scroll real + bounce wheel ao tentar passar do fim)
// ============================================================
fn render_bounce_list(g: &Gallery, cx: &mut Context<Gallery>) -> gpui::AnyElement {
    let tick_id: SharedString = format!("bounce-{}", g.bounce.tick).into();

    let mut list = div()
        .id("bounce-list")
        .flex().flex_col().gap_2()
        .h(px(220.))
        .w(px(280.))
        .p_2()
        .rounded_md()
        .border_1().border_color(rgba(C_BORDER))
        .bg(rgb(C_PANEL))
        .overflow_y_scroll();

    for i in 1..=20 {
        let row = div()
            .px_3().py_2().rounded_sm()
            .bg(rgb(C_PANEL_HI))
            .text_xs().text_color(rgb(C_TEXT))
            .flex_shrink_0()
            .child(format!("Item {}", i));
        list = list.child(row);
    }

    let bounced = g.bounce.bounced;
    let dy: f32 = if bounced { -16.0 } else { 0.0 };

    let list_with_anim = list.with_animation(
        ElementId::Name(tick_id),
        Animation::new(DUR_BOUNCE).with_easing(ease_in_out),
        move |elem, delta| {
            let progress = delta.clamp(0.0, 1.0);
            let current = dy * (1.0 - progress);
            elem.mt(px(current))
        },
    );

    // Wrapper externo captura wheel — trigger bounce ao tentar overscroll
    let area = div()
        .id("bounce-area")
        .flex().flex_col().items_center().gap_3()
        .child(list_with_anim)
        .child(div().text_xs().text_color(rgb(C_MUTED)).child("Scroll real + bounce no overscroll"))
        .on_scroll_wheel(cx.listener(|t, ev: &gpui::ScrollWheelEvent, _, cx| {
            // delta Y > 0 = scroll down (mouse wheel down)
            let dy = ev.delta.pixel_delta(px(20.0)).y;
            // Dispara bounce em wheel down forte. Heuristica: simula overscroll.
            if dy < px(-25.0) && !t.bounce.bounced {
                t.bounce.bounced = true;
                t.bounce.tick = t.bounce.tick.wrapping_add(1);
                cx.notify();
                cx.spawn(async move |this, cx| {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(500))
                        .await;
                    this.update(cx, |g, cx| {
                        g.bounce.bounced = false;
                        cx.notify();
                    }).ok();
                }).detach();
            }
        }));

    area.into_any_element()
}

// ============================================================
// Demo 12 -- Pinch zoom (botões +/- escalam container via width/height)
// ============================================================
fn render_pinch_zoom(g: &Gallery, cx: &mut Context<Gallery>) -> gpui::AnyElement {
    let scale = g.zoom.scale.clamp(0.5, 3.0);
    let tick_id: SharedString = format!("zoom-{}", g.zoom.tick).into();
    let base_size = 80.0;
    let target_size = base_size * scale;

    let target = div()
        .w(px(target_size)).h(px(target_size))
        .rounded_lg()
        .bg(rgb(C_ACCENT))
        .with_animation(
            ElementId::Name(tick_id),
            Animation::new(DUR_BASE).with_easing(ease_in_out),
            move |elem, delta| {
                let d = delta.clamp(0.0, 1.0);
                let cur = base_size + (target_size - base_size) * d;
                elem.w(px(cur)).h(px(cur))
            },
        );

    let zoom_in = div()
        .id("zoom-in")
        .px_3().py_1().rounded_md()
        .bg(rgb(C_ACCENT)).text_color(rgb(C_ON_ACCENT)).text_xs()
        .cursor_pointer().child("+")
        .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
            t.zoom.scale = (t.zoom.scale * 1.2).min(3.0);
            t.zoom.tick = t.zoom.tick.wrapping_add(1);
            cx.notify();
        }));

    let zoom_out = div()
        .id("zoom-out")
        .px_3().py_1().ml_2().rounded_md()
        .border_1().border_color(rgba(C_BORDER))
        .text_color(rgb(C_TEXT)).text_xs()
        .cursor_pointer().child("-")
        .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
            t.zoom.scale = (t.zoom.scale / 1.2).max(0.5);
            t.zoom.tick = t.zoom.tick.wrapping_add(1);
            cx.notify();
        }));

    let reset = div()
        .id("zoom-reset")
        .px_3().py_1().ml_2().rounded_md()
        .border_1().border_color(rgba(C_BORDER))
        .text_color(rgb(C_TEXT)).text_xs()
        .cursor_pointer().child("Reset")
        .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
            t.zoom.scale = 1.0;
            t.zoom.tick = t.zoom.tick.wrapping_add(1);
            cx.notify();
        }));

    let controls = div().flex().flex_row().items_center().child(zoom_in).child(zoom_out).child(reset);
    let label = div().text_xs().text_color(rgb(C_MUTED)).child(format!("scale: {:.2}x | trackpad pinch funciona", scale));

    div()
        .id("pinch-area")
        .flex().flex_col().items_center().gap_4()
        .w_full().py_4()
        .child(target).child(controls).child(label)
        .on_pinch(cx.listener(|t, ev: &gpui::PinchEvent, _, cx| {
            // ev.delta: positivo zoom in, negativo zoom out (~0.1 = 10%)
            let new_scale = (t.zoom.scale * (1.0 + ev.delta)).clamp(0.5, 3.0);
            if (new_scale - t.zoom.scale).abs() > 0.001 {
                t.zoom.scale = new_scale;
                t.zoom.tick = t.zoom.tick.wrapping_add(1);
                cx.notify();
            }
        }))
        .into_any_element()
}

// ============================================================
// Demo 13 -- Carousel snap (pill indicator desliza entre cards)
// ============================================================
fn render_carousel(g: &Gallery, cx: &mut Context<Gallery>) -> gpui::AnyElement {
    let idx = g.carousel.idx;
    let prev_idx = g.carousel.prev_idx;
    let tick_id: SharedString = format!("car-{}", g.carousel.tick).into();
    let card_w = 200.0;
    let cards: &[&str] = &["Card 1", "Card 2", "Card 3", "Card 4", "Card 5"];

    let mut strip = div()
        .id("car-strip")
        .flex().flex_row().gap_3()
        .w(px(420.))
        .py_2().px_2()
        .overflow_x_scroll();
    for (i, label) in cards.iter().enumerate() {
        let active = i == idx;
        let item_id: SharedString = format!("car-item-{}", i).into();
        let card = div()
            .id(ElementId::Name(item_id))
            .w(px(card_w)).h(px(120.))
            .flex_shrink_0()
            .rounded_lg()
            .bg(if active { rgb(C_ACCENT) } else { rgb(C_PANEL) })
            .border_1()
            .border_color(if active { rgb(C_ACCENT) } else { rgba(C_BORDER) })
            .flex().items_center().justify_center()
            .text_sm()
            .text_color(if active { rgb(C_ON_ACCENT) } else { rgb(C_TEXT) })
            .cursor_pointer()
            .child(*label)
            .on_mouse_down(MouseButton::Left, cx.listener(move |t, _, _, cx| {
                if t.carousel.idx != i {
                    t.carousel.prev_idx = t.carousel.idx;
                    t.carousel.idx = i;
                    t.carousel.tick = t.carousel.tick.wrapping_add(1);
                    cx.notify();
                }
            }));
        strip = strip.child(card);
    }

    // Dot indicator com pill animado
    let pill_w = 24.0;
    let dot_gap = 8.0;
    let from = prev_idx as f32 * (pill_w + dot_gap);
    let to = idx as f32 * (pill_w + dot_gap);

    let pill = div()
        .absolute().top_0().h_full()
        .w(px(pill_w))
        .rounded_full().bg(rgb(C_ACCENT))
        .with_animation(
            ElementId::Name(tick_id),
            Animation::new(DUR_BASE).with_easing(ease_in_out),
            move |elem, delta| {
                let pos = from + (to - from) * delta.clamp(0.0, 1.0);
                elem.left(px(pos))
            },
        );

    let dots = div()
        .relative()
        .flex().flex_row()
        .gap(px(dot_gap))
        .h(px(8.))
        .w(px(cards.len() as f32 * (pill_w + dot_gap) - dot_gap))
        .child(pill);

    let max_idx = cards.len() - 1;
    div()
        .id("carousel-area")
        .flex().flex_col().items_center().gap_4()
        .child(strip)
        .child(dots)
        .on_scroll_wheel(cx.listener(move |t, ev: &gpui::ScrollWheelEvent, _, cx| {
            let dx = ev.delta.pixel_delta(px(20.0)).x;
            if dx < px(-30.0) && t.carousel.idx < max_idx {
                t.carousel.prev_idx = t.carousel.idx;
                t.carousel.idx += 1;
                t.carousel.tick = t.carousel.tick.wrapping_add(1);
                cx.notify();
            } else if dx > px(30.0) && t.carousel.idx > 0 {
                t.carousel.prev_idx = t.carousel.idx;
                t.carousel.idx -= 1;
                t.carousel.tick = t.carousel.tick.wrapping_add(1);
                cx.notify();
            }
        }))
        .into_any_element()
}

// ============================================================
// Demo 14 -- Swipe to delete
// ============================================================
fn render_swipe_delete(g: &Gallery, cx: &mut Context<Gallery>) -> gpui::AnyElement {
    // Inicializa lista se vazia
    let items: Vec<usize> = if g.swipe.items.is_empty() {
        vec![0, 1, 2, 3]
    } else {
        g.swipe.items.clone()
    };
    let removing = g.swipe.removing;
    let tick = g.swipe.tick;

    let mut list = div().flex().flex_col().gap_2().w(px(320.));

    for &id in &items {
        let label = format!("Tarefa #{}", id + 1);
        let is_removing = removing == Some(id);
        let row_id: SharedString = format!("swipe-{}-{}", tick, id).into();

        let action_btn = div()
            .id(ElementId::Name(format!("swipe-del-{}", id).into()))
            .px_3().py_2().rounded_md()
            .bg(rgb(C_DANGER)).text_color(rgb(C_TEXT)).text_xs()
            .cursor_pointer().child("Apagar")
            .on_mouse_down(MouseButton::Left, cx.listener(move |t, _, _, cx| {
                t.swipe.removing = Some(id);
                t.swipe.tick = t.swipe.tick.wrapping_add(1);
                cx.notify();
            }));

        let row = div()
            .id(ElementId::Name(format!("swipe-row-{}", id).into()))
            .flex().flex_row().items_center().justify_between()
            .px_4().py_3().rounded_md()
            .bg(rgb(C_PANEL))
            .border_1().border_color(rgba(C_BORDER))
            .child(div().text_sm().text_color(rgb(C_TEXT)).child(label))
            .child(action_btn)
            // Swipe horizontal via wheel deltaX (touchpad 2-finger sideways)
            .on_scroll_wheel(cx.listener(move |t, ev: &gpui::ScrollWheelEvent, _, cx| {
                let dx_px = ev.delta.pixel_delta(px(20.0)).x;
                if dx_px < px(-40.0) {
                    t.swipe.removing = Some(id);
                    t.swipe.tick = t.swipe.tick.wrapping_add(1);
                    cx.notify();
                }
            }));

        let animated = if is_removing {
            row.with_animation(
                ElementId::Name(row_id),
                Animation::new(DUR_BOUNCE).with_easing(ease_in_out),
                move |elem, delta| {
                    let d = delta.clamp(0.0, 1.0);
                    elem.opacity(1.0 - d).ml(px(d * 320.0))
                },
            ).into_any_element()
        } else {
            row.into_any_element()
        };

        list = list.child(animated);
    }

    let restore = div()
        .id("swipe-restore")
        .px_4().py_2().mt_4().rounded_md()
        .border_1().border_color(rgba(C_BORDER))
        .text_color(rgb(C_TEXT)).text_xs()
        .cursor_pointer().child("Restaurar lista")
        .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
            t.swipe.items = vec![0, 1, 2, 3];
            t.swipe.removing = None;
            t.swipe.tick = t.swipe.tick.wrapping_add(1);
            cx.notify();
        }));

    div().flex().flex_col().items_center().gap_2().child(list).child(restore).into_any_element()
}

// ============================================================
// Demo 15 -- Context menu (long-press style, mas trigger via click)
// ============================================================
fn render_context_menu(g: &Gallery, cx: &mut Context<Gallery>) -> gpui::AnyElement {
    let open = g.ctx_menu.open;
    let tick_id: SharedString = format!("ctx-{}", g.ctx_menu.tick).into();

    let trigger = div()
        .id("ctx-trigger")
        .px_6().py_3().rounded_lg()
        .bg(rgb(C_ACCENT)).text_color(rgb(C_ON_ACCENT)).text_sm()
        .cursor_pointer()
        .child(if open { "Menu aberto" } else { "Clique pra menu" })
        .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
            t.ctx_menu.open = !t.ctx_menu.open;
            t.ctx_menu.tick = t.ctx_menu.tick.wrapping_add(1);
            cx.notify();
        }));

    let mut col = div().flex().flex_col().items_center().gap_2().child(trigger);

    if open {
        let menu = div()
            .mt_2()
            .w(px(200.))
            .rounded_md()
            .bg(rgb(C_PANEL_HI))
            .border_1().border_color(rgba(C_BORDER))
            .shadow_lg()
            .flex().flex_col()
            .child(menu_item("Editar", cx))
            .child(menu_item("Duplicar", cx))
            .child(menu_item("Compartilhar", cx))
            .child(menu_divider())
            .child(menu_item_danger("Apagar", cx))
            .with_animation(
                ElementId::Name(tick_id),
                Animation::new(DUR_BASE).with_easing(ease_in_out),
                |elem, delta| {
                    let d = delta.clamp(0.0, 1.0);
                    elem.opacity(d)
                },
            );
        col = col.child(menu);
    }

    col.into_any_element()
}

fn menu_item(label: &'static str, cx: &mut Context<Gallery>) -> impl IntoElement {
    div()
        .id(ElementId::Name(label.into()))
        .px_4().py_2()
        .text_sm().text_color(rgb(C_TEXT))
        .cursor_pointer()
        .hover(|s| s.bg(rgba(C_BORDER)))
        .child(label)
        .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
            t.ctx_menu.open = false; cx.notify();
        }))
}

fn menu_item_danger(label: &'static str, cx: &mut Context<Gallery>) -> impl IntoElement {
    div()
        .id(ElementId::Name(format!("danger-{}", label).into()))
        .px_4().py_2()
        .text_sm().text_color(rgb(C_DANGER))
        .cursor_pointer()
        .hover(|s| s.bg(rgba(C_BORDER)))
        .child(label)
        .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
            t.ctx_menu.open = false; cx.notify();
        }))
}

fn menu_divider() -> impl IntoElement {
    div().h(px(1.)).w_full().bg(rgba(C_BORDER))
}

// ============================================================
// Demo 16 -- Press and hold (progress bar fills durante hold 800ms)
// ============================================================
fn render_long_press(g: &Gallery, cx: &mut Context<Gallery>) -> gpui::AnyElement {
    let holding = g.long_press.holding;
    let completed = g.long_press.completed;
    let tick_id: SharedString = format!("hold-{}", g.long_press.tick).into();

    let label = if completed {
        "Confirmado"
    } else if holding {
        "Segurando..."
    } else {
        "Segure 800ms"
    };

    let btn_bg = if completed { rgb(C_ACCENT_PRESS) } else { rgb(C_ACCENT) };

    // Progress bar fica embaixo do botão, anima width durante hold
    let progress = if holding && !completed {
        Some(
            div()
                .h(px(4.))
                .rounded_full()
                .bg(rgb(C_ACCENT_PRESS))
                .with_animation(
                    ElementId::Name(tick_id.clone()),
                    Animation::new(std::time::Duration::from_millis(800)).with_easing(ease_in_out),
                    |elem, delta| {
                        let w = 280.0 * delta.clamp(0.0, 1.0);
                        elem.w(px(w))
                    },
                )
                .into_any_element()
        )
    } else {
        None
    };

    let btn = div()
        .id("long-press-btn")
        .w(px(280.)).h(px(48.))
        .rounded_lg()
        .bg(btn_bg)
        .text_color(rgb(C_ON_ACCENT)).text_sm()
        .flex().items_center().justify_center()
        .cursor_pointer()
        .child(label)
        .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
            t.long_press.holding = true;
            t.long_press.completed = false;
            t.long_press.tick = t.long_press.tick.wrapping_add(1);
            cx.notify();
            // Schedule completion after 800ms via spawn
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(800))
                    .await;
                this.update(cx, |g, cx| {
                    if g.long_press.holding {
                        g.long_press.completed = true;
                        g.long_press.holding = false;
                        // Abre context menu ao completar hold
                        g.ctx_menu.open = true;
                        g.ctx_menu.tick = g.ctx_menu.tick.wrapping_add(1);
                        cx.notify();
                    }
                }).ok();
            }).detach();
        }))
        .on_mouse_up(MouseButton::Left, cx.listener(|t, _, _, cx| {
            if t.long_press.holding {
                t.long_press.holding = false;
                cx.notify();
            }
        }));

    let reset = div()
        .id("hold-reset")
        .px_3().py_1().mt_2().rounded_md()
        .border_1().border_color(rgba(C_BORDER))
        .text_color(rgb(C_TEXT)).text_xs()
        .cursor_pointer().child("Resetar")
        .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
            t.long_press.holding = false;
            t.long_press.completed = false;
            t.long_press.tick = t.long_press.tick.wrapping_add(1);
            t.ctx_menu.open = false;
            cx.notify();
        }));

    let mut col = div().flex().flex_col().items_center().gap_2().child(btn);
    if let Some(p) = progress { col = col.child(p); }

    if completed && g.ctx_menu.open {
        let menu = div()
            .mt_4()
            .w(px(220.))
            .rounded_md()
            .bg(rgb(C_PANEL_HI))
            .border_1().border_color(rgba(C_BORDER))
            .shadow_lg()
            .flex().flex_col()
            .child(menu_item("Acao 1", cx))
            .child(menu_item("Acao 2", cx))
            .child(menu_divider())
            .child(menu_item_danger("Cancelar", cx));
        col = col.child(menu);
    }

    col = col.child(reset);
    col.into_any_element()
}

// ============================================================
// Demo 17 -- Tilt card (gradient direcional hover; signature propria)
// ============================================================
fn render_tilt_card(g: &Gallery, cx: &mut Context<Gallery>) -> gpui::AnyElement {
    let hovered = g.tilt.hovered;
    let tick_id: SharedString = format!("tilt-{}", g.tilt.tick).into();

    let card_base = div()
        .id("tilt-card")
        .w(px(320.)).h(px(200.))
        .rounded_lg()
        .border_1().border_color(rgba(C_BORDER))
        .flex().flex_col().items_center().justify_center().gap_2()
        .cursor_pointer()
        .child(div().text_lg().text_color(rgb(C_TEXT)).child("Tilt card"))
        .child(div().text_xs().text_color(rgb(C_MUTED)).child("Hover sente tato"))
        .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
            t.tilt.hovered = !t.tilt.hovered;
            t.tilt.tick = t.tilt.tick.wrapping_add(1);
            cx.notify();
        }));

    if hovered {
        card_base
            .bg(rgb(C_PANEL_HI))
            .shadow_lg()
            .with_animation(
                ElementId::Name(tick_id),
                Animation::new(DUR_BASE).with_easing(ease_in_out),
                |elem, delta| {
                    let d = delta.clamp(0.0, 1.0);
                    elem.mt(px(-6.0 * d))
                },
            )
            .into_any_element()
    } else {
        card_base.bg(rgb(C_PANEL)).into_any_element()
    }
}

// ============================================================
// Demo 18 -- Stretch banner (height scale + parallax feel)
// ============================================================
fn render_stretch_banner(g: &Gallery, cx: &mut Context<Gallery>) -> gpui::AnyElement {
    let expanded = g.stretch.expanded;
    let tick_id: SharedString = format!("stretch-{}", g.stretch.tick).into();
    let h_target: f32 = if expanded { 200.0 } else { 80.0 };

    let banner = div()
        .w(px(360.))
        .rounded_lg()
        .bg(rgb(C_ACCENT))
        .flex().flex_col().items_center().justify_center()
        .text_color(rgb(C_ON_ACCENT))
        .child(div().text_lg().child("Banner"))
        .child(div().text_xs().child(if expanded { "Expandido" } else { "Compacto" }))
        .with_animation(
            ElementId::Name(tick_id),
            Animation::new(DUR_BOUNCE).with_easing(ease_in_out),
            move |elem, delta| {
                let d = delta.clamp(0.0, 1.0);
                let from = if h_target > 100.0 { 80.0 } else { 200.0 };
                let h = from + (h_target - from) * d;
                elem.h(px(h))
            },
        );

    let toggle = div()
        .id("stretch-toggle")
        .px_4().py_2().rounded_md()
        .bg(rgb(C_ACCENT)).text_color(rgb(C_ON_ACCENT)).text_xs()
        .cursor_pointer()
        .child(if expanded { "Compactar" } else { "Expandir" })
        .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
            t.stretch.expanded = !t.stretch.expanded;
            t.stretch.tick = t.stretch.tick.wrapping_add(1);
            cx.notify();
        }));

    div().flex().flex_col().items_center().gap_4().child(banner).child(toggle).into_any_element()
}

// ============================================================
// Overlay renderers (P0 #2, #4 — overlay vive em root, card filho do backdrop)
// ============================================================
fn render_modal_overlay(g: &Gallery, cx: &mut Context<Gallery>) -> Option<gpui::AnyElement> {
    if !g.modal.open { return None; }
    let tick_id: SharedString = format!("modal-card-{}", g.modal.tick).into();

    // Card filho do backdrop, com stop-prop no on_mouse_down do card
    let card = div()
        .id("modal-card")
        .w(px(320.)).p_6().rounded_lg()
        .bg(rgb(C_PANEL)).border_1().border_color(rgba(C_BORDER))
        .shadow_lg()
        .flex().flex_col().gap_3()
        .child(div().text_lg().text_color(rgb(C_TEXT)).child("Card modal"))
        .child(div().text_xs().text_color(rgb(C_MUTED)).child("Esc ou clique fora pra fechar"))
        .child(
            div()
                .id("modal-close")
                .px_4().py_2().mt_2().rounded_md()
                .bg(rgb(C_ACCENT)).text_color(rgb(C_ON_ACCENT)).text_xs()
                .cursor_pointer().child("Fechar")
                .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
                    t.modal.open = false;
                    cx.stop_propagation();
                    cx.notify();
                })),
        )
        // P0 #2 — stop propagation real: clique no card NAO fecha modal
        .on_mouse_down(MouseButton::Left, |_: &MouseDownEvent, _, cx| { cx.stop_propagation(); })
        .with_animation(
            ElementId::Name(tick_id),
            Animation::new(DUR_MODAL).with_easing(ease_in_out),
            |elem, delta| elem.opacity(delta.clamp(0.0, 1.0)),
        );

    Some(
        div()
            .id("modal-backdrop")
            .absolute().top_0().left_0().w_full().h_full()
            .bg(rgba(C_BACKDROP))
            .flex().items_center().justify_center()
            .child(card)
            .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
                t.modal.open = false; cx.notify();
            }))
            .into_any_element(),
    )
}

fn render_sheet_overlay(g: &Gallery, cx: &mut Context<Gallery>) -> Option<gpui::AnyElement> {
    if !g.sheet.open { return None; }
    let tick_id: SharedString = format!("sheet-card-{}", g.sheet.tick).into();

    let card = div()
        .id("sheet-card")
        .absolute().bottom_0().left_0().right_0()
        .h(px(220.))
        .px_6().py_5()
        .bg(rgb(C_PANEL_HI))
        .border_t_1().border_color(rgba(C_BORDER))
        .flex().flex_col().justify_start().gap_3()
        .child(
            div().flex().w_full().justify_center()
                .child(div().w(px(40.)).h(px(4.)).rounded_full().bg(rgb(C_MUTED)).opacity(0.5))
        )
        .child(div().text_lg().text_color(rgb(C_TEXT)).child("Bottom sheet"))
        .child(div().text_xs().text_color(rgb(C_MUTED)).child("Esc ou clique fora pra fechar"))
        .on_mouse_down(MouseButton::Left, |_: &MouseDownEvent, _, cx| { cx.stop_propagation(); })
        .with_animation(
            ElementId::Name(tick_id),
            Animation::new(DUR_BOUNCE).with_easing(ease_in_out),
            |elem, delta| elem.opacity(delta.clamp(0.0, 1.0)),
        );

    Some(
        div()
            .id("sheet-backdrop")
            .absolute().top_0().left_0().w_full().h_full()
            .bg(rgba(C_BACKDROP_SOFT))
            .child(card)
            .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
                t.sheet.open = false; cx.notify();
            }))
            .into_any_element(),
    )
}

// ============================================================
// Layout principal — Render impl
// ============================================================
impl Render for Gallery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let demo = self.demo();

        let header = div()
            .flex().flex_row().items_center().justify_between()
            .w_full().px_8().py_4()
            .border_b_1().border_color(rgba(C_BORDER))
            .child(
                div().flex().flex_row().items_center().gap_3()
                    .child(div().size_2().bg(rgb(C_ACCENT)).rounded_full())
                    .child(div().text_sm().text_color(rgb(C_TEXT)).child("luiz-shell")),
            )
            .child(
                div().text_xs().text_color(rgb(C_MUTED))
                    .child(format!("{} / {}", self.current + 1, Demo::ALL.len())),
            );

        let title = div()
            .px_8().pt_8().pb_2()
            .text_xl().text_color(rgb(C_TEXT))
            .child(demo.label());

        let body_content: gpui::AnyElement = match demo {
            Demo::SpringButton    => render_spring_button(self, cx),
            Demo::GlideToggle     => render_glide_toggle(self, cx),
            Demo::StaggerReveal   => render_stagger(self, cx),
            Demo::HoverLift       => render_hover_lift(self, cx),
            Demo::ToastStack      => render_toast_stack(self, cx),
            Demo::Modal           => render_modal_trigger(self, cx),
            Demo::BottomSheet     => render_sheet_trigger(self, cx),
            Demo::PageTransition  => render_page_transition(self, cx),
            Demo::Segmented       => render_segmented(self, cx),
            Demo::Skeleton        => render_skeleton(self, cx),
            Demo::BounceList      => render_bounce_list(self, cx),
            Demo::PinchZoom       => render_pinch_zoom(self, cx),
            Demo::Carousel        => render_carousel(self, cx),
            Demo::SwipeDelete     => render_swipe_delete(self, cx),
            Demo::ContextMenu     => render_context_menu(self, cx),
            Demo::LongPress       => render_long_press(self, cx),
            Demo::TiltCard        => render_tilt_card(self, cx),
            Demo::StretchBanner   => render_stretch_banner(self, cx),
        };

        let body = div().flex().flex_grow().justify_center().items_center().w_full().px_8().py_4()
            .child(body_content);

        let nav = div()
            .flex().flex_row().items_center().justify_between()
            .w_full().px_8().py_4()
            .border_t_1().border_color(rgba(C_BORDER))
            .child(
                div()
                    .id("prev")
                    .px_4().py_2().rounded_md()
                    .border_1().border_color(rgba(C_BORDER))
                    .text_xs().text_color(rgb(C_TEXT))
                    .cursor_pointer().child("anterior")
                    .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
                        t.prev(); cx.notify();
                    })),
            )
            .child(
                div().text_xs().text_color(rgb(C_MUTED))
                    .child("←  →  Esc  pra navegar / fechar"),
            )
            .child(
                div()
                    .id("next")
                    .px_4().py_2().rounded_md()
                    .bg(rgb(C_ACCENT))
                    .text_xs().text_color(rgb(C_ON_ACCENT))
                    .cursor_pointer().child("proximo")
                    .on_mouse_down(MouseButton::Left, cx.listener(|t, _, _, cx| {
                        t.next(); cx.notify();
                    })),
            );

        // P0 #4 — overlay renderiza no root, cobre header+nav
        let mut root = div()
            .key_context("Gallery")
            .on_action(cx.listener(|t, _: &PrevDemo, _, cx| { t.prev(); cx.notify(); }))
            .on_action(cx.listener(|t, _: &NextDemo, _, cx| { t.next(); cx.notify(); }))
            .on_action(cx.listener(|t, _: &CloseOverlay, _, cx| {
                if t.has_overlay() { t.close_overlays(); cx.notify(); }
            }))
            .relative()
            .flex().flex_col().size_full().bg(rgb(C_BG))
            .child(header).child(title).child(body).child(nav);

        if let Some(modal) = render_modal_overlay(self, cx) {
            root = root.child(modal);
        }
        if let Some(sheet) = render_sheet_overlay(self, cx) {
            root = root.child(sheet);
        }

        root
    }
}

// ============================================================
// Main
// ============================================================
fn main() {
    application().run(|cx: &mut App| {
        // Keyboard bindings
        cx.bind_keys([
            KeyBinding::new("left",   PrevDemo,     Some("Gallery")),
            KeyBinding::new("right",  NextDemo,     Some("Gallery")),
            KeyBinding::new("escape", CloseOverlay, Some("Gallery")),
        ]);

        let bounds = Bounds::centered(None, size(px(900.0), px(640.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| Gallery {
                    current: 0,
                    spring: SpringState::default(),
                    toggle: ToggleState::default(),
                    stagger: StaggerState::default(),
                    toast: ToastState::default(),
                    modal: ModalState::default(),
                    sheet: SheetState::default(),
                    page: PageState::default(),
                    seg: SegState::default(),
                    skeleton: SkeletonState::default(),
                    bounce: BounceState::default(),
                    zoom: ZoomState::default(),
                    carousel: CarouselState::default(),
                    swipe: SwipeState { items: vec![0, 1, 2, 3], removing: None, tick: 0 },
                    ctx_menu: CtxMenuState::default(),
                    long_press: LongPressState::default(),
                    tilt: TiltState::default(),
                    stretch: StretchState::default(),
                })
            },
        )
        .expect("falha ao abrir janela GPUI");
        cx.activate(true);
    });
}
