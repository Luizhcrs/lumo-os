//! appmenu.rs -- DBus appmenu para lumo-settings.
//!
//! Panic policy:
//! - OwnedValue::try_from(Value::from(str)) panic mathematically impossible
//!   pra strings/structures bem-formados (variant valido). Mantemos .unwrap()
//!   sem codigo.
//! - OnceLock.set(...).unwrap() panic so se chamado >1x (codigo bug). Init-
//!   time, capturado por panic_hook (S1).
//! - .expect("[APP-MENU-NNN] ...") = fatal init com codigo.

use std::collections::HashMap;
use std::sync::OnceLock;

use zbus::blocking::connection::Builder;
use zbus::zvariant::{OwnedValue, StructureBuilder, Value};

use crate::app::Message;

#[derive(Debug, Clone)]
pub enum MenuAction {
    Quit,
    ShowAbout,
}

#[derive(Debug, Clone, Copy)]
enum ItemKind {
    Submenu,
    Action,
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
        MenuItem {
            id: 0,
            label: "",
            parent_id: -1,
            kind: ItemKind::Submenu,
        },
        MenuItem {
            id: 1,
            label: "File",
            parent_id: 0,
            kind: ItemKind::Submenu,
        },
        MenuItem {
            id: 10,
            label: "Sair",
            parent_id: 1,
            kind: ItemKind::Action,
        },
        MenuItem {
            id: 2,
            label: "Help",
            parent_id: 0,
            kind: ItemKind::Submenu,
        },
        MenuItem {
            id: 20,
            label: "Sobre lumo-settings",
            parent_id: 2,
            kind: ItemKind::Action,
        },
    ]
}

fn id_to_action(id: i32) -> Option<MenuAction> {
    match id {
        10 => Some(MenuAction::Quit),
        20 => Some(MenuAction::ShowAbout),
        _ => None,
    }
}

struct LumoSettingsMenu {
    items: Vec<MenuItem>,
    revision: std::sync::atomic::AtomicU32,
    tx: std::sync::mpsc::Sender<MenuAction>,
}

impl LumoSettingsMenu {
    fn new(tx: std::sync::mpsc::Sender<MenuAction>) -> Self {
        Self {
            items: build_menu(),
            revision: std::sync::atomic::AtomicU32::new(1),
            tx,
        }
    }

