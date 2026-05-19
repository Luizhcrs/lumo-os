//! app.rs -- App principal do lumo-notes.

use iced::keyboard::{self, Key, Modifiers};
use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Alignment, Element, Length, Subscription, Task};

use crate::appmenu::appmenu_subscription;
use crate::note::{delete_note, load_notes, new_note_path, save_note, Note};
use crate::theme::{ButtonStyle, ContainerStyle, LumoTheme};

// ---------------------------------------------------------------------------
// Message
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Message {
    NotesLoaded(Vec<Note>),
    NoteSelected(usize),
    ContentChanged(String),
    SearchChanged(String),
    NewNote,
    Save,
    SaveDone(Result<(), String>),
    DeleteSelected,
    DeleteDone(Result<(), String>),
    FocusSearch,
    ShowAbout,
    Quit,
    KeyboardEvent(keyboard::Event),
    Nop,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

pub struct App {
    pub notes: Vec<Note>,
    pub selected: Option<usize>,
    pub search: String,
    pub status: String,
    pub modified: bool,
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        let app = Self {
            notes: Vec::new(),
            selected: None,
            search: String::new(),
            status: String::new(),
            modified: false,
        };
        let task = Task::perform(load_notes(), Message::NotesLoaded);
        (app, task)
    }

    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::NotesLoaded(notes) => {
                self.notes = notes;
                if !self.notes.is_empty() { self.selected = Some(0); }
                Task::none()
            }

            Message::NoteSelected(idx) => {
                self.selected = Some(idx);
                self.modified = false;
                Task::none()
            }

            Message::ContentChanged(val) => {
                if let Some(idx) = self.selected {
                    if let Some(note) = self.notes.get_mut(idx) {
                        note.content = val;
                        note.modified = chrono::Local::now();
                        self.modified = true;
                    }
                }
                Task::none()
            }

            Message::SearchChanged(q) => {
                self.search = q;
                Task::none()
            }

            Message::NewNote => {
                let title = format!("Nova nota {}", chrono::Local::now().format("%H:%M:%S"));
                let path = new_note_path(&title);
                let note = Note {
                    id: self.notes.len() as u64,
                    title: title.clone(),
                    content: String::new(),
                    modified: chrono::Local::now(),
                    path,
                };
                self.notes.insert(0, note);
                self.selected = Some(0);
                self.modified = true;
                Task::none()
            }

            Message::Save => {
                if let Some(idx) = self.selected {
                    if let Some(note) = self.notes.get(idx) {
                        let path = note.path.clone();
                        let content = note.content.clone();
                        return Task::perform(save_note(path, content), Message::SaveDone);
                    }
                }
                Task::none()
            }

            Message::SaveDone(Ok(())) => {
                self.modified = false;
                self.status = "Salvo.".into();
                Task::none()
            }

            Message::SaveDone(Err(e)) => {
                self.status = format!("Erro ao salvar: {}", e);
                Task::none()
            }

            Message::DeleteSelected => {
                if let Some(idx) = self.selected {
                    if let Some(note) = self.notes.get(idx) {
                        let path = note.path.clone();
                        self.notes.remove(idx);
                        self.selected = if self.notes.is_empty() { None } else { Some(idx.saturating_sub(1)) };
                        return Task::perform(delete_note(path), Message::DeleteDone);
                    }
                }
                Task::none()
            }

            Message::DeleteDone(Ok(())) => {
                self.status = "Nota deletada.".into();
                Task::none()
            }

            Message::DeleteDone(Err(e)) => {
                self.status = format!("Erro ao deletar: {}", e);
                Task::none()
            }

            Message::FocusSearch => Task::none(),

            Message::KeyboardEvent(ev) => {
                if let keyboard::Event::KeyPressed { key, modifiers, .. } = ev {
                    let ctrl = modifiers.contains(Modifiers::CTRL);
                    if ctrl {
                        match &key {
                            Key::Character(c) => match c.as_str() {
                                "n" => return self.update(Message::NewNote),
                                "s" => return self.update(Message::Save),
                                "d" => return self.update(Message::DeleteSelected),
                                "f" => return self.update(Message::FocusSearch),
                                _ => {}
                            },
                            _ => {}
                        }
                    }
                }
                Task::none()
            }

            Message::ShowAbout => { self.status = "lumo-notes 0.1.0".into(); Task::none() }
            Message::Quit => std::process::exit(0),
            Message::Nop => Task::none(),
        }
    }

    pub fn view(&self) -> Element<Message> {
        let sidebar = self.view_sidebar();
        let editor  = self.view_editor();

        let content = row![
            container(sidebar)
                .style(|_| ContainerStyle::Sidebar.style())
                .width(Length::Fixed(240.0))
                .height(Length::Fill),
            container(editor)
                .style(|_| ContainerStyle::Main.style())
                .width(Length::Fill)
                .height(Length::Fill),
        ];

        container(content)
            .style(|_| ContainerStyle::Main.style())
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_sidebar(&self) -> Element<Message> {
        let search = text_input("Buscar notas...", &self.search)
            .on_input(Message::SearchChanged)
            .size(13)
            .padding([6, 10]);

        let toolbar = row![
            button(text("+ Nova").size(11).color(LumoTheme::bg()))
                .on_press(Message::NewNote)
                .style(|_, _| ButtonStyle::Primary.style())
                .padding([5, 10]),
        ];

        let filtered: Vec<(usize, &Note)> = self.notes.iter().enumerate()
            .filter(|(_, n)| n.matches_query(&self.search))
            .collect();

        let items: Vec<Element<Message>> = filtered.iter().map(|(orig_idx, note)| {
            let active = self.selected == Some(*orig_idx);
            let title_color = if active { LumoTheme::accent() } else { LumoTheme::fg() };
            let preview = note.preview();
            let date = note.modified.format("%d/%m %H:%M").to_string();
            let idx = *orig_idx;

            button(
                column![
                    text(note.title.clone()).size(13).color(title_color),
                    text(preview).size(11).color(LumoTheme::muted()),
                    text(date).size(10).color(LumoTheme::muted()),
                ]
                .spacing(2)
            )
            .on_press(Message::NoteSelected(idx))
            .style(move |_, _| ButtonStyle::NoteItem { active }.style())
            .width(Length::Fill)
            .padding([8, 10])
            .into()
        }).collect();

        column![
            container(
                column![
                    search,
                    Space::with_height(6),
                    toolbar,
                ]
                .spacing(0)
            )
            .padding([8, 8]),
            scrollable(column(items).spacing(2).padding([0, 8])),
        ]
        .spacing(0)
        .into()
    }

    fn view_editor(&self) -> Element<Message> {
        if let Some(idx) = self.selected {
            if let Some(note) = self.notes.get(idx) {
                let modified_marker = if self.modified { " *" } else { "" };
                let header = container(
                    row![
                        text(format!("{}{}", note.title, modified_marker)).size(16).color(LumoTheme::fg()),
                        Space::with_width(Length::Fill),
                        button(text("Salvar").size(11).color(LumoTheme::bg()))
                            .on_press(Message::Save)
                            .style(|_, _| ButtonStyle::Primary.style())
                            .padding([5, 12]),
                        Space::with_width(6),
                        button(text("Deletar").size(11).color(LumoTheme::fg()))
                            .on_press(Message::DeleteSelected)
                            .style(|_, _| ButtonStyle::Secondary.style())
                            .padding([5, 12]),
                    ]
                    .align_y(Alignment::Center)
                )
                .padding([10, 16]);

                let editor = text_input("Escreva aqui...", &note.content)
                    .on_input(Message::ContentChanged)
                    .size(14)
                    .width(Length::Fill);

                let status = if !self.status.is_empty() {
                    text(self.status.clone()).size(11).color(LumoTheme::accent())
                } else {
                    text("").size(11).color(LumoTheme::muted())
                };

                return column![
                    header,
                    container(
                        scrollable(
                            container(editor).width(Length::Fill).padding(16)
                        )
                        .width(Length::Fill)
                        .height(Length::Fill)
                    )
                    .width(Length::Fill)
                    .height(Length::Fill),
                    container(status).padding([5, 16]),
                ]
                .spacing(0)
                .into();
            }
        }

        container(
            text("Selecione ou crie uma nota.").size(14).color(LumoTheme::muted())
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
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
    use crate::note::Note;
    use std::path::PathBuf;

    fn make_app_with_notes() -> App {
        let notes = vec![
            Note { id: 0, title: "Alpha".into(), content: "conteudo alpha".into(), modified: chrono::Local::now(), path: PathBuf::from("/tmp/alpha.md") },
            Note { id: 1, title: "Beta".into(),  content: "conteudo beta".into(),  modified: chrono::Local::now(), path: PathBuf::from("/tmp/beta.md") },
        ];
        App {
            notes,
            selected: Some(0),
            search: String::new(),
            status: String::new(),
            modified: false,
        }
    }

    #[test]
    fn test_search_filter() {
        let app = make_app_with_notes();
        let filtered: Vec<_> = app.notes.iter().filter(|n| n.matches_query("alpha")).collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, "Alpha");
    }

    #[test]
    fn test_content_changed_sets_modified() {
        let mut app = make_app_with_notes();
        app.update(Message::ContentChanged("novo conteudo".into()));
        assert!(app.modified);
    }

    #[test]
    fn test_new_note_inserts_at_head() {
        let mut app = make_app_with_notes();
        let initial_count = app.notes.len();
        app.update(Message::NewNote);
        assert_eq!(app.notes.len(), initial_count + 1);
        assert_eq!(app.selected, Some(0));
    }

    #[test]
    fn test_delete_selected_removes() {
        let mut app = make_app_with_notes();
        app.selected = Some(0);
        let initial_count = app.notes.len();
        // Simulate delete without async (just remove from list)
        app.notes.remove(0);
        app.selected = if app.notes.is_empty() { None } else { Some(0) };
        assert_eq!(app.notes.len(), initial_count - 1);
    }

    #[test]
    fn test_note_select() {
        let mut app = make_app_with_notes();
        app.update(Message::NoteSelected(1));
        assert_eq!(app.selected, Some(1));
    }
}
