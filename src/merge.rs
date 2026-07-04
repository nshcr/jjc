use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Scrollbar;
use ratatui::widgets::ScrollbarOrientation;

use crate::buffer::TextBuffer;
use crate::config::AppConfig;
use crate::input;
use crate::render::StyledText;
use crate::render::string_lines;
use crate::scroll::ViewScroll;
use crate::scroll::scrollbar_area;
use crate::vim::Vim;
use crate::vim::VimMode;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Mode {
    Text,
    Command,
}

enum MergeContent {
    Text {
        left: String,
        base: String,
        right: String,
        output: TextBuffer,
    },
    Binary {
        left: Vec<u8>,
        base: Vec<u8>,
        right: Vec<u8>,
        selected: Side,
    },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Side {
    Left,
    Base,
    Right,
}

pub struct MergeApp {
    content: MergeContent,
    output: PathBuf,
    marker_length: usize,
    path: String,
    mode: Mode,
    vim: Vim,
    command: String,
    scroll: ViewScroll,
    config: AppConfig,
}

impl MergeApp {
    pub fn open(
        left: PathBuf,
        base: PathBuf,
        right: PathBuf,
        output: PathBuf,
        marker_length: usize,
        path: String,
    ) -> io::Result<Self> {
        let left_bytes = fs::read(&left)?;
        let base_bytes = fs::read(&base)?;
        let right_bytes = fs::read(&right)?;
        let content = match (
            String::from_utf8(left_bytes.clone()),
            String::from_utf8(base_bytes.clone()),
            String::from_utf8(right_bytes.clone()),
            read_optional_text(&output)?,
        ) {
            (Ok(left), Ok(base), Ok(right), Some(output)) => MergeContent::Text {
                left,
                base,
                right,
                output: TextBuffer::from_text(&output),
            },
            _ => MergeContent::Binary {
                left: left_bytes,
                base: base_bytes,
                right: right_bytes,
                selected: Side::Right,
            },
        };
        Ok(Self {
            content,
            output,
            marker_length,
            path,
            mode: Mode::Text,
            vim: Vim::new(),
            command: String::new(),
            scroll: ViewScroll::default(),
            config: AppConfig::load()?,
        })
    }

    pub fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
        loop {
            terminal.draw(|frame| {
                let rows = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(1)])
                    .split(frame.area());
                let columns = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(25),
                        Constraint::Percentage(25),
                        Constraint::Percentage(25),
                        Constraint::Percentage(25),
                    ])
                    .split(rows[0]);
                match &self.content {
                    MergeContent::Text {
                        left,
                        base,
                        right,
                        output,
                    } => {
                        let path = Path::new(&self.path);
                        let left_lines = string_lines(left);
                        let base_lines = string_lines(base);
                        let right_lines = string_lines(right);
                        let height = columns[3].height.saturating_sub(2) as usize;
                        self.scroll
                            .keep_visible(output.cursor_y(), output.lines().len(), height);
                        let scroll = self.scroll.offset();
                        let mut scrollbar_state =
                            self.scroll.scrollbar_state(output.lines().len(), height);
                        frame.render_widget(
                            pane("left", pane_lines(path, &left_lines, &self.config), scroll),
                            columns[0],
                        );
                        frame.render_widget(
                            pane("base", pane_lines(path, &base_lines, &self.config), scroll),
                            columns[1],
                        );
                        frame.render_widget(
                            pane(
                                "right",
                                pane_lines(path, &right_lines, &self.config),
                                scroll,
                            ),
                            columns[2],
                        );
                        frame.render_widget(
                            pane(
                                "output",
                                pane_lines(path, output.lines(), &self.config),
                                scroll,
                            ),
                            columns[3],
                        );
                        frame.render_stateful_widget(
                            Scrollbar::new(ScrollbarOrientation::VerticalRight),
                            scrollbar_area(columns[3]),
                            &mut scrollbar_state,
                        );
                        let x = columns[3].x
                            + 1
                            + output
                                .cursor_column()
                                .min(columns[3].width.saturating_sub(2) as usize)
                                as u16;
                        let y = columns[3].y
                            + 1
                            + self.scroll.visible_line(output.cursor_y(), height) as u16;
                        frame.set_cursor_position((x, y));
                    }
                    MergeContent::Binary {
                        left,
                        base,
                        right,
                        selected,
                    } => {
                        frame.render_widget(
                            binary_pane("left", left, *selected == Side::Left),
                            columns[0],
                        );
                        frame.render_widget(
                            binary_pane("base", base, *selected == Side::Base),
                            columns[1],
                        );
                        frame.render_widget(
                            binary_pane("right", right, *selected == Side::Right),
                            columns[2],
                        );
                        frame.render_widget(
                            Paragraph::new("binary conflict\nmanual editing disabled")
                                .block(Block::bordered().title("output")),
                            columns[3],
                        );
                    }
                }
                frame.render_widget(Paragraph::new(self.status()), rows[1]);
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

