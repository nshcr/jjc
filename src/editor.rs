use std::fs;
use std::io;
use std::path::PathBuf;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Scrollbar;
use ratatui::widgets::ScrollbarOrientation;

use crate::buffer::AgentSuggestion;
use crate::buffer::TextBuffer;
use crate::input;
use crate::scroll::ViewScroll;
use crate::scroll::scrollbar_area;
use crate::vim::Vim;
use crate::vim::VimMode;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Mode {
    Command,
    Text,
}

pub struct Editor {
    path: PathBuf,
    buffer: TextBuffer,
    mode: Mode,
    vim: Vim,
    command: String,
    scroll: ViewScroll,
}

impl Editor {
    pub fn open(path: PathBuf) -> io::Result<Self> {
        let content = fs::read_to_string(&path)?;
        Ok(Self {
            path,
            buffer: TextBuffer::from_text(&content),
            mode: Mode::Text,
            vim: Vim::new(),
            command: String::new(),
            scroll: ViewScroll::default(),
        })
    }

    pub fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
        loop {
            terminal.draw(|frame| {
                let rows = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(1)])
                    .split(frame.area());
                let text = self
                    .buffer
                    .lines()
                    .iter()
                    .map(|line| {
                        if line.starts_with("JJ:") {
                            Line::from(Span::styled(line, Style::new().add_modifier(Modifier::DIM)))
                        } else {
                            Line::from(line.as_str())
                        }
                    })
                    .collect::<Vec<_>>();
                let height = rows[0].height.saturating_sub(2) as usize;
                self.scroll
                    .keep_visible(self.buffer.cursor_y(), text.len(), height);
                let mut scrollbar_state = self.scroll.scrollbar_state(text.len(), height);

                frame.render_widget(
                    Paragraph::new(text)
                        .scroll((self.scroll.offset() as u16, 0))
                        .block(
                            Block::bordered().title(format!("jjc edit {}", self.path.display())),
                        ),
                    rows[0],
                );
                frame.render_stateful_widget(
                    Scrollbar::new(ScrollbarOrientation::VerticalRight),
                    scrollbar_area(rows[0]),
                    &mut scrollbar_state,
                );
                frame.render_widget(Paragraph::new(self.status()), rows[1]);

                let visible_y = self.scroll.visible_line(self.buffer.cursor_y(), height);
                let x = rows[0].x
                    + 1
                    + self
                        .buffer
                        .cursor_column()
                        .min(rows[0].width.saturating_sub(2) as usize) as u16;
                let y = rows[0].y + 1 + visible_y as u16;
                frame.set_cursor_position((x, y));
            })?;

            if self.handle_key(input::read_key()?)? {
                return Ok(());
            }
        }
    }

    pub fn run_scripted(&mut self) -> io::Result<()> {
        loop {
            if self.handle_key(input::read_key()?)? {
                return Ok(());
            }
        }
    }

    pub fn apply_suggestion(&mut self, suggestion: AgentSuggestion) {
        self.buffer.apply(suggestion.into_command());
    }

    fn handle_key(&mut self, key: KeyEvent) -> io::Result<bool> {
        match self.mode {
            Mode::Text => {
                if self.vim.mode() == VimMode::Normal && key.code == KeyCode::Char(':') {
                    self.mode = Mode::Command;
                    self.command.clear();
                    return Ok(false);
                }
                self.vim.handle_key(&mut self.buffer, key);
                Ok(false)
            }
            Mode::Command => self.handle_command(key),
        }
    }

    fn handle_command(&mut self, key: KeyEvent) -> io::Result<bool> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Text;
                self.vim.set_normal();
            }
            KeyCode::Backspace => {
                self.command.pop();
            }
            KeyCode::Enter => match self.command.as_str() {
                "wq" => {
                    fs::write(&self.path, self.buffer.to_text())?;
                    return Ok(true);
                }
                "q!" => {
                    return Err(io::Error::new(io::ErrorKind::Interrupted, "edit canceled"));
                }
                _ => {
                    self.command.clear();
                    self.mode = Mode::Text;
                    self.vim.set_normal();
                }
            },
            KeyCode::Char(c) => self.command.push(c),
            _ => {}
        }
        Ok(false)
    }

    fn status(&self) -> String {
        match self.mode {
            Mode::Text if self.vim.mode() == VimMode::Normal => {
                "NORMAL  i/a/o insert  h/j/k/l/w/b/e move  x/dd delete  yy/p paste  u/C-r undo  :wq save  :q! cancel".to_owned()
            }
            Mode::Text => "INSERT  Esc normal".to_owned(),
            Mode::Command => format!(":{}", self.command),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::time::SystemTime;
    use std::time::UNIX_EPOCH;

    static NEXT_TEMP_ID: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn q_bang_cancels_and_discards_buffer_changes() {
        let (root, path) = temp_file("original\n");
        let mut editor = Editor::open(path.clone()).unwrap();

        editor.handle_key(key('i')).unwrap();
        editor.handle_key(key('X')).unwrap();
        editor.handle_key(esc()).unwrap();
        editor.handle_key(key(':')).unwrap();
        editor.handle_key(key('q')).unwrap();
        assert!(editor.handle_key(key('!')).unwrap() == false);
        let err = editor.handle_key(enter()).unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::Interrupted);
        assert_eq!(fs::read_to_string(path).unwrap(), "original\n");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn wq_preserves_jj_comment_lines() {
        let content = "message\nJJ: keep this comment exactly\n";
        let (root, path) = temp_file(content);
        let mut editor = Editor::open(path.clone()).unwrap();

        editor.handle_key(key(':')).unwrap();
        editor.handle_key(key('w')).unwrap();
        editor.handle_key(key('q')).unwrap();
        assert!(editor.handle_key(enter()).unwrap());

        assert_eq!(fs::read_to_string(path).unwrap(), content);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn suggestion_updates_buffer_but_does_not_write_until_save() {
        let (root, path) = temp_file("original\n");
        let mut editor = Editor::open(path.clone()).unwrap();

        editor.apply_suggestion(AgentSuggestion::replace_all("suggested\n"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "original\n");

        editor.handle_key(key(':')).unwrap();
        editor.handle_key(key('w')).unwrap();
        editor.handle_key(key('q')).unwrap();
        assert!(editor.handle_key(enter()).unwrap());

        assert_eq!(fs::read_to_string(path).unwrap(), "suggested\n");
        fs::remove_dir_all(root).unwrap();
    }

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    fn esc() -> KeyEvent {
        KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)
    }

    fn enter() -> KeyEvent {
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
    }

    fn temp_file(content: &str) -> (PathBuf, PathBuf) {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "jjc-editor-test-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            id
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("message.txt");
        fs::write(&path, content).unwrap();
        (root, path)
    }
}
