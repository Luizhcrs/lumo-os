//! bar/registrar.rs - Servidor DBus com.canonical.AppMenu.Registrar.
//!
//! Provee o lado SERVER do protocolo AppMenu:
//!   - Apps GTK3+appmenu-gtk-module chamam RegisterWindow(window_id, path)
//!   - Bar chama GetMenuForWindow(window_id) -> (service, path)
//!   - UnregisterWindow limpa mapeamento quando app fecha
//!
//! Thread dedicada; estado compartilhado via RegistrarHandle (Arc<Mutex>).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Estado compartilhado entre o servidor DBus e o main loop da bar.
#[derive(Debug, Default)]
pub struct RegistrarState {
    /// window_id -> (service_name, object_path)
    pub registered: HashMap<u32, (String, String)>,
}

/// Handle clonavel pra ler/escrever estado do Registrar.
pub type RegistrarHandle = Arc<Mutex<RegistrarState>>;

/// Cria handle vazio. Chamado antes de spawn_registrar.
pub fn new_handle() -> RegistrarHandle {
    Arc::new(Mutex::new(RegistrarState::default()))
}

/// Spawn da thread dedicada do servidor DBus.
/// Retorna imediatamente. Thread roda ate o processo terminar.
pub fn spawn_registrar(handle: RegistrarHandle) {
    std::thread::Builder::new()
        .name("lumo-appmenu-registrar".into())
        .spawn(move || {
            if let Err(e) = run_server(handle) {
                eprintln!("[registrar] servidor DBus encerrou: {}", e);
            }
        })
        .expect("spawn registrar thread");
}

// ============================================================
// Interface DBus
// ============================================================

struct RegistrarImpl {
    state: RegistrarHandle,
}

#[zbus::interface(name = "com.canonical.AppMenu.Registrar")]
impl RegistrarImpl {
    async fn register_window(
        &mut self,
        window_id: u32,
        menu_object_path: zbus::zvariant::ObjectPath<'_>,
        #[zbus(header)] hdr: zbus::message::Header<'_>,
        #[zbus(signal_emitter)] emitter: zbus::object_server::SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        let sender = hdr.sender().map(|s| s.to_string()).unwrap_or_default();
        let path_str = menu_object_path.to_string();
        eprintln!(
            "[registrar] RegisterWindow: window_id={} sender={} path={}",
            window_id, sender, path_str
        );
        if let Ok(mut st) = self.state.lock() {
            st.registered
                .insert(window_id, (sender.clone(), path_str.clone()));
        }
        emitter
            .window_registered(window_id, &sender, menu_object_path)
            .await
            .ok();
        Ok(())
    }

    fn get_menu_for_window(
        &self,
        window_id: u32,
    ) -> zbus::fdo::Result<(String, zbus::zvariant::OwnedObjectPath)> {
        let st = self
            .state
            .lock()
            .map_err(|e| zbus::fdo::Error::Failed(format!("mutex poisoned: {}", e)))?;
        match st.registered.get(&window_id) {
            Some((svc, path)) => {
                let opath = zbus::zvariant::OwnedObjectPath::try_from(path.as_str())
                    .map_err(|e| zbus::fdo::Error::Failed(format!("bad path: {}", e)))?;
                Ok((svc.clone(), opath))
            }
            None => Err(zbus::fdo::Error::Failed(format!(
                "window {} not registered",
                window_id
            ))),
        }
    }

    async fn unregister_window(
        &mut self,
        window_id: u32,
        #[zbus(signal_emitter)] emitter: zbus::object_server::SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        eprintln!("[registrar] UnregisterWindow: window_id={}", window_id);
        if let Ok(mut st) = self.state.lock() {
            st.registered.remove(&window_id);
        }
        emitter.window_unregistered(window_id).await.ok();
        Ok(())
    }

    #[zbus(signal)]
    async fn window_registered(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        window_id: u32,
        service: &str,
        menu_object_path: zbus::zvariant::ObjectPath<'_>,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn window_unregistered(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        window_id: u32,
    ) -> zbus::Result<()>;
}

fn run_server(handle: RegistrarHandle) -> Result<(), Box<dyn std::error::Error>> {
    use zbus::blocking::connection::Builder;

    let iface = RegistrarImpl { state: handle };

    let _conn = Builder::session()?
        .serve_at("/com/canonical/AppMenu/Registrar", iface)?
        .name("com.canonical.AppMenu.Registrar")?
        .build()?;

    eprintln!("[registrar] com.canonical.AppMenu.Registrar ativo no session bus");

    // Manter thread viva — zbus processa requisicoes internamente
    // enquanto a Connection existir.
    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}
