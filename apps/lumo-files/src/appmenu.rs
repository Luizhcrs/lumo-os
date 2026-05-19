//! appmenu.rs -- Exporta menu via com.canonical.dbusmenu + registra no AppMenu.Registrar.
//!
//! A bar Lumo busca o menu pelo PID via GetMenuForWindow(pid).
//! Thread dedicada serve o menu e encaminha clicks como MenuAction pro loop iced.

use std::collections::HashMap;
use std::sync::OnceLock;

use zbus::blocking::connection::Builder;
use zbus::zvariant::{OwnedValue, StructureBuilder, Value};

use crate::app::Message;

// ---------------------------------------------------------------------------
// MenuAction -- dispatched to iced update loop
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum MenuAction {
    NewWindow,
    NewFolder,
    Quit,
    Cut,
    Copy,
    Paste,
    SelectAll,
    Refresh,
    ToggleHidden,
    ShowAbout,
    ShowShortcuts,
}

// ---------------------------------------------------------------------------
// Menu tree
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
enum ItemKind {
    Submenu,
    Action,
    Separator,
}

#[derive(Debug, Clone)]
struct MenuItem {
    id: i32,
    label: &'static str,
    parent_id: i32,
    kind: ItemKind,
}

fn build_menu() -> Vec<MenuItem> {
    vec![
        MenuItem { id: 0,  label: "",                 parent_id: -1, kind: ItemKind::Submenu   },
        MenuItem { id: 1,  label: "File",             parent_id: 0,  kind: ItemKind::Submenu   },
        MenuItem { id: 10, label: "Nova janela",      parent_id: 1,  kind: ItemKind::Action    },
        MenuItem { id: 11, label: "Nova pasta",       parent_id: 1,  kind: ItemKind::Action    },
        MenuItem { id: 12, label: "",                 parent_id: 1,  kind: ItemKind::Separator },
        MenuItem { id: 13, label: "Sair",             parent_id: 1,  kind: ItemKind::Action    },
        MenuItem { id: 2,  label: "Edit",             parent_id: 0,  kind: ItemKind::Submenu   },
        MenuItem { id: 20, label: "Recortar",         parent_id: 2,  kind: ItemKind::Action    },
        MenuItem { id: 21, label: "Copiar",           parent_id: 2,  kind: ItemKind::Action    },
        MenuItem { id: 22, label: "Colar",            parent_id: 2,  kind: ItemKind::Action    },
        MenuItem { id: 23, label: "",                 parent_id: 2,  kind: ItemKind::Separator },
        MenuItem { id: 24, label: "Selecionar tudo",  parent_id: 2,  kind: ItemKind::Action    },
        MenuItem { id: 3,  label: "View",             parent_id: 0,  kind: ItemKind::Submenu   },
        MenuItem { id: 30, label: "Atualizar",        parent_id: 3,  kind: ItemKind::Action    },
        MenuItem { id: 31, label: "",                 parent_id: 3,  kind: ItemKind::Separator },
        MenuItem { id: 32, label: "Mostrar ocultos",  parent_id: 3,  kind: ItemKind::Action    },
        MenuItem { id: 4,  label: "Help",             parent_id: 0,  kind: ItemKind::Submenu   },
        MenuItem { id: 40, label: "Sobre lumo-files", parent_id: 4,  kind: ItemKind::Action    },
        MenuItem { id: 41, label: "Atalhos teclado",  parent_id: 4,  kind: ItemKind::Action    },
    ]
}

fn id_to_action(id: i32) -> Option<MenuAction> {
    match id {
        10 => Some(MenuAction::NewWindow),
        11 => Some(MenuAction::NewFolder),
        13 => Some(MenuAction::Quit),
        20 => Some(MenuAction::Cut),
        21 => Some(MenuAction::Copy),
        22 => Some(MenuAction::Paste),
        24 => Some(MenuAction::SelectAll),
        30 => Some(MenuAction::Refresh),
        32 => Some(MenuAction::ToggleHidden),
        40 => Some(MenuAction::ShowAbout),
        41 => Some(MenuAction::ShowShortcuts),
        _  => None,
    }
}

// ---------------------------------------------------------------------------
// DBus interface
// ---------------------------------------------------------------------------

struct LumoFilesMenu {
    items: Vec<MenuItem>,
    revision: std::sync::atomic::AtomicU32,
    tx: std::sync::mpsc::Sender<MenuAction>,
}

impl LumoFilesMenu {
    fn new(tx: std::sync::mpsc::Sender<MenuAction>) -> Self {
        Self { items: build_menu(), revision: std::sync::atomic::AtomicU32::new(1), tx }
    }

