//! Hot-reload watcher do lumo.css.

use crate::Stylesheet;
use std::path::PathBuf;
use std::sync::mpsc;

/// Spawn thread watching default_path(). Cada change re-parse + send.
/// Receiver bar uses try_recv() em tick pra detect mudancas.
pub fn spawn_watcher() -> Option<mpsc::Receiver<Stylesheet>> {
    let path = crate::default_path()?;
    let (tx, rx) = mpsc::channel::<Stylesheet>();
    let p = path.clone();
    std::thread::spawn(move || {
        use notify::{Config, RecursiveMode, Watcher};
        let (notify_tx, notify_rx) = mpsc::channel();
        let mut watcher = match notify::recommended_watcher(notify_tx) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("[lumo-style] watcher init: {:?}", e);
                return;
            }
        };
        if let Some(parent) = p.parent() {
            let _ = watcher.watch(parent, RecursiveMode::NonRecursive);
        }
        eprintln!("[lumo-style] watching {}", p.display());
        for ev in notify_rx {
            match ev {
                Ok(event) => {
                    let touched = event.paths.iter().any(|pp| pp == &p);
                    if !touched {
                        continue;
                    }
                    match crate::load_from_disk() {
                        Ok(sheet) => {
                            eprintln!("[lumo-style] reload OK ({} rules)", sheet.rules.len());
                            let _ = tx.send(sheet);
                        }
                        Err(e) => eprintln!("[lumo-style] reload erro: {:?}", e),
                    }
                }
                Err(e) => eprintln!("[lumo-style] notify err: {:?}", e),
            }
        }
    });
    Some(rx)
}
