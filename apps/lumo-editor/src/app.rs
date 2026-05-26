//! app.rs -- App principal do lumo-text.
//!
//! Editor de texto single-buffer. Open/Save, Undo/Redo, Find/Replace.

use iced::keyboard::{self, Key, Modifiers};
use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Alignment, Element, Length, Subscription, Task};

use crate::appmenu::appmenu_subscription;
use crate::theme::{ButtonStyle, ContainerStyle, LumoTheme};

const MAX_UNDO: usize = 200;

// ---------------------------------------------------------------------------
// Message
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Message {
    ContentChanged(String),
    Save,
    OpenFileDialog,
    FileOpened(Result<(String, String), String>),
    FileSaved(Result<(), String>),
    Undo,
    Redo,
    ToggleFind,
    ToggleReplace,
    FindQueryChanged(String),
    ReplaceQueryChanged(String),
    FindNext,
    DoReplace,
    KeyboardEvent(keyboard::Event),
    ShowAbout,
    Quit,
    Nop,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

pub struct App {
    pub path: Option<String>,
    pub content: String,
    pub modified: bool,

    pub undo_stack: Vec<String>,
    pub redo_stack: Vec<String>,

    pub find_open: bool,
    pub replace_open: bool,
    pub find_query: String,
    pub replace_query: String,
    pub find_result: String,

    pub status: String,
}

impl App {
    pub fn new(initial_path: Option<String>) -> (Self, Task<Message>) {
        let app = Self {
            path: initial_path.clone(),
            content: String::new(),
            modified: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            find_open: false,
            replace_open: false,
            find_query: String::new(),
            replace_query: String::new(),
            find_result: String::new(),
            status: String::new(),
        };
        let task = if let Some(p) = initial_path {
            Task::perform(load_file(p), Message::FileOpened)
        } else {
            Task::none()
        };
        (app, task)
    }

    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::ContentChanged(new_val) => {
                if self.undo_stack.len() >= MAX_UNDO {
                    self.undo_stack.remove(0);
                }
                self.undo_stack.push(self.content.clone());
                self.redo_stack.clear();
                self.content = new_val;
                self.modified = true;
                Task::none()
            }

            Message::Save => {
                if let Some(ref p) = self.path.clone() {
                    let content = self.content.clone();
                    let path = p.clone();
                    Task::perform(save_file(path, content), Message::FileSaved)
                } else {
                    self.status = "Sem arquivo — use File > Abrir primeiro.".into();
                    Task::none()
                }
            }

            Message::OpenFileDialog => {
                self.status = "Abrir: passe o path como argumento CLI.".into();
                Task::none()
            }

            Message::FileOpened(Ok((path, content))) => {
                self.path = Some(path.clone());
                self.content = content;
                self.modified = false;
                self.undo_stack.clear();
                self.redo_stack.clear();
                self.status = format!("Aberto: {}", path);
                Task::none()
            }

            Message::FileOpened(Err(e)) => {
                self.status = format!("Erro ao abrir: {}", e);
                Task::none()
            }

            Message::FileSaved(Ok(())) => {
                self.modified = false;
                self.status = "Salvo.".into();
                Task::none()
            }

            Message::FileSaved(Err(e)) => {
                self.status = format!("Erro ao salvar: {}", e);
                Task::none()
            }

            Message::Undo => {
                if let Some(prev) = self.undo_stack.pop() {
                    self.redo_stack.push(self.content.clone());
                    self.content = prev;
                    self.modified = true;
                }
                Task::none()
            }

            Message::Redo => {
                if let Some(next) = self.redo_stack.pop() {
                    self.undo_stack.push(self.content.clone());
                    self.content = next;
                    self.modified = true;
                }
                Task::none()
            }

            Message::ToggleFind => {
                self.find_open = !self.find_open;
                if !self.find_open {
                    self.replace_open = false;
                }
                Task::none()
            }

            Message::ToggleReplace => {
                self.replace_open = !self.replace_open;
                if self.replace_open {
                    self.find_open = true;
                }
                Task::none()
            }

            Message::FindQueryChanged(q) => {
                self.find_query = q;
                Task::none()
            }
            Message::ReplaceQueryChanged(q) => {
                self.replace_query = q;
                Task::none()
            }

            Message::FindNext => {
                if self.find_query.is_empty() {
                    self.find_result = String::new();
                } else if self.content.contains(&self.find_query) {
                    let count = self.content.matches(&self.find_query).count();
                    self.find_result = format!("{} ocorrencia(s)", count);
                } else {
                    self.find_result = "Nao encontrado.".into();
                }
                Task::none()
            }

            Message::DoReplace => {
                if !self.find_query.is_empty() {
                    let count = self.content.matches(&self.find_query).count();
                    self.content = self.content.replace(&self.find_query, &self.replace_query);
                    self.modified = true;
                    self.find_result = format!("{} substituicao(es)", count);
                }
                Task::none()
            }

            Message::KeyboardEvent(ev) => {
                if let keyboard::Event::KeyPressed { key, modifiers, .. } = ev {
                    let ctrl = modifiers.contains(Modifiers::CTRL);
                    match &key {
                        Key::Character(c) if ctrl => match c.as_str() {
                            "s" => return self.update(Message::Save),
                            "z" => return self.update(Message::Undo),
                            "y" => return self.update(Message::Redo),
                            "f" => return self.update(Message::ToggleFind),
                            "h" => return self.update(Message::ToggleReplace),
                            _ => {}
                        },
                        _ => {}
                    }
                }
                Task::none()
            }

            Message::ShowAbout => {
                self.status = "lumo-text 0.1.0".into();
                Task::none()
            }
            Message::Quit => std::process::exit(0),
            Message::Nop => Task::none(),
        }
    }

    pub fn view(&self) -> Element<Message> {
        let title_bar = self.view_titlebar();
        let find_bar = if self.find_open {
            Some(self.view_findbar())
        } else {
            None
        };
        let editor = self.view_editor();
        let status_bar = self.view_statusbar();

        let mut col = column![title_bar];
        if let Some(fb) = find_bar {
            col = col.push(fb);
        }
        col = col.push(editor).push(status_bar);

        container(col)
            .style(|_| ContainerStyle::Bg.style())
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_titlebar(&self) -> Element<Message> {
        let modified_marker = if self.modified { " *" } else { "" };
        let fname = self
            .path
            .as_deref()
            .and_then(|p| std::path::Path::new(p).file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("sem titulo");
        let title = text(format!("{}{}", fname, modified_marker))
            .size(13)
            .color(LumoTheme::fg());

        let toolbar = row![
            button(text("Salvar").size(11).color(LumoTheme::bg()))
                .on_press(Message::Save)
                .style(|_, _| ButtonStyle::Primary.style())
                .padding([4, 10]),
            Space::with_width(6),
            button(text("Localizar").size(11).color(LumoTheme::fg()))
                .on_press(Message::ToggleFind)
                .style(|_, _| ButtonStyle::Toolbar.style())
                .padding([4, 10]),
        ]
        .align_y(Alignment::Center);

        container(
            row![title, Space::with_width(Length::Fill), toolbar,]
                .align_y(Alignment::Center)
                .padding([0, 12]),
        )
        .style(|_| ContainerStyle::Toolbar.style())
        .width(Length::Fill)
        .padding([8, 0])
        .into()
    }

    fn view_findbar(&self) -> Element<Message> {
        let find_input = text_input("Localizar...", &self.find_query)
            .on_input(Message::FindQueryChanged)
            .on_submit(Message::FindNext)
            .size(13)
            .width(Length::Fixed(200.0));

        let mut row_items: Vec<Element<Message>> = vec![
            text("Localizar:").size(12).color(LumoTheme::muted()).into(),
            Space::with_width(8).into(),
            find_input.into(),
            Space::with_width(6).into(),
            button(text("Prox").size(11).color(LumoTheme::bg()))
                .on_press(Message::FindNext)
                .style(|_, _| ButtonStyle::Primary.style())
                .padding([4, 8])
                .into(),
        ];

        if self.replace_open {
            let replace_input = text_input("Substituir...", &self.replace_query)
                .on_input(Message::ReplaceQueryChanged)
                .size(13)
                .width(Length::Fixed(200.0));
            row_items.push(Space::with_width(10).into());
            row_items.push(
                text("Substituir:")
                    .size(12)
                    .color(LumoTheme::muted())
                    .into(),
            );
            row_items.push(Space::with_width(8).into());
            row_items.push(replace_input.into());
            row_items.push(Space::with_width(6).into());
            row_items.push(
                button(text("Substituir tudo").size(11).color(LumoTheme::bg()))
                    .on_press(Message::DoReplace)
                    .style(|_, _| ButtonStyle::Primary.style())
                    .padding([4, 8])
                    .into(),
            );
        }

        if !self.find_result.is_empty() {
            row_items.push(Space::with_width(10).into());
            row_items.push(
                text(self.find_result.clone())
                    .size(11)
                    .color(LumoTheme::accent())
                    .into(),
            );
        }

        container(row(row_items).align_y(Alignment::Center).padding([0, 12]))
            .style(|_| ContainerStyle::FindBar.style())
            .width(Length::Fill)
            .padding([6, 0])
            .into()
    }

    fn view_editor(&self) -> Element<Message> {
        let area = text_input("", &self.content)
            .on_input(Message::ContentChanged)
            .size(14)
            .width(Length::Fill);

        container(
            scrollable(container(area).width(Length::Fill).padding(16))
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .style(|_| ContainerStyle::Bg.style())
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn view_statusbar(&self) -> Element<Message> {
        let lines = self.content.lines().count();
        let chars = self.content.len();
        let info = format!("{} linhas | {} chars", lines, chars);

        container(
            row![
                text(info).size(11).color(LumoTheme::muted()),
                Space::with_width(Length::Fill),
                text(self.status.clone())
                    .size(11)
                    .color(LumoTheme::accent()),
            ]
            .padding([0, 12]),
        )
        .style(|_| ContainerStyle::Toolbar.style())
        .width(Length::Fill)
        .padding([5, 0])
        .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        use iced::event::listen_with;
        let kbd = listen_with(|ev, _, _| {
            if let iced::Event::Keyboard(k) = ev {
                Some(Message::KeyboardEvent(k))
            } else {
                None
            }
        });
        Subscription::batch([appmenu_subscription(), kbd])
    }
}

// ---------------------------------------------------------------------------
// Async helpers
// ---------------------------------------------------------------------------

async fn load_file(path: String) -> Result<(String, String), String> {
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| e.to_string())?;
    Ok((path, content))
}

async fn save_file(path: String, content: String) -> Result<(), String> {
    tokio::fs::write(&path, content)
        .await
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_app() -> App {
        App {
            path: None,
            content: "hello world".into(),
            modified: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            find_open: false,
            replace_open: false,
            find_query: String::new(),
            replace_query: String::new(),
            find_result: String::new(),
            status: String::new(),
        }
    }

    #[test]
    fn test_undo_redo_cycle() {
        let mut app = make_app();
        app.update(Message::ContentChanged("hello world!".into()));
        assert_eq!(app.content, "hello world!");
        app.update(Message::Undo);
        assert_eq!(app.content, "hello world");
        app.update(Message::Redo);
        assert_eq!(app.content, "hello world!");
    }

    #[test]
    fn test_find_count() {
        let mut app = make_app();
        app.content = "aa bb aa cc aa".into();
        app.find_query = "aa".into();
        app.update(Message::FindNext);
        assert!(app.find_result.contains('3'));
    }

    #[test]
    fn test_replace_all() {
        let mut app = make_app();
        app.content = "foo foo foo".into();
        app.find_query = "foo".into();
        app.replace_query = "bar".into();
        app.update(Message::DoReplace);
        assert_eq!(app.content, "bar bar bar");
    }

    #[test]
    fn test_undo_max_stack() {
        let mut app = make_app();
        for i in 0..250 {
            app.update(Message::ContentChanged(format!("v{}", i)));
        }
        assert!(app.undo_stack.len() <= MAX_UNDO);
    }

    #[test]
    fn test_modified_flag() {
        let mut app = make_app();
        assert!(!app.modified);
        app.update(Message::ContentChanged("x".into()));
        assert!(app.modified);
    }
}
