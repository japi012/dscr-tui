use color_eyre::eyre::bail;
use crossterm::event::{
    Event as CrosstermEvent, KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::{Constraint, Layout};
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

use crate::event::SentEvent::SelectCallSign;

use super::dictionary::Dictionary;
use super::event::{EventHandler, RecievedEvent, SentEvent};
use super::message::DSCRMessage;
use super::transmission::{CallSign, Transmission};

#[derive(Debug, Clone)]
enum StatusBar {
    Nothing,
    Ok(String),
    Error(String),
}

#[derive(Debug, Clone, Copy)]
enum Selected {
    InputBox,
    ChatBox,
    CallSignBox,
}

#[derive(Debug)]
pub struct App {
    _url: String,
    running: bool,
    text_area: TextArea<'static>,
    input_box_area: Rect,
    transmissions: Vec<Transmission>,
    dictionary: Dictionary,
    status_bar: StatusBar,
    message_scroll: usize,
    message_scroll_max: usize,
    currently_selected: Selected,
    call_sign: CallSign,
    call_sign_usable: bool,
    call_sign_text_area: TextArea<'static>,
    call_sign_box_area: Rect,
    active_call_signs: Vec<CallSign>,
    event_handler: EventHandler,
}

impl App {
    pub fn new(
        _url: String,
        call_sign: CallSign,
        dictionary: Dictionary,
        event_handler: EventHandler,
    ) -> Self {
        Self {
            _url,
            running: true,
            text_area: TextArea::new(Vec::new()),
            transmissions: Vec::new(),
            dictionary,
            status_bar: StatusBar::Nothing,
            message_scroll: 0,
            message_scroll_max: 0,
            currently_selected: Selected::InputBox,
            input_box_area: Rect::default(),
            call_sign,
            call_sign_usable: false,
            call_sign_text_area: TextArea::new(vec![call_sign.to_string()]),
            call_sign_box_area: Rect::default(),
            active_call_signs: Vec::new(),
            event_handler,
        }
    }

    pub fn quit(&mut self) {
        self.running = false;
    }

    pub async fn start(&mut self) -> color_eyre::Result<()> {
        self.event_handler
            .send(SentEvent::SelectCallSign(self.call_sign, false))?;

        Ok(())
    }

    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> color_eyre::Result<()> {
        self.start().await?;

        loop {
            self.render(terminal)?;

            let event = self.event_handler.next().await?;
            self.handle_event(event)?;

            if !self.running {
                break;
            }
        }
        Ok(())
    }

    pub fn render(&mut self, terminal: &mut DefaultTerminal) -> color_eyre::Result<()> {
        self.update_message_scroll_max();
        terminal.draw(|frame| {
            self.draw(frame);
        })?;
        Ok(())
    }

    pub fn handle_event(&mut self, event: RecievedEvent) -> color_eyre::Result<()> {
        match event {
            RecievedEvent::CrosstermEvent(event) => self.handle_crossterm_event(event),
            RecievedEvent::DSCREvent(event) => self.handle_message(event),
            RecievedEvent::SocketClosed => {
                todo!("socket closed");
            }
            RecievedEvent::Error => bail!("error"),
        }
    }

    pub fn handle_message(&mut self, message: DSCRMessage) -> color_eyre::Result<()> {
        match message {
            DSCRMessage::CallSignInUse => {
                let CallSign::Valid(call_sign) = self.call_sign else {
                    unreachable!();
                };
                self.call_sign_usable = false;
                self.status_bar =
                    StatusBar::Error(format!("Call sign {} is already in use", self.call_sign));
                self.call_sign = CallSign::Valid(call_sign + 1);
                self.event_handler
                    .send(SelectCallSign(self.call_sign, false))?;
            }
            DSCRMessage::CallSignOk(call_sign) => {
                self.status_bar = StatusBar::Ok(format!("Logged in as {call_sign}"));
                self.call_sign_usable = true;
                self.call_sign = call_sign;
                self.call_sign_text_area = TextArea::new(vec![call_sign.to_string()]);
            }
            DSCRMessage::ReconnectionOk => todo!(),
            DSCRMessage::ReceivedMessage(transmission) => self.push_transmission(transmission)?,
            DSCRMessage::ActiveCallSigns(call_signs) => self.active_call_signs = call_signs,
        }
        Ok(())
    }

    fn push_transmission(&mut self, transmission: Transmission) -> color_eyre::Result<()> {
        self.transmissions.push(transmission);
        Ok(())
    }

    fn handle_event_input_box(&mut self, event: CrosstermEvent) -> color_eyre::Result<()> {
        match event {
            CrosstermEvent::Mouse(mouse_event) => {
                handle_mouse(&mut self.text_area, self.input_box_area, mouse_event)
            }
            CrosstermEvent::Paste(pasted) => {
                self.text_area.set_yank_text(pasted);
                self.text_area.paste();
            }
            CrosstermEvent::Key(key_event) if key_event.is_press() || key_event.is_repeat() => {
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
                                self.event_handler.send(SentEvent::SendMessage(message))?;
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

    fn handle_event_call_sign_box(&mut self, event: CrosstermEvent) -> color_eyre::Result<()> {
        match event {
            CrosstermEvent::Mouse(mouse_event) => handle_mouse(
                &mut self.call_sign_text_area,
                self.call_sign_box_area,
                mouse_event,
            ),
            CrosstermEvent::Paste(pasted) => {
                let [line] = self.call_sign_text_area.lines() else {
                    unreachable!()
                };

                let can_paste_amount = 4usize.saturating_sub(line.len());

                let pasted = pasted
                    .chars()
                    .filter(|&c| c.is_ascii_digit() && c != '8' && c != '9')
                    .collect::<String>();

                self.call_sign_text_area
                    .set_yank_text(&pasted[0..can_paste_amount.min(pasted.len())]);
                self.call_sign_text_area.paste();
            }
            CrosstermEvent::Key(key_event) if key_event.is_press() || key_event.is_repeat() => {
                let input: Input = key_event.into();
                match input {
                    Input {
                        key: Key::Char('q'),
                        ctrl: true,
                        ..
                    } => self.quit(),
                    Input { key: Key::Esc, .. } => self.currently_selected = Selected::ChatBox,
                    Input {
                        key: Key::Enter, ..
                    } => {
                        let [line] = self.call_sign_text_area.lines() else {
                            unreachable!()
                        };
                        let call_sign = u16::from_str_radix(line, 8)?;
                        self.event_handler
                            .send(SentEvent::SelectCallSign(CallSign::new(call_sign), false))?;
                        self.currently_selected = Selected::InputBox;
                    }
                    Input {
                        key: Key::Char(c), ..
                    } if c.is_ascii_digit() && c != '8' && c != '9' => {
                        let [line] = self.call_sign_text_area.lines() else {
                            unreachable!()
                        };
                        if line.len() < 4 {
                            self.call_sign_text_area.input(input);
                        }
                    }
                    Input {
                        key: Key::Char(_), ..
                    } => (),
                    input => {
                        self.call_sign_text_area.input(input);
                    }
                };
            }
            _ => (),
        }
        Ok(())
    }

    fn handle_event_chat_box(&mut self, event: CrosstermEvent) -> color_eyre::Result<()> {
        match event {
            CrosstermEvent::Key(key) if key.is_press() || key.is_repeat() => match key.code {
                KeyCode::Up => {
                    self.message_scroll = self.message_scroll.saturating_sub(1);
                }
                KeyCode::Down => {
                    self.message_scroll = self.message_scroll_max.min(self.message_scroll + 1);
                }
                KeyCode::Char('i') => self.currently_selected = Selected::InputBox,
                KeyCode::Char('c') => self.currently_selected = Selected::CallSignBox,
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
            .map(|t| self.dictionary.format_signals(t.message()).lines().count())
            .sum();
    }

    fn handle_crossterm_event(&mut self, event: CrosstermEvent) -> color_eyre::Result<()> {
        match self.currently_selected {
            Selected::InputBox => self.handle_event_input_box(event)?,
            Selected::ChatBox => self.handle_event_chat_box(event)?,
            Selected::CallSignBox => self.handle_event_call_sign_box(event)?,
        }
        Ok(())
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();

        let default_style = Style::new().dark_gray();
        let selected_style = Style::new().white();

        let layout = Layout::horizontal([Constraint::Length(8), Constraint::Fill(5)]).spacing(2);
        let [active_call_signs_area, main_area] = area.layout(&layout);

        let active_call_signs_block = Block::bordered()
            .border_type(BorderType::Plain)
            .border_style(default_style)
            .title("ACTIVE".white());

        let active_call_signs = Paragraph::new(
            self.active_call_signs
                .iter()
                .map(|call_sign| {
                    Line::from(
                        call_sign
                            .to_string()
                            .set_style(Style::new().fg(call_sign.color())),
                    )
                })
                .collect::<Vec<_>>(),
        )
        .block(active_call_signs_block)
        .centered();

        frame.render_widget(active_call_signs, active_call_signs_area);

        let input_box_height = { self.text_area.lines().len() + 2 }
            .try_into()
            .unwrap_or(area.height);

        let central_layout = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Max(input_box_height),
        ]);

        let [chat_area, status_bar_area, bottom_area] = main_area.layout(&central_layout);

        let bottom_layout = Layout::horizontal([
            Constraint::Length(7),
            Constraint::Length(1),
            Constraint::Fill(1),
        ]);
        let [call_sign_area, _, input_area] = bottom_area.layout(&bottom_layout);

        self.input_box_area = input_area;
        self.call_sign_box_area = call_sign_area;

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
                .map(|s| Constraint::Max(s.lines().count() as u16)),
        );

        let messages_area = Rect {
            y: message_area.y + 1,
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

            let color = transmission.sender().color();

            let layout = Layout::horizontal([
                Constraint::Length(2),
                Constraint::Max(4),
                Constraint::Max(5),
                Constraint::Fill(1),
            ]);
            let [_, id_area, sender_area, content_area] = rect.layout(&layout);

            let message = Paragraph::new(message.as_str())
                .white()
                .scroll((scroll_amount, 0));

            frame.render_widget(message, content_area);

            let id = Line::raw(format!("{:03}", transmission.id())).dark_gray();
            frame.render_widget(id, id_area);

            let sender =
                Line::raw(transmission.sender().to_string()).set_style(Style::new().fg(color));
            frame.render_widget(sender, sender_area);
        }

        let scroll_bar =
            Scrollbar::new(ScrollbarOrientation::VerticalRight).style(Style::new().white());
        let mut scroll_bar_state = ScrollbarState::new(
            self.message_scroll_max
                .saturating_sub(message_area.height as usize * 2)
                .saturating_add(2),
        )
        .position(self.message_scroll);

        frame.render_stateful_widget(scroll_bar, scroll_bar_area, &mut scroll_bar_state);

        let clear = Clear::default();
        frame.render_widget(clear, status_bar_area.union(input_area));

        let chat_block = Block::bordered()
            .border_type(BorderType::Plain)
            .border_style(if let Selected::ChatBox = self.currently_selected {
                selected_style
            } else {
                default_style
            })
            .merge_borders(MergeStrategy::Replace);
        frame.render_widget(chat_block, chat_area);

        let input_block = Block::bordered()
            .border_type(BorderType::Plain)
            .border_style(if let Selected::InputBox = self.currently_selected {
                selected_style
            } else {
                default_style
            })
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

        let call_sign_block = Block::bordered()
            .border_type(BorderType::Plain)
            .border_style(if let Selected::CallSignBox = self.currently_selected {
                selected_style
            } else {
                default_style
            })
            .merge_borders(MergeStrategy::Fuzzy);

        let call_sign_id = u16::from_str_radix(&self.call_sign_text_area.lines()[0], 8);
        let color = call_sign_id
            .map(|id| CallSign::new(id).color())
            .unwrap_or(Color::White);
        self.call_sign_text_area.set_style(Style::new().fg(color));
        self.call_sign_text_area.set_cursor_line_style(Style::new());

        if let Selected::CallSignBox = self.currently_selected {
            self.call_sign_text_area
                .set_cursor_style(Style::new().on_white());
        } else {
            self.call_sign_text_area.set_cursor_style(Style::new());
        }

        self.call_sign_text_area.set_block(call_sign_block);
        frame.render_widget(&self.call_sign_text_area, call_sign_area);

        let status_bar_content = match &self.status_bar {
            StatusBar::Nothing => Span::default(),
            StatusBar::Error(err) => err.as_str().red(),
            StatusBar::Ok(ok) => ok.as_str().green(),
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
