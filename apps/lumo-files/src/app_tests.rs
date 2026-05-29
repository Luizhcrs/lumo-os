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

    // W37: context menu fix - abrir Item/Area + nao fechar em key press
    #[tokio::test]
    async fn test_context_menu_abre_em_area_vazia() {
        use crate::app::ContextMenu;
        let temp = tempfile::tempdir().unwrap();
        let (mut app, _) = App::new_with_dir(temp.path().to_path_buf());

        let _ = app.update(Message::ContextMenuOpen(ContextMenu::Area {
            x: 500.0,
            y: 300.0,
        }));

        assert!(matches!(app.context_menu, Some(ContextMenu::Area { .. })));
    }

    #[tokio::test]
    async fn test_context_menu_abre_em_item() {
        use crate::app::ContextMenu;
        let temp = tempfile::tempdir().unwrap();
        let (mut app, _) = App::new_with_dir(temp.path().to_path_buf());

        let _ = app.update(Message::ContextMenuOpen(ContextMenu::Item {
            x: 100.0,
            y: 200.0,
        }));

        assert!(matches!(app.context_menu, Some(ContextMenu::Item { .. })));
    }

    // W38: right-click sobre item seleciona o item E abre menu Item (antes so
    // lia selecao previa -> vazio -> menu Area com ops greyed = "bugado").
    #[tokio::test]
    async fn test_right_click_item_seleciona_e_abre_menu_item() {
        use crate::app::ContextMenu;
        let temp = tempfile::tempdir().unwrap();
        let (mut app, _) = App::new_with_dir(temp.path().to_path_buf());

        // Sem selecao previa: right-click sobre idx 3 deve selecionar idx 3.
        assert!(app.current_tab().file_list.selected.is_empty());
        let _ = app.update(Message::ItemRightClicked(3));

        assert!(
            matches!(app.context_menu, Some(ContextMenu::Item { .. })),
            "right-click em item deve abrir menu Item, nao Area"
        );
        assert!(
            app.current_tab().file_list.selected.contains(&3),
            "right-click deve selecionar o item sob o cursor"
        );
        assert_eq!(
            app.current_tab().file_list.selected.len(),
            1,
            "right-click e single-select"
        );
    }

    // W38: right-click sobre item B troca a selecao (nao opera no A anterior).
    #[tokio::test]
    async fn test_right_click_item_troca_selecao_anterior() {
        let temp = tempfile::tempdir().unwrap();
        let (mut app, _) = App::new_with_dir(temp.path().to_path_buf());

        let _ = app.update(Message::ItemClicked { idx: 1, ctrl: false, shift: false });
        assert!(app.current_tab().file_list.selected.contains(&1));

        let _ = app.update(Message::ItemRightClicked(5));
        assert!(app.current_tab().file_list.selected.contains(&5));
        assert!(!app.current_tab().file_list.selected.contains(&1));
    }

    #[tokio::test]
    async fn test_context_menu_close_message() {
        use crate::app::ContextMenu;
        let temp = tempfile::tempdir().unwrap();
        let (mut app, _) = App::new_with_dir(temp.path().to_path_buf());

        let _ = app.update(Message::ContextMenuOpen(ContextMenu::Area {
            x: 0.0,
            y: 0.0,
        }));
        assert!(app.context_menu.is_some());

        let _ = app.update(Message::ContextMenuClose);
        assert!(app.context_menu.is_none());
    }

    #[tokio::test]
    async fn test_context_menu_persiste_apos_key_nao_escape() {
        // W37: bug fix - modifier key emitido por right-click nao deve fechar menu.
        use crate::app::ContextMenu;
        use iced::keyboard::{key::Named, Key, Modifiers};
        let temp = tempfile::tempdir().unwrap();
        let (mut app, _) = App::new_with_dir(temp.path().to_path_buf());

        let _ = app.update(Message::ContextMenuOpen(ContextMenu::Area {
            x: 100.0,
            y: 100.0,
        }));
        assert!(app.context_menu.is_some());

        // Tecla generica (modificador-like) nao fecha menu
        let _ = app.update(Message::KeyPressed(Key::Named(Named::Control), Modifiers::CTRL));
        assert!(
            app.context_menu.is_some(),
            "menu nao deve fechar em tecla nao-Escape"
        );

        // Escape fecha
        let _ = app.update(Message::KeyPressed(Key::Named(Named::Escape), Modifiers::default()));
        assert!(app.context_menu.is_none(), "Escape fecha menu");
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
