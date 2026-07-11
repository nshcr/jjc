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
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Scrollbar;
use ratatui::widgets::ScrollbarOrientation;

use crate::buffer::AgentSuggestion;
use crate::buffer::TextBuffer;
use crate::config::AppConfig;
use crate::input;
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
    Command,
    Text,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum EditProfile {
    Description,
    Sparse,
    Generic,
}

impl EditProfile {
    fn for_path(path: &std::path::Path) -> Self {
        match path.extension().and_then(|extension| extension.to_str()) {
            Some("jjdescription") => Self::Description,
            Some("jjsparse") => Self::Sparse,
            _ => Self::Generic,
        }
    }

    fn dims_jj_instructions(self) -> bool {
        matches!(self, Self::Description | Self::Sparse)
    }
}

pub struct Editor {
    path: PathBuf,
    buffer: TextBuffer,
    mode: Mode,
    vim: Vim,
    command: String,
    profile: EditProfile,
    pending_empty_save: bool,
    scroll: ViewScroll,
    config: AppConfig,
    render_cache: StyledTextCache,
}

impl Editor {
    pub fn open(path: PathBuf) -> io::Result<Self> {
        let content = fs::read_to_string(&path)?;
        let profile = EditProfile::for_path(&path);
        Ok(Self {
            path,
            buffer: TextBuffer::from_text(&content),
            mode: Mode::Text,
            vim: Vim::new(),
            command: String::new(),
            profile,
            pending_empty_save: false,
            scroll: ViewScroll::default(),
            config: AppConfig::load()?,
            render_cache: StyledTextCache::default(),
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
                let text = self.render_text();
                let height = rows[0].height.saturating_sub(2) as usize;
                let width = rows[0].width.saturating_sub(2) as usize;
                self.scroll
                    .keep_visible(self.buffer.cursor_y(), text.len(), height);
                let cursor_column = display_width(
                    &self.buffer.current_line()[..self.buffer.cursor_byte()],
                    TAB_WIDTH,
                );
                let content_width = self
                    .buffer
                    .lines()
                    .iter()
                    .map(|line| display_width(line, TAB_WIDTH))
                    .max()
                    .unwrap_or(0);
                self.scroll
                    .keep_column_visible(cursor_column, content_width, width);
                self.scroll
                    .set_horizontal_offset(display_boundary_at_or_after(
                        self.buffer.current_line(),
                        self.scroll.horizontal_offset(),
                        TAB_WIDTH,
                    ));
                let mut scrollbar_state = self.scroll.scrollbar_state(text.len(), height);

                frame.render_widget(
                    Paragraph::new(text)
                        .scroll((
                            terminal_offset(self.scroll.offset()),
                            terminal_offset(self.scroll.horizontal_offset()),
                        ))
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
                let visible_x = self.scroll.visible_column(cursor_column, width);
                let x = rows[0].x + 1 + visible_x as u16;
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
                if self.vim.handle_key(&mut self.buffer, key) {
                    self.pending_empty_save = false;
                }
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
                "wq" => return self.save(),
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
                if self.pending_empty_save {
                    return "EMPTY MESSAGE  edit content or :wq again to save anyway".to_owned();
                }
                if is_large_content(self.buffer.lines()) {
                    return "NORMAL  PLAIN LARGE FILE  syntax disabled  :wq save  :q! cancel"
                        .to_owned();
                }
                "NORMAL  i/a/o insert  h/j/k/l/w/b/e move  x/dd delete  yy/p paste  u/C-r undo  :wq save  :q! cancel".to_owned()
            }
            Mode::Text => "INSERT  Esc normal".to_owned(),
            Mode::Command => format!(":{}", self.command),
        }
    }

    fn cursor_mode(&self) -> VimMode {
        match self.mode {
            Mode::Text => self.vim.mode(),
            Mode::Command => VimMode::Normal,
        }
    }

    fn render_text(&mut self) -> Vec<Line<'static>> {
        self.render_cache.lines_with_jj_instructions(
            &self.path,
            self.buffer.lines(),
            &self.config,
            self.profile.dims_jj_instructions(),
        )
    }

    fn save(&mut self) -> io::Result<bool> {
        if self.profile == EditProfile::Description
            && self.message_is_empty()
            && !self.pending_empty_save
        {
            self.pending_empty_save = true;
            self.mode = Mode::Text;
            self.vim.set_normal();
            return Ok(false);
        }
        fs::write(&self.path, self.buffer.to_text())?;
        Ok(true)
    }

    fn message_is_empty(&self) -> bool {
        self.buffer
            .lines()
            .iter()
            .filter(|line| !line.starts_with("JJ:"))
            .all(|line| line.trim().is_empty())
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
        assert!(!editor.handle_key(key('!')).unwrap());
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
    fn wq_warns_before_saving_empty_message() {
        let (root, path) = temp_file_with_suffix("\nJJ: comment\n", "jjdescription");
        let mut editor = Editor::open(path.clone()).unwrap();

        editor.handle_key(key(':')).unwrap();
        editor.handle_key(key('w')).unwrap();
        editor.handle_key(key('q')).unwrap();
        assert!(!editor.handle_key(enter()).unwrap());

        assert_eq!(fs::read_to_string(&path).unwrap(), "\nJJ: comment\n");
        assert!(editor.status().contains("EMPTY MESSAGE"));

        editor.handle_key(key(':')).unwrap();
        editor.handle_key(key('w')).unwrap();
        editor.handle_key(key('q')).unwrap();
        assert!(editor.handle_key(enter()).unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn generic_empty_file_saves_without_commit_message_warning() {
        let (root, path) = temp_file("");
        let mut editor = Editor::open(path).unwrap();

        editor.handle_key(key(':')).unwrap();
        editor.handle_key(key('w')).unwrap();
        editor.handle_key(key('q')).unwrap();
        assert!(editor.handle_key(enter()).unwrap());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn sparse_empty_file_saves_without_commit_message_warning() {
        let (root, path) = temp_file_with_suffix("", "jjsparse");
        let mut editor = Editor::open(path).unwrap();

        editor.handle_key(key(':')).unwrap();
        editor.handle_key(key('w')).unwrap();
        editor.handle_key(key('q')).unwrap();
        assert!(editor.handle_key(enter()).unwrap());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn edit_profile_is_inferred_from_jj_temp_suffix() {
        assert_eq!(
            EditProfile::for_path(std::path::Path::new("editor-1.jjdescription")),
            EditProfile::Description
        );
        assert_eq!(
            EditProfile::for_path(std::path::Path::new("editor-1.jjsparse")),
            EditProfile::Sparse
        );
        assert_eq!(
            EditProfile::for_path(std::path::Path::new("notes.txt")),
            EditProfile::Generic
        );
    }

    #[test]
    fn generic_jj_prefix_is_rendered_as_plain_text() {
        let (root, path) = temp_file("JJ: ordinary text\n");
        let mut editor = Editor::open(path).unwrap();

        let rendered = editor.render_text();
        assert!(
            !rendered[0].spans[0]
                .style
                .add_modifier
                .contains(ratatui::style::Modifier::DIM)
        );

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
        temp_file_with_suffix(content, "txt")
    }

    fn temp_file_with_suffix(content: &str, suffix: &str) -> (PathBuf, PathBuf) {
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
        let path = root.join(format!("message.{suffix}"));
        fs::write(&path, content).unwrap();
        (root, path)
    }
}
