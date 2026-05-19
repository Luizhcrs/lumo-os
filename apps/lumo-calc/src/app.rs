//! app.rs -- App principal do lumo-calc.
//!
//! Calculadora 4x5 grid. Eval via meval. History 10 entradas.

use iced::keyboard::{self, Key};
use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Alignment, Element, Length, Subscription, Task};

use crate::appmenu::appmenu_subscription;
use crate::theme::{container_bg, container_display, container_history, ButtonKind, LumoTheme};

const HISTORY_MAX: usize = 10;

// ---------------------------------------------------------------------------
// Calc logic
// ---------------------------------------------------------------------------

/// Evaluate expression string. Returns Result<f64, String>.
pub fn eval_expr(expr: &str) -> Result<f64, String> {
    let clean = expr.replace('x', "*").replace(',', ".");
    meval::eval_str(&clean).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Grid layout: 5 rows x 4 cols
// ---------------------------------------------------------------------------

// Each entry: (label, key kind)
const GRID: &[&[(&str, &str)]] = &[
    &[("C", "clear"), ("+/-", "special"), ("%", "op"), ("/", "op")],
    &[("7", "digit"), ("8", "digit"),     ("9", "digit"), ("*", "op")],
    &[("4", "digit"), ("5", "digit"),     ("6", "digit"), ("-", "op")],
    &[("1", "digit"), ("2", "digit"),     ("3", "digit"), ("+", "op")],
    &[("0", "digit"), (".", "digit"),     ("=", "equals"), ("", "noop")],
];

// ---------------------------------------------------------------------------
// Message
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Message {
    ButtonPressed(String),
    KeyboardEvent(keyboard::Event),
    CopyResult,
    ShowAbout,
    Quit,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

pub struct App {
    pub display: String,
    pub expression: String,
    pub history: Vec<String>,
    pub error: bool,
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        (
            Self {
                display: "0".into(),
                expression: String::new(),
                history: Vec::new(),
                error: false,
            },
            Task::none(),
        )
    }

    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::ButtonPressed(key) => {
                self.handle_key(&key);
                Task::none()
            }
            Message::KeyboardEvent(ev) => {
                if let keyboard::Event::KeyPressed { key, .. } = ev {
                    let s = match &key {
                        Key::Character(c) => match c.as_str() {
                            k @ ("0"|"1"|"2"|"3"|"4"|"5"|"6"|"7"|"8"|"9"|"+"|"-"|"*"|"/"|".") => k.to_string(),
                            "=" | "\r" => "=".to_string(),
                            "c" | "C"  => "C".to_string(),
                            _ => return Task::none(),
                        },
                        Key::Named(iced::keyboard::key::Named::Enter) => "=".to_string(),
                        Key::Named(iced::keyboard::key::Named::Backspace) => "backspace".to_string(),
                        Key::Named(iced::keyboard::key::Named::Escape) => "C".to_string(),
                        _ => return Task::none(),
                    };
                    self.handle_key(&s);
                }
                Task::none()
            }
            Message::CopyResult => {
                // Clipboard access would need arboard; stub log only.
                eprintln!("[calc] copy result: {}", self.display);
                Task::none()
            }
            Message::ShowAbout => { eprintln!("lumo-calc 0.1.0"); Task::none() }
            Message::Quit => std::process::exit(0),
        }
    }

    fn handle_key(&mut self, key: &str) {
        match key {
            "C" => {
                self.display = "0".into();
                self.expression = String::new();
                self.error = false;
            }
            "+/-" => {
                if self.expression.starts_with('-') {
                    self.expression = self.expression[1..].to_string();
                } else if !self.expression.is_empty() && self.expression != "0" {
                    self.expression = format!("-{}", self.expression);
                }
                self.display = self.expression.clone();
            }
            "%" => {
                if let Ok(v) = eval_expr(&self.expression) {
                    let result = format!("{}", v / 100.0);
                    self.expression = result.clone();
                    self.display = result;
                }
            }
            "=" => {
                if !self.expression.is_empty() {
                    match eval_expr(&self.expression) {
                        Ok(v) => {
                            let result = if v.fract() == 0.0 && v.abs() < 1e15 {
                                format!("{}", v as i64)
                            } else {
                                format!("{:.10}", v).trim_end_matches('0').trim_end_matches('.').to_string()
                            };
                            let entry = format!("{} = {}", self.expression, result);
                            self.push_history(entry);
                            self.display = result.clone();
                            self.expression = result;
                            self.error = false;
                        }
                        Err(e) => {
                            self.display = format!("Erro: {}", e);
                            self.error = true;
                        }
                    }
                }
            }
            "backspace" => {
                if !self.expression.is_empty() {
                    self.expression.pop();
                    self.display = if self.expression.is_empty() {
                        "0".into()
                    } else {
                        self.expression.clone()
                    };
                }
            }
            "" | "noop" => {}
            digit_or_op => {
                if self.error {
                    self.expression = String::new();
                    self.error = false;
                }
                self.expression.push_str(digit_or_op);
                self.display = self.expression.clone();
            }
        }
    }

    fn push_history(&mut self, entry: String) {
        if self.history.len() >= HISTORY_MAX {
            self.history.remove(0);
        }
        self.history.push(entry);
    }

    pub fn view(&self) -> Element<Message> {
        let display_color = if self.error { LumoTheme::danger() } else { LumoTheme::fg() };

        let display = container(
            text(self.display.clone())
                .size(28)
                .color(display_color)
        )
        .style(|_| container_display())
        .width(Length::Fill)
        .padding([12, 16])
        .align_x(iced::alignment::Horizontal::Right);

        // Button grid
        let mut grid_rows: Vec<Element<Message>> = Vec::new();
        for row_def in GRID {
            let mut btns: Vec<Element<Message>> = Vec::new();
            for &(label, kind) in *row_def {
                if label.is_empty() {
                    btns.push(Space::with_width(Length::Fill).into());
                    continue;
                }
                let btn_kind = match kind {
                    "digit"   => ButtonKind::Digit,
                    "op"      => ButtonKind::Op,
                    "equals"  => ButtonKind::Equals,
                    "clear"   => ButtonKind::Clear,
                    _         => ButtonKind::Special,
                };
                let lbl = label.to_string();
                let msg = Message::ButtonPressed(lbl.clone());
                btns.push(
                    button(
                        text(lbl).size(18).color(match kind {
                            "equals"  => LumoTheme::bg(),
                            "clear"   => LumoTheme::danger(),
                            "op"      => LumoTheme::accent(),
                            "special" => LumoTheme::muted(),
                            _         => LumoTheme::fg(),
                        })
                    )
                    .on_press(msg)
                    .style(move |_, _| btn_kind.style())
                    .width(Length::Fill)
                    .height(Length::Fixed(60.0))
                    .into()
                );
            }
            grid_rows.push(row(btns).spacing(8).into());
        }

        let buttons_col = column(grid_rows).spacing(8);

        // History pane
        let hist_items: Vec<Element<Message>> = self.history.iter().rev().map(|entry| {
            text(entry.clone()).size(11).color(LumoTheme::muted()).into()
        }).collect();

        let history = container(
            column![
                text("Historico").size(12).color(LumoTheme::muted()),
                Space::with_height(6),
                scrollable(column(hist_items).spacing(4)),
            ]
        )
        .style(|_| container_history())
        .width(Length::Fixed(180.0))
        .height(Length::Fill)
        .padding(10);

        let main_col = column![
            display,
            Space::with_height(12),
            buttons_col,
        ]
        .spacing(0)
        .width(Length::Fill);

        container(
            row![
                main_col,
                Space::with_width(12),
                history,
            ]
            .align_y(Alignment::Start)
        )
        .style(|_| container_bg())
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(16)
        .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        use iced::event::listen_with;
        let kbd = listen_with(|ev, _, _| {
            if let iced::Event::Keyboard(k) = ev { Some(Message::KeyboardEvent(k)) } else { None }
        });
        Subscription::batch([appmenu_subscription(), kbd])
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_addition() {
        let r = eval_expr("3+4").unwrap();
        assert!((r - 7.0).abs() < 1e-9);
    }

    #[test]
    fn test_eval_multiplication() {
        let r = eval_expr("6*7").unwrap();
        assert!((r - 42.0).abs() < 1e-9);
    }

    #[test]
    fn test_eval_division() {
        let r = eval_expr("10/4").unwrap();
        assert!((r - 2.5).abs() < 1e-9);
    }

    #[test]
    fn test_eval_complex() {
        let r = eval_expr("(2+3)*4-1").unwrap();
        assert!((r - 19.0).abs() < 1e-9);
    }

    #[test]
    fn test_eval_invalid() {
        assert!(eval_expr("abc").is_err());
    }

    #[test]
    fn test_history_max() {
        let (mut app, _) = App::new();
        for i in 0..15 {
            app.push_history(format!("entry {}", i));
        }
        assert_eq!(app.history.len(), HISTORY_MAX);
    }

    #[test]
    fn test_clear_resets() {
        let (mut app, _) = App::new();
        app.handle_key("5");
        app.handle_key("+");
        app.handle_key("3");
        app.handle_key("C");
        assert_eq!(app.display, "0");
        assert!(app.expression.is_empty());
    }

    #[test]
    fn test_equals_eval() {
        let (mut app, _) = App::new();
        app.handle_key("8");
        app.handle_key("+");
        app.handle_key("4");
        app.handle_key("=");
        assert_eq!(app.display, "12");
    }
}
