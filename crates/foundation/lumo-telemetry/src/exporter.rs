//! Unix socket exporter: streams JSON snapshots every 1s to connected clients.

use std::fs;
use std::io::Write;
use std::os::unix::net::UnixListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::store::TelemetryStore;

const SOCKET_PATH: &str = "/run/user/1000/lumo-metrics.sock";

pub fn spawn_exporter(store: Arc<Mutex<TelemetryStore>>) {
    thread::Builder::new()
        .name("lumo-telemetry-exporter".into())
        .spawn(move || run_exporter(store))
        .expect("spawn exporter thread");
}

fn run_exporter(store: Arc<Mutex<TelemetryStore>>) {
    // Remove stale socket from previous run.
    let _ = fs::remove_file(SOCKET_PATH);

    let listener = match UnixListener::bind(SOCKET_PATH) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("lumo-telemetry: failed to bind {}: {}", SOCKET_PATH, e);
            return;
        }
    };

    for stream in listener.incoming() {
        let store = Arc::clone(&store);
        match stream {
            Ok(mut conn) => {
                thread::Builder::new()
                    .name("lumo-telemetry-client".into())
                    .spawn(move || loop {
                        let snapshot = {
                            let mut s = store.lock().expect("lock store");
                            s.build_snapshot()
                        };
                        let mut line =
                            serde_json::to_string(&snapshot).unwrap_or_else(|_| "{}".into());
                        line.push('\n');
                        if conn.write_all(line.as_bytes()).is_err() {
                            break;
                        }
                        thread::sleep(Duration::from_secs(1));
                    })
                    .expect("spawn client thread");
            }
            Err(_) => break,
        }
    }
}