    fn layout_node(
        &self,
        id: i32,
        depth: i32,
    ) -> (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>) {
        let item_opt = self.items.iter().find(|it| it.id == id);
        let mut props: HashMap<String, OwnedValue> = HashMap::new();
        if let Some(item) = item_opt {
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
        let children: Vec<OwnedValue> = if depth != 0 {
            let next_depth = if depth < 0 { -1 } else { depth - 1 };
            self.items
                .iter()
                .filter(|it| it.parent_id == id)
                .map(|it| {
                    let (cid, cprops, cchildren) = self.layout_node(it.id, next_depth);
                    let s = StructureBuilder::new()
                        .add_field(cid)
                        .add_field(cprops)
                        .add_field(cchildren)
                        .build()
                        .unwrap();
                    OwnedValue::try_from(Value::Structure(s)).unwrap()
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
            props.insert(
                "label".into(),
                OwnedValue::try_from(Value::from(item.label)).unwrap(),
            );
        }
        props
    }
}

#[zbus::interface(name = "com.canonical.dbusmenu")]
impl LumoSettingsMenu {
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
        _names: Vec<String>,
    ) -> zbus::fdo::Result<(u32, (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>))> {
        let node = self.layout_node(parent_id, recursion_depth);
        Ok((
            self.revision.load(std::sync::atomic::Ordering::Relaxed),
            node,
        ))
    }

    fn get_group_properties(
        &self,
        ids: Vec<i32>,
        _names: Vec<String>,
    ) -> zbus::fdo::Result<Vec<(i32, HashMap<String, OwnedValue>)>> {
        Ok(ids
            .into_iter()
            .map(|id| (id, self.item_props(id)))
            .collect())
    }

    fn event(
        &self,
        id: i32,
        event_id: String,
        _data: OwnedValue,
        _timestamp: u32,
    ) -> zbus::fdo::Result<()> {
        if event_id == "clicked" {
            if let Some(a) = id_to_action(id) {
                let _ = self.tx.send(a);
            }
        }
        Ok(())
    }

    fn event_group(
        &self,
        events: Vec<(i32, String, OwnedValue, u32)>,
    ) -> zbus::fdo::Result<Vec<i32>> {
        for (id, ev, _, _) in events {
            if ev == "clicked" {
                if let Some(a) = id_to_action(id) {
                    let _ = self.tx.send(a);
                }
            }
        }
        Ok(Vec::new())
    }

    fn about_to_show(&self, id: i32) -> zbus::fdo::Result<bool> {
        Ok(self
            .items
            .iter()
            .any(|it| it.id == id && matches!(it.kind, ItemKind::Submenu)))
    }

    fn about_to_show_group(&self, ids: Vec<i32>) -> zbus::fdo::Result<(Vec<i32>, Vec<i32>)> {
        let _ = ids;
        Ok((Vec::new(), Vec::new()))
    }
}

const MENU_PATH: &str = "/com/lumo/lumo_settings/menus/main";

pub fn serve(tx: std::sync::mpsc::Sender<MenuAction>) {
    if let Err(e) = serve_inner(tx) {
        eprintln!("[appmenu] encerrou: {}", e);
    }
}

fn serve_inner(tx: std::sync::mpsc::Sender<MenuAction>) -> Result<(), Box<dyn std::error::Error>> {
    let pid = std::process::id();
    let iface = LumoSettingsMenu::new(tx);
    let conn = Builder::session()?.serve_at(MENU_PATH, iface)?.build()?;
    let svc = conn
        .unique_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    eprintln!("[appmenu] dbusmenu ativo em {} service={}", MENU_PATH, svc);
    {
        let reg = zbus::blocking::Proxy::new(
            &conn,
            "com.canonical.AppMenu.Registrar",
            "/com/canonical/AppMenu/Registrar",
            "com.canonical.AppMenu.Registrar",
        )?;
        let mp = zbus::zvariant::ObjectPath::try_from(MENU_PATH)?;
        let _: () = reg.call("RegisterWindow", &(pid, mp))?;
    }
    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}

static MENU_RX: OnceLock<tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<MenuAction>>> =
    OnceLock::new();
static MENU_TOK_TX: OnceLock<tokio::sync::mpsc::UnboundedSender<MenuAction>> = OnceLock::new();

pub fn init_channel() -> std::sync::mpsc::Sender<MenuAction> {
    let (std_tx, std_rx) = std::sync::mpsc::channel::<MenuAction>();
    let (tok_tx, tok_rx) = tokio::sync::mpsc::unbounded_channel::<MenuAction>();
    MENU_RX
        .set(tokio::sync::Mutex::new(tok_rx))
        .expect("[APP-MENU-001] init_channel called twice");
    MENU_TOK_TX
        .set(tok_tx)
        .expect("[APP-MENU-002] init_channel tx called twice");
    std::thread::Builder::new()
        .name("settings-appmenu-bridge".into())
        .spawn(move || {
            for action in std_rx {
                if let Some(tx) = MENU_TOK_TX.get() {
                    let _ = tx.send(action);
                }
            }
        })
        .expect("spawn bridge");
    std_tx
}

pub fn appmenu_subscription() -> iced::Subscription<Message> {
    use futures::stream::StreamExt as _;
    iced::Subscription::run_with_id(
        "settings-appmenu-dbus",
        futures::stream::unfold((), |()| async {
            let a = if let Some(rx) = MENU_RX.get() {
                rx.lock().await.recv().await
            } else {
                None
            };
            Some((a, ()))
        })
        .filter_map(|opt| async move { opt })
        .map(|a| match a {
            MenuAction::Quit => Message::Quit,
            MenuAction::ShowAbout => Message::ShowAbout,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_menu_count() {
        assert_eq!(build_menu().len(), 5);
    }

    #[test]
    fn test_action_quit() {
        assert!(matches!(id_to_action(10), Some(MenuAction::Quit)));
    }

    #[test]
    fn test_action_about() {
        assert!(matches!(id_to_action(20), Some(MenuAction::ShowAbout)));
    }

    #[test]
    fn test_action_unknown() {
        assert!(id_to_action(999).is_none());
    }
}
