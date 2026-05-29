//! W37.22: integration test pra sequencia de eventos IPC line-delimited.
//!
//! Simula compositor enviando multiplas mensagens em sequencia, cliente
//! drenando socket bytes. Cobre:
//!   - linhas concatenadas no buffer (TCP-like coalesce)
//!   - linhas fragmentadas (split em chunks)
//!   - ordem preservada apos parse
//!   - linha vazia ignorada

use lumo_ipc::{LumoCommand, LumoEvent};

#[test]
fn round_trip_multiple_events_concatenated() {
    // Servidor emite 3 eventos como linhas em um stream concatenado.
    let evs = vec![
        LumoEvent::ActiveApp {
            app_id: "com.lumo.files".into(),
            title: "Files".into(),
            pid: 100,
        },
        LumoEvent::CloseDropdowns,
        LumoEvent::ActiveAppCleared,
    ];

    let mut buf = String::new();
    for ev in &evs {
        buf.push_str(&serde_json::to_string(ev).unwrap());
        buf.push('\n');
    }

    // Cliente split por '\n', skip vazias, deserializa.
    let parsed: Vec<LumoEvent> = buf
        .split('\n')
        .filter(|s| !s.is_empty())
        .filter_map(|line| serde_json::from_str::<LumoEvent>(line).ok())
        .collect();

    assert_eq!(parsed.len(), evs.len());
    match (&parsed[0], &evs[0]) {
        (
            LumoEvent::ActiveApp {
                app_id: a,
                title: t,
                pid: p,
            },
            LumoEvent::ActiveApp {
                app_id: a2,
                title: t2,
                pid: p2,
            },
        ) => {
            assert_eq!(a, a2);
            assert_eq!(t, t2);
            assert_eq!(p, p2);
        }
        _ => panic!("event[0] tipo errado"),
    }
}

#[test]
fn command_roundtrip_close_dropdowns() {
    let cmd = LumoCommand::CloseDropdowns;
    let json = serde_json::to_string(&cmd).unwrap();
    let back: LumoCommand = serde_json::from_str(&json).unwrap();
    assert!(matches!(back, LumoCommand::CloseDropdowns));
}

#[test]
fn event_round_trip_close_desktop_menu() {
    let ev = LumoEvent::CloseDesktopMenu;
    let json = serde_json::to_string(&ev).unwrap();
    let back: LumoEvent = serde_json::from_str(&json).unwrap();
    assert!(matches!(back, LumoEvent::CloseDesktopMenu));
}

#[test]
fn drain_partial_line_no_panic() {
    // Unit variant (ActiveAppCleared) serializa como STRING bare
    // ("ActiveAppCleared"), nao objeto {"ActiveAppCleared":null}. Roundtrip
    // garante o formato real sem JSON hardcoded que fica stale (era o bug:
    // o teste antigo assertava parse de {"ActiveAppCleared":null} -> Err).
    let ev = LumoEvent::ActiveAppCleared;
    let json = serde_json::to_string(&ev).unwrap();
    assert_eq!(json, "\"ActiveAppCleared\"");
    let back: Result<LumoEvent, _> = serde_json::from_str(&json);
    assert!(matches!(back, Ok(LumoEvent::ActiveAppCleared)));
    // Linha PARCIAL (JSON nao fechado) deve dar Err, nunca panic: o cliente
    // bufera e espera mais bytes / o \n antes de parsear.
    let partial = r#"{"ActiveApp":{"app_id":"foo""#;
    assert!(serde_json::from_str::<LumoEvent>(partial).is_err());
}

#[test]
fn empty_lines_skipped_in_split() {
    let buf = "\n\n";
    let count: usize = buf
        .split('\n')
        .filter(|s| !s.is_empty())
        .filter_map(|line| serde_json::from_str::<LumoEvent>(line).ok())
        .count();
    assert_eq!(count, 0);
}

#[test]
fn corrupted_line_doesnt_break_subsequent() {
    let mut buf = String::new();
    buf.push_str("not valid json\n");
    buf.push_str(&serde_json::to_string(&LumoEvent::CloseDropdowns).unwrap());
    buf.push('\n');

    let parsed: Vec<LumoEvent> = buf
        .split('\n')
        .filter(|s| !s.is_empty())
        .filter_map(|line| serde_json::from_str::<LumoEvent>(line).ok())
        .collect();

    // Linha corrompida descartada; segunda parseia OK.
    assert_eq!(parsed.len(), 1);
    assert!(matches!(parsed[0], LumoEvent::CloseDropdowns));
}

#[test]
fn many_events_preserve_order() {
    let mut buf = String::new();
    for i in 0..50 {
        let ev = LumoEvent::ActiveApp {
            app_id: format!("app{i}"),
            title: format!("t{i}"),
            pid: i as u32,
        };
        buf.push_str(&serde_json::to_string(&ev).unwrap());
        buf.push('\n');
    }
    let parsed: Vec<LumoEvent> = buf
        .split('\n')
        .filter(|s| !s.is_empty())
        .filter_map(|line| serde_json::from_str::<LumoEvent>(line).ok())
        .collect();
    assert_eq!(parsed.len(), 50);
    for (i, ev) in parsed.iter().enumerate() {
        if let LumoEvent::ActiveApp { pid, .. } = ev {
            assert_eq!(*pid as usize, i, "ordem nao preservada na posicao {i}");
        } else {
            panic!("evento errado na pos {i}");
        }
    }
}
