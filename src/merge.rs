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
#[cfg(test)]
use crate::render::StyledText;
use crate::render::StyledTextCache;
use crate::render::TAB_WIDTH;
use crate::render::display_boundary_at_or_after;
use crate::render::display_width;
use crate::render::is_large_content;
use crate::render::set_vim_cursor_style;
use crate::scroll::ViewScroll;
use crate::scroll::scrollbar_area;
use crate::scroll::terminal_offset;
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
    pending_marker_save: bool,
    scroll: ViewScroll,
    config: AppConfig,
    left_cache: StyledTextCache,
    base_cache: StyledTextCache,
    right_cache: StyledTextCache,
    output_cache: StyledTextCache,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ConflictBlock {
    start: usize,
    base_marker: Option<usize>,
    separator: usize,
    end: usize,
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
        if marker_length == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "marker length must be greater than zero",
            ));
        }
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
            pending_marker_save: false,
            scroll: ViewScroll::default(),
            config: AppConfig::load()?,
            left_cache: StyledTextCache::default(),
            base_cache: StyledTextCache::default(),
            right_cache: StyledTextCache::default(),
            output_cache: StyledTextCache::default(),
        })
    }

    pub fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
        loop {
            set_vim_cursor_style(terminal.backend_mut(), self.cursor_mode())?;
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
                        let height = columns[3].height.saturating_sub(2) as usize;
                        let width = columns[3].width.saturating_sub(2) as usize;
                        self.scroll
                            .keep_visible(output.cursor_y(), output.lines().len(), height);
                        let cursor_column = display_width(
                            &output.current_line()[..output.cursor_byte()],
                            TAB_WIDTH,
                        );
                        let content_width = output
                            .lines()
                            .iter()
                            .map(|line| display_width(line, TAB_WIDTH))
                            .max()
                            .unwrap_or(0);
                        self.scroll
                            .keep_column_visible(cursor_column, content_width, width);
                        self.scroll
                            .set_horizontal_offset(display_boundary_at_or_after(
                                output.current_line(),
                                self.scroll.horizontal_offset(),
                                TAB_WIDTH,
                            ));
                        let scroll = self.scroll.offset();
                        let horizontal_scroll = self.scroll.horizontal_offset();
                        let mut scrollbar_state =
                            self.scroll.scrollbar_state(output.lines().len(), height);
                        frame.render_widget(
                            pane(
                                "left",
                                self.left_cache.text(path, left, &self.config),
                                scroll,
                                horizontal_scroll,
                            ),
                            columns[0],
                        );
                        frame.render_widget(
                            pane(
                                "base",
                                self.base_cache.text(path, base, &self.config),
                                scroll,
                                horizontal_scroll,
                            ),
                            columns[1],
                        );
                        frame.render_widget(
                            pane(
                                "right",
                                self.right_cache.text(path, right, &self.config),
                                scroll,
                                horizontal_scroll,
                            ),
                            columns[2],
                        );
                        frame.render_widget(
                            pane(
                                "output",
                                self.output_cache.lines(path, output.lines(), &self.config),
                                scroll,
                                horizontal_scroll,
                            ),
                            columns[3],
                        );
                        frame.render_stateful_widget(
                            Scrollbar::new(ScrollbarOrientation::VerticalRight),
                            scrollbar_area(columns[3]),
                            &mut scrollbar_state,
                        );
                        let visible_x = self.scroll.visible_column(cursor_column, width);
                        let x = columns[3].x + 1 + visible_x as u16;
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
            if self.vim.mode() == VimMode::Normal {
                match key.code {
                    KeyCode::Char('n') => {
                        self.jump_conflict(1);
                        return Ok(false);
                    }
                    KeyCode::Char('p') => {
                        self.jump_conflict(-1);
                        return Ok(false);
                    }
                    KeyCode::Char('1') => {
                        self.accept_side(Side::Left);
                        return Ok(false);
                    }
                    KeyCode::Char('2') => {
                        self.accept_side(Side::Base);
                        return Ok(false);
                    }
                    KeyCode::Char('3') => {
                        self.accept_side(Side::Right);
                        return Ok(false);
                    }
                    _ => {}
                }
            }
            if let MergeContent::Text { output, .. } = &mut self.content
                && self.vim.handle_key(output, key)
            {
                self.pending_marker_save = false;
                return Ok(false);
            }
        }

        match key.code {
            KeyCode::Char('1') => self.accept_side(Side::Left),
            KeyCode::Char('2') => self.accept_side(Side::Base),
            KeyCode::Char('3') => self.accept_side(Side::Right),
            KeyCode::Char('w') => {
                return self.save();
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
                    return self.save();
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
            (MergeContent::Text { output, .. }, _) if self.pending_marker_save => format!(
                "{}  conflict markers remain; save again to write anyway",
                self.path
            ),
            (MergeContent::Text { output, .. }, Mode::Text) if is_large_content(output.lines()) => {
                format!(
                    "{}  PLAIN LARGE FILE  syntax disabled  :wq save  q cancel",
                    self.path
                )
            }
            (MergeContent::Text { output, .. }, Mode::Text)
                if conflict_blocks(output.lines(), self.marker_length).len() == 1
                    && self.vim.mode() == VimMode::Normal =>
            {
                format!(
                    "{}  marker-length={}  NORMAL  n/p conflict  1 left  2 base  3 right  :wq save  q cancel",
                    self.path, self.marker_length
                )
            }
            (_, Mode::Text) if self.vim.mode() == VimMode::Normal => format!(
                "{}  marker-length={}  NORMAL  n/p conflict  1 left  2 base  3 right  i/a/o edit  :wq save  q cancel",
                self.path, self.marker_length
            ),
            (_, Mode::Text) => format!("{}  INSERT  Esc normal", self.path),
            (_, Mode::Command) => format!(":{}", self.command),
        }
    }

    fn cursor_mode(&self) -> VimMode {
        match self.mode {
            Mode::Text => self.vim.mode(),
            Mode::Command => VimMode::Normal,
        }
    }

    fn accept_side(&mut self, side: Side) {
        match &mut self.content {
            MergeContent::Text {
                left,
                base,
                right,
                output,
            } => {
                self.pending_marker_save = false;
                if !accept_current_conflict_block(output, side, self.marker_length) {
                    output.set_text(match side {
                        Side::Left => left,
                        Side::Base => base,
                        Side::Right => right,
                    });
                }
            }
            MergeContent::Binary { selected, .. } => *selected = side,
        }
    }

    fn jump_conflict(&mut self, delta: isize) {
        let MergeContent::Text { output, .. } = &mut self.content else {
            return;
        };
        let blocks = conflict_blocks(output.lines(), self.marker_length);
        if blocks.is_empty() {
            return;
        }
        let cursor = output.cursor_y();
        let target = if delta >= 0 {
            blocks
                .iter()
                .find(|block| block.start > cursor)
                .unwrap_or(&blocks[0])
        } else {
            blocks
                .iter()
                .rev()
                .find(|block| block.start < cursor)
                .unwrap_or_else(|| blocks.last().unwrap())
        };
        output.move_to_line(target.start);
    }

    fn save(&mut self) -> io::Result<bool> {
        match &self.content {
            MergeContent::Text { output, .. } => {
                if has_conflict_markers(output.lines(), self.marker_length)
                    && !self.pending_marker_save
                {
                    self.pending_marker_save = true;
                    return Ok(false);
                }
                fs::write(&self.output, output.to_text())?;
                Ok(true)
            }
            MergeContent::Binary {
                left,
                base,
                right,
                selected,
            } => {
                fs::write(
                    &self.output,
                    match selected {
                        Side::Left => left,
                        Side::Base => base,
                        Side::Right => right,
                    },
                )?;
                Ok(true)
            }
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

fn pane(
    title: &str,
    lines: Vec<Line<'static>>,
    scroll: usize,
    horizontal_scroll: usize,
) -> Paragraph<'static> {
    Paragraph::new(lines)
        .scroll((terminal_offset(scroll), terminal_offset(horizontal_scroll)))
        .block(Block::bordered().title(title.to_owned()))
}

#[cfg(test)]
fn pane_lines(path: &Path, lines: &[String], config: &AppConfig) -> Vec<Line<'static>> {
    StyledText::new(path, lines, config).lines()
}

