//! W37.22: Integration test pra state machine do context menu desktop.
//!
//! Casos de uso:
//!   - Right-click empty area abre menu (state machine)
//!   - Right-click sobre icon abre ctx_menu
//!   - Left-click fora do menu fecha
//!   - IPC CloseDesktopMenu fecha estado
//!   - IPC CloseDropdowns NAO fecha (W37.5 fix)

use lumo_ipc::LumoEvent;
use lumo_shell::desktop::state::apply_event;
use lumo_shell::desktop::icons::{ctx_menu_h, ctx_menu_hit, CTX_MENU_W};

#[test]
fn close_desktop_menu_triggers_close() {
    let mut close = false;
    let mut open_sel = false;
    let mut theme = None;
    apply_event(LumoEvent::CloseDesktopMenu, &mut close, &mut open_sel, &mut theme);
    assert!(close, "CloseDesktopMenu deve setar close_menu=true");
    assert!(!open_sel);
    assert!(theme.is_none());
}

#[test]
fn close_dropdowns_does_not_trigger_close() {
    // W37.5: lumo-desktop envia CloseDropdowns pro bar, compositor broadcasta
    // de volta. Desktop nao pode fechar proprio menu nesse echo.
    let mut close = false;
    let mut open_sel = false;
    let mut theme = None;
    apply_event(LumoEvent::CloseDropdowns, &mut close, &mut open_sel, &mut theme);
    assert!(!close, "CloseDropdowns NAO deve setar close_menu");
}

#[test]
fn desktop_open_selected_triggers_flag() {
    let mut close = false;
    let mut open_sel = false;
    let mut theme = None;
    apply_event(
        LumoEvent::DesktopOpenSelected,
        &mut close,
        &mut open_sel,
        &mut theme,
    );
    assert!(open_sel);
    assert!(!close);
}

#[test]
fn unrelated_event_no_side_effects() {
    let mut close = false;
    let mut open_sel = false;
    let mut theme = None;
    apply_event(
        LumoEvent::Workspaces { active: 1, total: 4 },
        &mut close,
        &mut open_sel,
        &mut theme,
    );
    assert!(!close);
    assert!(!open_sel);
    assert!(theme.is_none());
}

#[test]
fn ctx_menu_hit_above_origin_returns_none() {
    // Cursor acima do menu_y -> None.
    let menu_x = 100.0;
    let menu_y = 200.0;
    let result = ctx_menu_hit(menu_x + 10.0, menu_y - 5.0, menu_x, menu_y);
    assert!(result.is_none());
}

#[test]
fn ctx_menu_hit_left_of_origin_returns_none() {
    let menu_x = 100.0;
    let menu_y = 200.0;
    let result = ctx_menu_hit(menu_x - 5.0, menu_y + 10.0, menu_x, menu_y);
    assert!(result.is_none());
}

#[test]
fn ctx_menu_hit_inside_returns_some() {
    let menu_x = 100.0;
    let menu_y = 200.0;
    let result = ctx_menu_hit(menu_x + CTX_MENU_W / 2.0, menu_y + 15.0, menu_x, menu_y);
    assert!(result.is_some(), "click dentro do menu deve retornar item idx");
}

#[test]
fn ctx_menu_height_positive() {
    // Sanity: altura sempre > 0 (item_h * count + pad).
    assert!(ctx_menu_h() > 0.0);
}
