//! app_tests.rs — unit tests for App::update and state management.

use super::app::{App, Message};
use std::path::PathBuf;

#[test]
fn test_app_initial_state() {
    let temp = tempfile::tempdir().unwrap();
    let (app, _) = App::new_with_dir(temp.path().to_path_buf());
    
    assert_eq!(app.tabs.len(), 1);
    assert_eq!(app.active_tab, 0);
    assert_eq!(app.current_tab().current_dir, temp.path());
    assert!(app.current_tab().back_stack.is_empty());
    assert!(app.current_tab().forward_stack.is_empty());
}

#[tokio::test]
async fn test_navigate_pushes_to_back_stack() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().to_path_buf();
    let subdir = root.join("subdir");
    std::fs::create_dir(&subdir).unwrap();

    let (mut app, _) = App::new_with_dir(root.clone());
    
    // Simula navegacao para subdir
    let _ = app.update(Message::Navigate(subdir.clone()));
    
    // Verifica se root foi para o back_stack
    assert_eq!(app.current_tab().back_stack.len(), 1);
    assert_eq!(app.current_tab().back_stack[0], root);
}

#[tokio::test]
async fn test_navigate_back_logic() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().to_path_buf();
    let subdir = root.join("subdir");
    std::fs::create_dir(&subdir).unwrap();

    let (mut app, _) = App::new_with_dir(root.clone());
    
    // 1. Navega para subdir
    let _ = app.update(Message::Navigate(subdir.clone()));
    // Mock DirLoaded para subdir
    let _ = app.update(Message::DirLoaded(subdir.clone(), vec![]));
    
    assert_eq!(app.current_tab().current_dir, subdir);
    assert_eq!(app.current_tab().back_stack.len(), 1);

    // 2. Volta
    let _ = app.update(Message::NavigateBack);
    
    // Back stack deve estar vazio (prev foi removido para ser usado)
    assert!(app.current_tab().back_stack.is_empty());
    // Forward stack deve conter o subdir que deixamos
    assert_eq!(app.current_tab().forward_stack.len(), 1);
    assert_eq!(app.current_tab().forward_stack[0], subdir);
}

#[tokio::test]
async fn test_tab_management() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().to_path_buf();
    let (mut app, _) = App::new_with_dir(root.clone());

    // New Tab
    let _ = app.update(Message::NewTab);
    assert_eq!(app.tabs.len(), 2);
    assert_eq!(app.active_tab, 1);

    // Switch Tab
    let _ = app.update(Message::SwitchTab(0));
    assert_eq!(app.active_tab, 0);

    // Close Tab
    let _ = app.update(Message::CloseTab(1));
    assert_eq!(app.tabs.len(), 1);
    assert_eq!(app.active_tab, 0);
}

#[tokio::test]
async fn test_search_toggle_clears_query() {
    let temp = tempfile::tempdir().unwrap();
    let (mut app, _) = App::new_with_dir(temp.path().to_path_buf());

    let _ = app.update(Message::ToggleSearch);
    let _ = app.update(Message::SearchChanged("lumo".to_string()));
    assert!(app.search_visible);
    assert_eq!(app.search_query, "lumo");

    let _ = app.update(Message::ToggleSearch);
    assert!(!app.search_visible);
    assert!(app.search_query.is_empty());
    }
    #[tokio::test]
    async fn test_clipboard_copy_logic() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        let file1 = root.join("file1.txt");
        std::fs::write(&file1, "content").unwrap();

        let (mut app, _) = App::new_with_dir(root.clone());

        // Mock entries loaded
        let _ = app.update(Message::DirLoaded(root.clone(), vec![file1.clone()]));

        // Seleciona file1 (index 0)
        let _ = app.update(Message::ItemClicked { idx: 0, ctrl: false, shift: false });

        // Copia selecionados
        let _ = app.update(Message::CopySelected);

        assert!(app.clipboard.is_some());
    }

    #[tokio::test]
    async fn test_error_flow_invalid_directory() {
        let (mut app, _) = App::new_with_dir(PathBuf::from("/invalid/path/that/doesnt/exist"));

        // Simula falha no carregamento
        let _ = app.update(Message::OpError("Acesso negado".to_string()));

        // O estado do app deve conter a mensagem de erro para o usuario
        assert!(!app.toasts.is_empty());
        assert!(app.toasts.items().iter().any(|t| t.message.contains("Acesso negado")));
    }