fn binary_pane(title: &str, bytes: &[u8], selected: bool) -> Paragraph<'static> {
    let marker = if selected { "selected" } else { "" };
    Paragraph::new(format!("binary\n{} bytes\n{marker}", bytes.len()))
        .block(Block::bordered().title(title.to_owned()))
}

fn accept_current_conflict_block(
    output: &mut TextBuffer,
    side: Side,
    marker_length: usize,
) -> bool {
    let Some(block) = current_conflict_block(output.lines(), output.cursor_y(), marker_length)
    else {
        return false;
    };
    let replacement = match side {
        Side::Left => {
            output.lines()[block.start + 1..block.base_marker.unwrap_or(block.separator)].to_vec()
        }
        Side::Base => {
            let Some(base_marker) = block.base_marker else {
                return false;
            };
            output.lines()[base_marker + 1..block.separator].to_vec()
        }
        Side::Right => output.lines()[block.separator + 1..block.end].to_vec(),
    };
    output.replace_lines(block.start, block.end + 1, &replacement);
    true
}

fn current_conflict_block(
    lines: &[String],
    cursor: usize,
    marker_length: usize,
) -> Option<ConflictBlock> {
    conflict_blocks(lines, marker_length)
        .into_iter()
        .find(|block| block.start <= cursor && cursor <= block.end)
        .or_else(|| conflict_blocks(lines, marker_length).into_iter().next())
}