    fn handle_key(&mut self, key: KeyEvent) -> io::Result<bool> {
        match self.mode {
            Mode::Text => self.handle_text(key),
            Mode::Command => self.handle_command(key),
        }
    }

    fn handle_text(&mut self, key: KeyEvent) -> io::Result<bool> {
        if self.is_text() {
            if self.vim.mode() == VimMode::Normal && key.code == KeyCode::Char(':') {
                self.mode = Mode::Command;
                self.command.clear();
                return Ok(false);
            }
            if let MergeContent::Text { output, .. } = &mut self.content
                && self.vim.handle_key(output, key)
            {
                return Ok(false);
            }
        }

        match key.code {
            KeyCode::Char('1') => self.accept_side(Side::Left),
            KeyCode::Char('2') => self.accept_side(Side::Base),
            KeyCode::Char('3') => self.accept_side(Side::Right),
            KeyCode::Char('w') => {
                self.save()?;
                return Ok(true);
            }
            KeyCode::Char('q') => {
                return Err(io::Error::new(io::ErrorKind::Interrupted, "merge canceled"));
            }
            _ => {}
        }
        Ok(false)
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
                    self.save()?;
                    return Ok(true);
                }
                "q!" => return Err(io::Error::new(io::ErrorKind::Interrupted, "merge canceled")),
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
        match (&self.content, self.mode) {
            (MergeContent::Binary { .. }, _) => format!(
                "{}  marker-length={}  BINARY  1 left  2 base  3 right  w save  q cancel",
                self.path, self.marker_length
            ),
            (_, Mode::Text) if self.vim.mode() == VimMode::Normal => format!(
                "{}  marker-length={}  NORMAL  1 left  2 base  3 right  i/a/o edit  :wq save  q cancel",
                self.path, self.marker_length
            ),
            (_, Mode::Text) => format!("{}  INSERT  Esc normal", self.path),
            (_, Mode::Command) => format!(":{}", self.command),
        }
    }

    fn accept_side(&mut self, side: Side) {
        match &mut self.content {
            MergeContent::Text {
                left,
                base,
                right,
                output,
            } => output.set_text(match side {
                Side::Left => left,
                Side::Base => base,
                Side::Right => right,
            }),
            MergeContent::Binary { selected, .. } => *selected = side,
        }
    }

    fn save(&self) -> io::Result<()> {
        match &self.content {
            MergeContent::Text { output, .. } => fs::write(&self.output, output.to_text()),
            MergeContent::Binary {
                left,
                base,
                right,
                selected,
            } => fs::write(
                &self.output,
                match selected {
                    Side::Left => left,
                    Side::Base => base,
                    Side::Right => right,
                },
            ),
        }
    }

    fn is_text(&self) -> bool {
        matches!(self.content, MergeContent::Text { .. })
    }
}

fn read_optional_text(path: &Path) -> io::Result<Option<String>> {
    match fs::read(path) {
        Ok(bytes) => Ok(String::from_utf8(bytes).ok()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Some(String::new())),
        Err(err) => Err(err),
    }
}