    /// Build layout node for `id` up to `depth` levels deep.
    /// depth=-1 means unlimited; depth=0 means no children.
    fn layout_node(
        &self,
        id: i32,
        depth: i32,
    ) -> (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>) {
        let item_opt = self.items.iter().find(|it| it.id == id);
        let mut props: HashMap<String, OwnedValue> = HashMap::new();

        if let Some(item) = item_opt {
            match item.kind {
                ItemKind::Separator => {
                    props.insert(
                        "type".into(),
                        OwnedValue::try_from(Value::from("separator")).unwrap(),
                    );
                }
                ItemKind::Action | ItemKind::Submenu => {
                    props.insert(
                        "label".into(),
                        OwnedValue::try_from(Value::from(item.label)).unwrap(),
                    );
                    if matches!(item.kind, ItemKind::Submenu) && id != 0 {
                        props.insert(
                            "children-display".into(),
                            OwnedValue::try_from(Value::from("submenu")).unwrap(),
                        );
                    }
                }
            }
        }

        let children: Vec<OwnedValue> = if depth != 0 {
            let next_depth = if depth < 0 { -1 } else { depth - 1 };
            self.items
                .iter()
                .filter(|it| it.parent_id == id)
                .map(|it| {
                    let (cid, cprops, cchildren) = self.layout_node(it.id, next_depth);
                    let child_struct = StructureBuilder::new()
                        .add_field(cid)
                        .add_field(cprops)
                        .add_field(cchildren)
                        .build()
                        .unwrap();
                    OwnedValue::try_from(Value::Structure(child_struct)).unwrap()
                })
                .collect()
        } else {
            Vec::new()
        };

        (id, props, children)
    }

    fn item_props(&self, id: i32) -> HashMap<String, OwnedValue> {
        let mut props: HashMap<String, OwnedValue> = HashMap::new();
        if let Some(item) = self.items.iter().find(|it| it.id == id) {
            match item.kind {
                ItemKind::Separator => {
                    props.insert(
                        "type".into(),
                        OwnedValue::try_from(Value::from("separator")).unwrap(),
                    );
                }
                _ => {
                    props.insert(
                        "label".into(),
                        OwnedValue::try_from(Value::from(item.label)).unwrap(),
                    );
                }
            }
        }
        props
    }
}

#[zbus::interface(name = "com.canonical.dbusmenu")]
impl LumoFilesMenu {
    #[zbus(property)]
    fn version(&self) -> u32 {
        3
    }

    #[zbus(property)]
    fn text_direction(&self) -> &str {
        "ltr"
    }

    #[zbus(property)]
    fn status(&self) -> &str {
        "normal"
    }

    fn get_layout(
        &self,
        parent_id: i32,
        recursion_depth: i32,
        _property_names: Vec<String>,
    ) -> zbus::fdo::Result<(u32, (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>))> {
        let node = self.layout_node(parent_id, recursion_depth);
        Ok((self.revision.load(std::sync::atomic::Ordering::Relaxed), node))
    }

    fn get_group_properties(
        &self,
        ids: Vec<i32>,
        _property_names: Vec<String>,
    ) -> zbus::fdo::Result<Vec<(i32, HashMap<String, OwnedValue>)>> {
        Ok(ids.into_iter().map(|id| (id, self.item_props(id))).collect())
    }

    fn event(
        &self,
        id: i32,
        event_id: String,
        _data: OwnedValue,
        _timestamp: u32,
    ) -> zbus::fdo::Result<()> {
        if event_id == "clicked" {
            if let Some(action) = id_to_action(id) {
                let _ = self.tx.send(action);
            }
        }
        Ok(())
    }

    fn event_group(
        &self,
        events: Vec<(i32, String, OwnedValue, u32)>,
    ) -> zbus::fdo::Result<Vec<i32>> {
        for (id, event_id, _data, _ts) in events {
            if event_id == "clicked" {
                if let Some(action) = id_to_action(id) {
                    let _ = self.tx.send(action);
                }
            }
        }
        Ok(Vec::new())
    }

    fn about_to_show(&self, id: i32) -> zbus::fdo::Result<bool> {
        // Return true for submenus so clients refetch when menu becomes dynamic.
        let is_submenu = self.items.iter().any(|it| it.id == id && matches!(it.kind, ItemKind::Submenu));
        Ok(is_submenu)
    }

    fn about_to_show_group(
        &self,
        ids: Vec<i32>,
    ) -> zbus::fdo::Result<(Vec<i32>, Vec<i32>)> {
        let _ = ids;
        Ok((Vec::new(), Vec::new()))
    }
}

// ---------------------------------------------------------------------------
// Serve: blocking thread entry point
// ---------------------------------------------------------------------------

const MENU_PATH: &str = "/com/lumo/lumo_files/menus/main";

/// Blocking. Run in a dedicated std::thread::spawn.
pub fn serve(tx: std::sync::mpsc::Sender<MenuAction>) {
    if let Err(e) = serve_inner(tx) {
        eprintln!("[appmenu] serve encerrou: {}", e);
    }
}

