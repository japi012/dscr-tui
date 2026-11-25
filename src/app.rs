use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::{
    event::{
        DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        MouseButton, MouseEvent, MouseEventKind,
    },
    execute,
};
use ratatui::layout::{Constraint, Layout};
use ratatui::palette::Hsl;
use ratatui::style::{Color, Styled, Stylize};
use ratatui::symbols::merge::MergeStrategy;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Position, Rect},
    style::Style,
};
use ratatui_textarea::{CursorMove, DataCursor, Input, Key, TextArea};

use crate::transmission::{CallSign, Signal};

use super::dictionary::Dictionary;
use super::transmission::Transmission;

#[derive(Debug, Clone)]
enum StatusBar {
    Nothing,
    Error(String),
}

#[derive(Debug, Clone, Copy)]
enum Selected {
    InputBox,
    ChatBox,
}

#[derive(Debug)]
pub struct App {
    pub running: bool,
    text_area: TextArea<'static>,
    latest_transmission_number: u16,
    transmissions: Vec<Transmission>,
    dictionary: Dictionary,
    status_bar: StatusBar,
    message_scroll: usize,
    message_scroll_max: usize,
    currently_selected: Selected,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        Self {
            running: true,
            text_area: TextArea::new(Vec::new()),
            latest_transmission_number: 0,
            transmissions: Vec::new(),
            dictionary: Dictionary::default(),
            status_bar: StatusBar::Nothing,
            message_scroll: 0,
            message_scroll_max: 0,
            currently_selected: Selected::InputBox,
        }
    }

    pub fn with_dictionary(dictionary: Dictionary) -> Self {
        Self {
            dictionary,
            ..Default::default()
        }
    }

    pub fn quit(&mut self) {
        self.running = false;
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> color_eyre::Result<()> {
        execute!(
            std::io::stderr(),
            EnableBracketedPaste,
            EnableMouseCapture,
            // PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )?;
        while self.running {
            let mut area = Rect::default();
            terminal.draw(|frame| {
                area = frame.area();
                self.draw(frame);
            })?;
            self.handle_events(area)?;
        }
        execute!(
            std::io::stderr(),
            DisableBracketedPaste,
            DisableMouseCapture,
            // PopKeyboardEnhancementFlags
        )?;
        Ok(())
    }

    fn push_transmission(
        &mut self,
        message: Box<[Signal]>,
        sender: CallSign,
    ) -> color_eyre::Result<()> {
        self.transmissions.push(Transmission::new(
            self.latest_transmission_number,
            sender,
            message,
        ));
        self.latest_transmission_number += 1;
        if self.latest_transmission_number >= 1000 {
            self.latest_transmission_number = 0;
        }
        Ok(())
    }

    fn handle_event_input_box(&mut self, area: Rect, event: Event) -> color_eyre::Result<()> {
        match event {
            Event::Mouse(mouse_event) => handle_mouse(&mut self.text_area, area, mouse_event),
            Event::Paste(pasted) => {
                self.text_area.set_yank_text(pasted);
                self.text_area.paste();
            }
            Event::Key(key_event) if key_event.is_press() || key_event.is_repeat() => {
                let input: Input = key_event.into();
                match input {
                    Input {
                        key: Key::Char('q'),
                        ctrl: true,
                        ..
                    } => self.quit(),
                    Input { key: Key::Esc, .. } => self.currently_selected = Selected::ChatBox,
                    Input {
                        key: Key::Enter,
                        shift: true,
                        ..
                    }
                    | Input {
                        key: Key::Enter,
                        ctrl: true,
                        ..
                    }
                    | Input {
                        key: Key::Char('j'),
                        ctrl: true,
                        ..
                    } => {
                        self.text_area.insert_newline();
                    }
                    Input {
                        key: Key::Enter, ..
                    } => {
                        let lines = self.text_area.lines().join("\n");
                        let message = self.dictionary.parse_signals(&lines);
                        match message {
                            Ok(message) => {
                                self.push_transmission(
                                    message.into_boxed_slice(),
                                    CallSign::new(0o1212),
                                )?;
                                self.text_area = TextArea::new(Vec::new());
                                self.status_bar = StatusBar::Nothing;
                                self.update_message_scroll_max();
                            }
                            Err(error) => {
                                self.status_bar = StatusBar::Error(format!(
                                    "Error while trying to send message: {error}"
                                ))
                            }
                        }
                    }
                    Input {
                        key: Key::Char(c),
                        shift: false,
                        ..
                    } => {
                        self.text_area.input(Input {
                            key: Key::Char(c.to_ascii_uppercase()),
                            ..input
                        });
                    }
                    input => {
                        self.text_area.input(input);
                    }
                };
            }
            _ => (),
        }
        Ok(())
    }

    fn handle_event_chat_box(&mut self, event: Event) -> color_eyre::Result<()> {
        match event {
            Event::Key(key) if key.is_press() || key.is_repeat() => match key.code {
                KeyCode::Up => {
                    self.message_scroll = self.message_scroll.saturating_sub(1);
                }
                KeyCode::Down => {
                    self.message_scroll = self.message_scroll_max.min(self.message_scroll + 1);
                }
                KeyCode::Char('i') => self.currently_selected = Selected::InputBox,
                KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => self.quit(),
                _ => (),
            },
            _ => (),
        }
        Ok(())
    }

    fn update_message_scroll_max(&mut self) {
        self.message_scroll_max = self
            .transmissions
            .iter()
            .map(|t| self.dictionary.format_signals(t.message()).lines().count() + 1)
            .sum();
    }

    fn handle_events(&mut self, area: Rect) -> color_eyre::Result<()> {
        if event::poll(Duration::from_millis(16))? {
            let event = event::read()?;
            match self.currently_selected {
                Selected::InputBox => self.handle_event_input_box(area, event)?,
                Selected::ChatBox => self.handle_event_chat_box(event)?,
            }
        }
        Ok(())
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();

        let input_box_height = { self.text_area.lines().len() + 2 }
            .try_into()
            .unwrap_or(area.height);

        let layout = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Max(1),
            Constraint::Max(input_box_height),
        ]);
        let [chat_area, status_bar_area, input_area] = area.layout(&layout);

        let chat_layout = Layout::horizontal([Constraint::Fill(1), Constraint::Length(1)]);
        let [scroll_bar_area, _] = chat_area.layout(&chat_layout);
        let message_area = chat_area;

        let messages: Vec<_> = self
            .transmissions
            .iter()
            .map(|t| self.dictionary.format_signals(t.message()))
            .collect();

        let message_layout = Layout::vertical(
            messages
                .iter()
                .map(|s| Constraint::Max(s.lines().count() as u16 + 2)),
        )
        .spacing(-1);

        let messages_area = Rect {
            height: self.message_scroll_max as u16 + 1,
            ..message_area
        };
        for ((rect, transmission), message) in messages_area
            .layout_vec(&message_layout)
            .into_iter()
            .zip(self.transmissions.iter())
            .zip(messages.iter())
        {
            let y = rect.y.checked_sub(self.message_scroll as u16);

            let scroll_amount = if y.is_none() {
                self.message_scroll as u16 - rect.y
            } else {
                0
            };

            let rect = Rect {
                y: y.unwrap_or_default(),
                height: rect.height.saturating_sub(scroll_amount),
                ..rect
            };

            let layout = Layout::horizontal([
                Constraint::Length(1),
                Constraint::Max(4),
                Constraint::Max(5),
                Constraint::Fill(1),
            ]);
            let [_, id_area, sender_area, content_area] = rect.layout(&layout);

            let message_block = Block::bordered()
                .border_type(BorderType::LightDoubleDashed)
                .border_style(Style::new().dark_gray())
                .merge_borders(MergeStrategy::Fuzzy);

            let message = Paragraph::new(message.as_str())
                .block(message_block.clone())
                .white()
                .scroll((scroll_amount, 0));

            frame.render_widget(message, content_area);
            frame.render_widget(message_block, rect);

            if rect.height > 2 {
                let id = Line::raw(format!("{:03}", transmission.id())).dark_gray();
                frame.render_widget(
                    id,
                    Rect {
                        y: id_area.y + 1,
                        ..id_area
                    },
                );

                let sender = Line::raw(transmission.sender().to_string()).set_style(
                    Style::new().fg(Color::from_hsl(Hsl::from_components((
                        transmission.sender().hue() as f32,
                        1.0,
                        0.7,
                    )))),
                );
                frame.render_widget(
                    sender,
                    Rect {
                        y: sender_area.y + 1,
                        ..sender_area
                    },
                );
            }
        }

        let scroll_bar =
            Scrollbar::new(ScrollbarOrientation::VerticalRight).style(Style::new().white());
        let mut scroll_bar_state = ScrollbarState::new(
            self.message_scroll_max
                .saturating_sub(message_area.height as usize),
        )
        .position(self.message_scroll);

        frame.render_stateful_widget(scroll_bar, scroll_bar_area, &mut scroll_bar_state);

        let clear = Clear::default();
        frame.render_widget(clear, status_bar_area.union(input_area));

        let chat_block = Block::bordered()
            .border_type(BorderType::Plain)
            .border_style(Style::new().white())
            .merge_borders(MergeStrategy::Replace);
        frame.render_widget(chat_block, chat_area);

        let input_block = Block::bordered()
            .border_type(BorderType::Plain)
            .border_style(Style::new().white())
            .merge_borders(MergeStrategy::Fuzzy);

        self.text_area.set_block(input_block);
        self.text_area.set_style(Style::new().white());
        self.text_area.set_cursor_line_style(Style::new());

        if let Selected::InputBox = self.currently_selected {
            self.text_area.set_cursor_style(Style::new().on_white());
        } else {
            self.text_area.set_cursor_style(Style::new());
        }

        frame.render_widget(&self.text_area, input_area);

        let status_bar_content = match &self.status_bar {
            StatusBar::Nothing => Span::default(),
            StatusBar::Error(err) => err.as_str().red(),
        };

        let status_bar = Paragraph::new(status_bar_content);
        frame.render_widget(status_bar, status_bar_area);
    }
}

