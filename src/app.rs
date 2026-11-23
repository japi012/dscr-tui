use std::time::Duration;

use crossterm::event::{
    self, Event, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use crossterm::{
    event::{
        DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        MouseButton, MouseEvent, MouseEventKind,
    },
    execute,
};
use ratatui::layout::{Constraint, Layout};
use ratatui::symbols::merge::MergeStrategy;
use ratatui::widgets::{Block, BorderType, Paragraph};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Position, Rect},
    style::Style,
    widgets::Widget,
};
use ratatui_textarea::{CursorMove, DataCursor, Input, Key, TextArea};

#[derive(Debug)]
pub struct App {
    pub running: bool,
    text_area: TextArea<'static>,
    messages: Vec<String>,
}

impl App {
    pub fn new() -> Self {
        Self {
            running: true,
            text_area: TextArea::new(Vec::new()),
            messages: Vec::new(),
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

    pub fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let buf = frame.buffer_mut();

        let input_box_height = { self.text_area.lines().len() + 2 }
            .try_into()
            .unwrap_or(area.height);

        let layout =
            Layout::vertical([Constraint::Fill(1), Constraint::Max(input_box_height)]).spacing(-1);
        let [chat_area, input_area] = area.layout(&layout);

        let chat_block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title("CHAT BOX")
            .merge_borders(MergeStrategy::Fuzzy);
        let messages = self.messages.join("\n");
        let messages_len = self.messages.len() as u16;
        let chat = Paragraph::new(messages)
            .block(chat_block)
            .scroll((messages_len.saturating_sub(chat_area.height), 0));
        chat.render(chat_area, buf);

        let input_block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title("INPUT BOX")
            .merge_borders(MergeStrategy::Fuzzy);

        self.text_area.set_block(input_block);
        self.text_area.set_cursor_line_style(Style::default());
        self.text_area.render(input_area, buf);
    }

    pub fn handle_events(&mut self, area: Rect) -> color_eyre::Result<()> {
        if event::poll(Duration::from_millis(16))? {
            match event::read()? {
                Event::Mouse(mouse_event) => handle_mouse(&mut self.text_area, area, mouse_event),
                Event::Paste(pasted) => {
                    self.text_area.set_yank_text(pasted);
                    self.text_area.paste();
                }
                Event::Key(key_event) if key_event.is_press() => {
                    // let displayed_input = format!("{key_event:?}");
                    let input: Input = key_event.into();
                    self.messages.push(format!("{input:?}"));
                    match input {
                        Input { key: Key::Esc, .. } => self.quit(),
                        // Input {
                        //     key: Key::Enter,
                        //     shift: true,
                        //     ..
                        // } => {
                        //     todo!("uhhhhh");
                        //     self.text_area.insert_newline();
                        // }
                        // Input {
                        //     key: Key::Enter,
                        //     shift: false,
                        //     ..
                        // } => {
                        //     todo!("message sending");
                        // }
                        input => {
                            self.text_area.input(input);
                        }
                    };
                }
                _ => (),
            }
        }
        Ok(())
    }

    // pub fn handle_key_event(&mut self, key_event: KeyEvent) -> color_eyre::Result<()> {
    //     let KeyEvent { code, .. } = key_event;
    //     match code {
    //         KeyCode::Char('p') => panic!("p for panic"),
    //         KeyCode::Char('q') => self.running = false,
    //         _ => (),
    //     }
    //     Ok(())
    // }
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