fn serve_inner(tx: std::sync::mpsc::Sender<MenuAction>) -> Result<(), Box<dyn std::error::Error>> {
    let pid = std::process::id();
    let iface = LumoFilesMenu::new(tx);

    let conn = Builder::session()?
        .serve_at(MENU_PATH, iface)?
        .build()?;

    let service_name = conn
        .unique_name()
        .map(|n| n.to_string())
        .unwrap_or_default();

    eprintln!("[appmenu] dbusmenu ativo em {} service={}", MENU_PATH, service_name);

    {
        let registrar = zbus::blocking::Proxy::new(
            &conn,
            "com.canonical.AppMenu.Registrar",
            "/com/canonical/AppMenu/Registrar",
            "com.canonical.AppMenu.Registrar",
        )?;
        let menu_path = zbus::zvariant::ObjectPath::try_from(MENU_PATH)?;
        let _: () = registrar.call("RegisterWindow", &(pid, menu_path))?;
        eprintln!("[appmenu] RegisterWindow(pid={}) OK", pid);
    }

    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}

// ---------------------------------------------------------------------------
// Channel init + iced Subscription bridge
// ---------------------------------------------------------------------------

static MENU_RX: OnceLock<tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<MenuAction>>> =
    OnceLock::new();

static MENU_TOK_TX: OnceLock<tokio::sync::mpsc::UnboundedSender<MenuAction>> =
    OnceLock::new();

/// Init channel. Returns std Sender for the zbus blocking thread.
/// Bridges to tokio UnboundedSender for zero-polling iced subscription.
pub fn init_channel() -> std::sync::mpsc::Sender<MenuAction> {
    let (std_tx, std_rx) = std::sync::mpsc::channel::<MenuAction>();
    let (tok_tx, tok_rx) = tokio::sync::mpsc::unbounded_channel::<MenuAction>();
    MENU_RX.set(tokio::sync::Mutex::new(tok_rx)).expect("init_channel called twice");
    MENU_TOK_TX.set(tok_tx).expect("init_channel tx called twice");
    // Bridge: forward std mpsc -> tokio unbounded
    std::thread::Builder::new()
        .name("appmenu-bridge".into())
        .spawn(move || {
            for action in std_rx {
                if let Some(tx) = MENU_TOK_TX.get() {
                    let _ = tx.send(action);
                }
            }
        })
        .expect("spawn appmenu-bridge");
    std_tx
}

/// Iced Subscription: event-driven, zero 50ms polling.
pub fn appmenu_subscription() -> iced::Subscription<Message> {
    use futures::stream::StreamExt as _;

    iced::Subscription::run_with_id(
        "appmenu-dbus",
        futures::stream::unfold((), |()| async {
            let action = if let Some(rx) = MENU_RX.get() {
                rx.lock().await.recv().await
            } else {
                None
            };
            Some((action, ()))
        })
        .filter_map(|opt| async move { opt })
        .map(|action| match action {
            MenuAction::NewWindow     => Message::AppMenuNewWindow,
            MenuAction::NewFolder     => Message::NewFolder,
            MenuAction::Quit          => Message::AppMenuQuit,
            MenuAction::Cut           => Message::CutSelected,
            MenuAction::Copy          => Message::CopySelected,
            MenuAction::Paste         => Message::Paste,
            MenuAction::SelectAll     => Message::AppMenuSelectAll,
            MenuAction::Refresh       => Message::Refresh,
            MenuAction::ToggleHidden  => Message::AppMenuToggleHidden,
            MenuAction::ShowAbout     => Message::AppMenuShowAbout,
            MenuAction::ShowShortcuts => Message::AppMenuShowShortcuts,
        }),
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_menu_count() {
        let menu = build_menu();
        // root(1) + File(1+4) + Edit(1+5) + View(1+3) + Help(1+2) = 19
        assert_eq!(menu.len(), 19);
    }

    #[test]
    fn test_build_menu_top_level() {
        let menu = build_menu();
        let top: Vec<_> = menu.iter().filter(|it| it.parent_id == 0).collect();
        assert_eq!(top.len(), 4);
        let labels: Vec<_> = top.iter().map(|it| it.label).collect();
        assert!(labels.contains(&"File"));
        assert!(labels.contains(&"Edit"));
        assert!(labels.contains(&"View"));
        assert!(labels.contains(&"Help"));
    }

    #[test]
    fn test_id_to_action_all_11() {
        let ids = [10, 11, 13, 20, 21, 22, 24, 30, 32, 40, 41];
        for &id in &ids {
            assert!(id_to_action(id).is_some(), "id {} sem action", id);
        }
        assert!(id_to_action(0).is_none());
        assert!(id_to_action(1).is_none());
        assert!(id_to_action(12).is_none());
        assert!(id_to_action(23).is_none());
    }

    #[test]
    fn test_file_menu_children() {
        let menu = build_menu();
        let ch: Vec<_> = menu.iter().filter(|it| it.parent_id == 1).collect();
        assert_eq!(ch.len(), 4);
    }

    #[test]
    fn test_layout_node_root_4_children() {
        let (tx, _rx) = std::sync::mpsc::channel::<MenuAction>();
        let m = LumoFilesMenu::new(tx);
        let (_id, _props, children) = m.layout_node(0, 1);
        assert_eq!(children.len(), 4);
    }
}