fn clicked_cursor(textarea: &TextArea<'_>, area: Rect, event: &MouseEvent) -> Option<CursorMove> {
    let inner = textarea.block().map_or(area, |block| block.inner(area));
    if !inner.contains(Position::new(event.column, event.row)) {
        return None;
    }

    let (top_row, top_col) = textarea.scroll_offset();
    let row = event.row - inner.y + top_row;
    let col = (event.column - inner.x).saturating_sub(textarea.line_number_width()) + top_col;

    let DataCursor(row, col) = textarea.screen_to_data(usize::from(row), usize::from(col));
    Some(CursorMove::Jump(row as u16, col as u16))
}

fn handle_mouse(textarea: &mut TextArea<'_>, area: Rect, event: MouseEvent) {
    match event.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if let Some(cursor) = clicked_cursor(textarea, area, &event) {
                textarea.cancel_selection();
                textarea.move_cursor(cursor);
                textarea.start_selection();
            }
        }
        MouseEventKind::Drag(MouseButton::Left)
            if let Some(cursor) = clicked_cursor(textarea, area, &event) =>
        {
            textarea.move_cursor(cursor);
        }
        MouseEventKind::Drag(MouseButton::Left) => {}
        MouseEventKind::Up(MouseButton::Left) => {
            if textarea
                .selection_range()
                .is_some_and(|(from, to)| from == to)
            {
                textarea.cancel_selection();
            }
        }
        _ => {
            textarea.input(event);
        }
    }
}