fn has_conflict_markers(lines: &[String], marker_length: usize) -> bool {
    !conflict_blocks(lines, marker_length).is_empty()
}

fn conflict_blocks(lines: &[String], marker_length: usize) -> Vec<ConflictBlock> {
    let mut blocks = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let Some(block) = parse_conflict_block(lines, index, marker_length) else {
            index += 1;
            continue;
        };
        index = block.end + 1;
        blocks.push(block);
    }
    blocks
}

fn parse_conflict_block(
    lines: &[String],
    start: usize,
    marker_length: usize,
) -> Option<ConflictBlock> {
    if !is_marker_line(lines.get(start)?, '<', marker_length) {
        return None;
    }
    let mut base_marker = None;
    let mut separator = None;
    for (index, line) in lines.iter().enumerate().skip(start + 1) {
        if is_marker_line(line, '<', marker_length) {
            return None;
        }
        if is_marker_line(line, '|', marker_length) {
            if base_marker.is_some() || separator.is_some() {
                return None;
            }
            base_marker = Some(index);
        } else if is_marker_line(line, '=', marker_length) {
            if separator.is_some() {
                return None;
            }
            separator = Some(index);
        } else if is_marker_line(line, '>', marker_length) {
            return Some(ConflictBlock {
                start,
                base_marker,
                separator: separator?,
                end: index,
            });
        }
    }
    None
}

