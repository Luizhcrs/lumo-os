//! W37.22: Integration tests pra fluxo de navegacao do lumo-files.
//!
//! Cobre casos de uso reais:
//!   - Navigate -> BackStack push, ForwardStack clear
//!   - NavigateBack -> ForwardStack push, BackStack pop
//!   - NavigateForward -> BackStack push, ForwardStack pop
//!   - Navigate apos Back limpa ForwardStack (regra browser-like)
//!   - BackStack size limit (50)

use lumo_files::app::{App, Message};
use std::path::PathBuf;

fn make_app() -> (App, tempfile::TempDir) {
    let temp = tempfile::tempdir().unwrap();
    let (app, _) = App::new_with_dir(temp.path().to_path_buf());
    (app, temp)
}

#[tokio::test]
async fn fresh_app_has_empty_history() {
    let (app, _t) = make_app();
    assert!(app.current_tab().back_stack.is_empty());
    assert!(app.current_tab().forward_stack.is_empty());
}

#[tokio::test]
async fn navigate_pushes_back_clears_forward() {
    let (mut app, t) = make_app();
    let root = t.path().to_path_buf();
    let sub = root.join("sub");
    std::fs::create_dir(&sub).unwrap();

    // popular forward_stack manualmente
    app.current_tab_mut().forward_stack.push_back(PathBuf::from("/fake"));

    let _ = app.update(Message::Navigate(sub.clone()));

    assert_eq!(app.current_tab().back_stack.len(), 1);
    assert!(
        app.current_tab().forward_stack.is_empty(),
        "Navigate deve limpar forward_stack (regra browser)"
    );
}

#[tokio::test]
async fn back_pushes_forward() {
    let (mut app, t) = make_app();
    let root = t.path().to_path_buf();
    let sub = root.join("sub");
    std::fs::create_dir(&sub).unwrap();

    let _ = app.update(Message::Navigate(sub.clone()));
    assert_eq!(app.current_tab().back_stack.len(), 1);

    let _ = app.update(Message::NavigateBack);
    assert_eq!(app.current_tab().back_stack.len(), 0);
    assert_eq!(
        app.current_tab().forward_stack.len(),
        1,
        "Back deve empilhar em forward_stack"
    );
}

#[tokio::test]
async fn forward_pushes_back() {
    let (mut app, t) = make_app();
    let sub = t.path().join("sub");
    std::fs::create_dir(&sub).unwrap();

    let _ = app.update(Message::Navigate(sub.clone()));
    let _ = app.update(Message::NavigateBack);
    assert_eq!(app.current_tab().forward_stack.len(), 1);

    let _ = app.update(Message::NavigateForward);
    assert_eq!(app.current_tab().forward_stack.len(), 0);
    assert_eq!(app.current_tab().back_stack.len(), 1);
}

#[tokio::test]
async fn back_with_empty_stack_noop() {
    let (mut app, _t) = make_app();
    let before = app.current_tab().current_dir.clone();
    let _ = app.update(Message::NavigateBack);
    // current_dir nao muda; nao panica.
    assert_eq!(app.current_tab().current_dir, before);
}

#[tokio::test]
async fn forward_with_empty_stack_noop() {
    let (mut app, _t) = make_app();
    let before = app.current_tab().current_dir.clone();
    let _ = app.update(Message::NavigateForward);
    assert_eq!(app.current_tab().current_dir, before);
}

#[tokio::test]
async fn navigate_after_back_clears_forward() {
    // Caso de uso: user vai p/ A, B, volta p/ A, vai p/ C. Forward deve sumir.
    let (mut app, t) = make_app();
    let a = t.path().join("a");
    let b = t.path().join("b");
    let c = t.path().join("c");
    for p in [&a, &b, &c] {
        std::fs::create_dir(p).unwrap();
    }

    let _ = app.update(Message::Navigate(a.clone())); // root -> a
    let _ = app.update(Message::Navigate(b.clone())); // a -> b
    let _ = app.update(Message::NavigateBack); // b -> a, forward=[b]
    assert_eq!(app.current_tab().forward_stack.len(), 1);

    let _ = app.update(Message::Navigate(c.clone())); // a -> c
    assert!(
        app.current_tab().forward_stack.is_empty(),
        "Navigate apos Back deve apagar branch forward"
    );
}

#[tokio::test]
async fn back_stack_capped_at_50() {
    let (mut app, t) = make_app();
    // Navega 60 vezes pra subdir.
    for i in 0..60 {
        let p = t.path().join(format!("d{i}"));
        std::fs::create_dir(&p).unwrap();
        let _ = app.update(Message::Navigate(p));
    }
    assert!(
        app.current_tab().back_stack.len() <= 50,
        "back_stack deve clampar em 50, tem {}",
        app.current_tab().back_stack.len()
    );
}

#[tokio::test]
async fn round_trip_back_forward_preserves_path() {
    let (mut app, t) = make_app();
    let sub = t.path().join("sub");
    std::fs::create_dir(&sub).unwrap();

    let _ = app.update(Message::Navigate(sub.clone()));
    let dir_after_nav = app.current_tab().current_dir.clone();

    let _ = app.update(Message::NavigateBack);
    let _ = app.update(Message::NavigateForward);

    assert_eq!(
        app.current_tab().current_dir,
        dir_after_nav,
        "back+forward deve voltar pro mesmo path"
    );
}