fn pane(title: &str, lines: Vec<Line<'static>>, scroll: usize) -> Paragraph<'static> {
    Paragraph::new(lines)
        .scroll((scroll as u16, 0))
        .block(Block::bordered().title(title.to_owned()))
}

fn pane_lines(path: &Path, lines: &[String], config: &AppConfig) -> Vec<Line<'static>> {
    StyledText::new(path, lines, config).lines()
}

fn binary_pane(title: &str, bytes: &[u8], selected: bool) -> Paragraph<'static> {
    let marker = if selected { "selected" } else { "" };
    Paragraph::new(format!("binary\n{} bytes\n{marker}", bytes.len()))
        .block(Block::bordered().title(title.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    static NEXT_TEMP_ID: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn save_writes_output_buffer() {
        let (root, output) = temp_output();
        let mut app = app(output.clone());
        app.accept_side(Side::Right);
        if let MergeContent::Text { output, .. } = &mut app.content {
            output.set_text("manual\n");
        }

        app.save().unwrap();

        assert_eq!(fs::read_to_string(output).unwrap(), "manual\n");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn accept_side_then_save_writes_that_side() {
        let (root, output) = temp_output();
        let mut app = app(output.clone());
        app.accept_side(Side::Right);
        app.save().unwrap();

        assert_eq!(fs::read_to_string(output).unwrap(), "right\n");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn binary_accept_side_writes_bytes() {
        let (root, output) = temp_output();
        let app = MergeApp {
            content: MergeContent::Binary {
                left: vec![0, 1],
                base: vec![2, 3],
                right: vec![4, 5],
                selected: Side::Left,
            },
            output: output.clone(),
            marker_length: 7,
            path: "file.bin".into(),
            mode: Mode::Text,
            vim: Vim::new(),
            command: String::new(),
            scroll: ViewScroll::default(),
            config: AppConfig::default(),
        };

        app.save().unwrap();

        assert_eq!(fs::read(output).unwrap(), vec![0, 1]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn q_cancels_merge() {
        let (root, output) = temp_output();
        let mut app = app(output);

        let err = app
            .handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE))
            .unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::Interrupted);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn text_merge_writes_with_command_wq() {
        let (root, output) = temp_output();
        let mut app = app(output.clone());

        app.handle_key(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE))
            .unwrap();
        app.handle_key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE))
            .unwrap();
        app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE))
            .unwrap();
        app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE))
            .unwrap();
        assert!(
            app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
                .unwrap()
        );

        assert_eq!(fs::read_to_string(output).unwrap(), "right\n");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn merge_pane_lines_highlight_non_rust_code() {
        let lines = vec![
            "def main():".to_owned(),
            "    return \"right\" # side".to_owned(),
        ];
        let rendered = pane_lines(Path::new("app.py"), &lines, &AppConfig::default());

        assert!(has_span(
            &rendered,
            "return",
            crate::syntax::HighlightClass::Keyword
        ));
        assert!(has_span(
            &rendered,
            "\"right\"",
            crate::syntax::HighlightClass::String
        ));
        assert!(has_span(
            &rendered,
            "# side",
            crate::syntax::HighlightClass::Comment
        ));
    }

    fn app(output: PathBuf) -> MergeApp {
        MergeApp {
            content: MergeContent::Text {
                left: "left\n".into(),
                base: "base\n".into(),
                right: "right\n".into(),
                output: TextBuffer::from_text(""),
            },
            output,
            marker_length: 7,
            path: "file.txt".into(),
            mode: Mode::Text,
            vim: Vim::new(),
            command: String::new(),
            scroll: ViewScroll::default(),
            config: AppConfig::default(),
        }
    }

    fn temp_output() -> (PathBuf, PathBuf) {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("jjc-merge-test-{}-{id}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let output = root.join("out.txt");
        (root, output)
    }

    fn has_span(lines: &[Line<'_>], text: &str, class: crate::syntax::HighlightClass) -> bool {
        let style = AppConfig::default().theme.style(class);
        lines.iter().any(|line| {
            line.spans
                .iter()
                .any(|span| span.content.as_ref() == text && span.style == style)
        })
    }
}