fn is_marker_line(line: &str, marker: char, marker_length: usize) -> bool {
    marker_length > 0
        && line
            .chars()
            .take_while(|current| *current == marker)
            .count()
            == marker_length
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    static NEXT_TEMP_ID: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn open_rejects_zero_marker_length() {
        let error = MergeApp::open(
            PathBuf::from("missing-left"),
            PathBuf::from("missing-base"),
            PathBuf::from("missing-right"),
            PathBuf::from("missing-output"),
            0,
            "file.txt".to_owned(),
        )
        .err()
        .expect("zero marker length must fail before paths are read");

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("greater than zero"));
    }

    #[test]
    fn save_writes_output_buffer() {
        let (root, output) = temp_output();
        let mut app = app(output.clone());
        app.accept_side(Side::Right);
        if let MergeContent::Text { output, .. } = &mut app.content {
            output.set_text("manual\n");
        }

        assert!(app.save().unwrap());

        assert_eq!(fs::read_to_string(output).unwrap(), "manual\n");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn accept_side_then_save_writes_that_side() {
        let (root, output) = temp_output();
        let mut app = app(output.clone());
        app.accept_side(Side::Right);
        assert!(app.save().unwrap());

        assert_eq!(fs::read_to_string(output).unwrap(), "right\n");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn binary_accept_side_writes_bytes() {
        let (root, output) = temp_output();
        let mut app = MergeApp {
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
            pending_marker_save: false,
            scroll: ViewScroll::default(),
            config: AppConfig::default(),
            left_cache: StyledTextCache::default(),
            base_cache: StyledTextCache::default(),
            right_cache: StyledTextCache::default(),
            output_cache: StyledTextCache::default(),
        };

        assert!(app.save().unwrap());

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
    fn accepts_dynamic_length_block_without_consuming_short_marker_literals() {
        let (root, output_path) = temp_output();
        let mut app = app(output_path);
        app.marker_length = 11;
        if let MergeContent::Text { output, .. } = &mut app.content {
            output.set_text(
                "before\n<<<<<<<<<<< left\nleft\n||||||||||| base\nbase\n===========\nright-before\n>>>>>>>\nright-after\n>>>>>>>>>>> right\nafter\n",
            );
        }

        app.handle_key(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE))
            .unwrap();

        if let MergeContent::Text { output, .. } = &app.content {
            assert_eq!(
                output.to_text(),
                "before\nright-before\n>>>>>>>\nright-after\nafter\n"
            );
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn marker_lines_require_the_exact_configured_run_length() {
        let lines = vec![
            "<<<<<<<< short".to_owned(),
            "<<<<<<<<<< too-long".to_owned(),
            "<<<<<<<<< left".to_owned(),
            "left".to_owned(),
            "||||||||| base".to_owned(),
            "base".to_owned(),
            "=========".to_owned(),
            "right".to_owned(),
            ">>>>>>>>> right".to_owned(),
        ];

        assert_eq!(
            conflict_blocks(&lines, 9),
            vec![ConflictBlock {
                start: 2,
                base_marker: Some(4),
                separator: 6,
                end: 8,
            }]
        );
        assert!(conflict_blocks(&lines, 8).is_empty());
        assert!(conflict_blocks(&lines, 10).is_empty());
    }

    #[test]
    fn malformed_or_incomplete_marker_sequences_are_not_blocks() {
        let malformed = vec![
            "<<<<<<< left".to_owned(),
            "left".to_owned(),
            "=======".to_owned(),
            "right".to_owned(),
            "||||||| base-after-separator".to_owned(),
            "base".to_owned(),
            ">>>>>>> right".to_owned(),
        ];
        let incomplete = vec![
            "<<<<<<< left".to_owned(),
            "left".to_owned(),
            "=======".to_owned(),
            "right".to_owned(),
        ];

        assert!(conflict_blocks(&malformed, 7).is_empty());
        assert!(conflict_blocks(&incomplete, 7).is_empty());
        assert!(!has_conflict_markers(&malformed, 7));
        assert!(!has_conflict_markers(&incomplete, 7));
        assert!(conflict_blocks(&incomplete, 0).is_empty());
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

    #[test]
    fn accepts_current_conflict_block_side() {
        let (root, output_path) = temp_output();
        let mut app = app(output_path);
        if let MergeContent::Text { output, .. } = &mut app.content {
            output.set_text(
                "before\n<<<<<<< left\nleft\n||||||| base\nbase\n=======\nright\n>>>>>>> right\nafter\n",
            );
            output.move_to_line(2);
        }

        app.handle_key(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE))
            .unwrap();

        if let MergeContent::Text { output, .. } = &app.content {
            assert_eq!(output.to_text(), "before\nright\nafter\n");
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn save_warns_once_when_conflict_markers_remain() {
        let (root, output_path) = temp_output();
        fs::write(&output_path, "original\n").unwrap();
        let mut app = app(output_path.clone());
        if let MergeContent::Text { output, .. } = &mut app.content {
            output.set_text("<<<<<<< left\nleft\n=======\nright\n>>>>>>> right\n");
        }

        assert!(!app.save().unwrap());
        assert_eq!(fs::read_to_string(&output_path).unwrap(), "original\n");
        assert!(app.pending_marker_save);
        assert!(app.save().unwrap());
        assert!(
            fs::read_to_string(&output_path)
                .unwrap()
                .contains("<<<<<<<")
        );
        fs::remove_dir_all(root).unwrap();
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
            pending_marker_save: false,
            scroll: ViewScroll::default(),
            config: AppConfig::default(),
            left_cache: StyledTextCache::default(),
            base_cache: StyledTextCache::default(),
            right_cache: StyledTextCache::default(),
            output_cache: StyledTextCache::default(),
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
