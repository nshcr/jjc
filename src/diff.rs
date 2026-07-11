use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::str;

use crossterm::event::KeyCode;
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
use similar::Algorithm;
use similar::DiffOp;
use similar::DiffTag;
use similar::TextDiff;

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
use crate::syntax;
use crate::vim::Vim;
use crate::vim::VimMode;

pub struct DiffApp {
    output: PathBuf,
    files: Vec<DiffFile>,
    entries: Vec<Entry>,
    cursor: usize,
    line_cursor: usize,
    scroll: ViewScroll,
    mode: Mode,
    edit_vim: Vim,
    undo: Vec<SelectionSnapshot>,
    redo: Vec<SelectionSnapshot>,
    config: AppConfig,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Mode {
    Select,
    Edit,
}

struct DiffFile {
    path: PathBuf,
    kind: DiffFileKind,
    left_entry: TreeEntry,
    right_entry: TreeEntry,
    left_lines: Vec<String>,
    right_lines: Vec<String>,
    hunks: Vec<Hunk>,
    manual_output: Option<TextBuffer>,
    left_cache: StyledTextCache,
    right_cache: StyledTextCache,
    manual_cache: StyledTextCache,
    unsupported: Option<String>,
}

struct ChangedPath {
    path: PathBuf,
    left: TreeEntry,
    right: TreeEntry,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum DiffFileKind {
    ModifiedText,
    AddedText,
    DeletedText,
    ModifiedBinary,
    AddedBinary,
    DeletedBinary,
    EntryChange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TreeEntry {
    Missing,
    File { contents: Vec<u8>, executable: bool },
    Symlink { target: PathBuf },
    Directory,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TreeEntryKind {
    Missing,
    File,
    Symlink,
    Directory,
    Other,
}

#[derive(Clone, Copy)]
struct Entry {
    file: usize,
    hunk: usize,
}

struct Hunk {
    role: HunkRole,
    old_start: usize,
    old_end: usize,
    new_start: usize,
    new_end: usize,
    selected: bool,
    rows: Vec<DiffRow>,
    function: Option<String>,
    summary: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HunkRole {
    Content,
    Executable,
    Entry,
}

struct DiffRow {
    kind: DiffRowKind,
    selected: bool,
    group: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiffRowKind {
    Equal { new_index: usize },
    Delete { old_index: usize },
    Insert { new_index: usize },
}

#[derive(Clone, Eq, PartialEq)]
struct SelectionSnapshot(Vec<Vec<Vec<bool>>>);

impl DiffApp {
    pub fn open(left: PathBuf, right: PathBuf, output: PathBuf) -> io::Result<Self> {
        let paths = changed_paths(&left, &right)?;
        let mut files = Vec::new();
        let mut entries = Vec::new();

        for changed in paths {
            let file = DiffFile::load(changed.path, changed.left, changed.right)?;
            let file_index = files.len();
            for hunk_index in 0..file.hunks.len() {
                entries.push(Entry {
                    file: file_index,
                    hunk: hunk_index,
                });
            }
            files.push(file);
        }

        Ok(Self {
            output,
            files,
            entries,
            cursor: 0,
            line_cursor: 0,
            scroll: ViewScroll::default(),
            mode: Mode::Select,
            edit_vim: Vim::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            config: AppConfig::load()?,
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
                let height = rows[0].height.saturating_sub(2) as usize;
                let width = rows[0].width.saturating_sub(2) as usize;
                let line_count = self.line_count();
                self.scroll
                    .keep_visible(self.cursor_line_index(), line_count, height);
                let cursor_column = if self.mode == Mode::Edit {
                    self.current_edit_buffer()
                        .map(|buffer| {
                            display_width(&buffer.current_line()[..buffer.cursor_byte()], TAB_WIDTH)
                        })
                        .unwrap_or(0)
                } else {
                    0
                };
                let content_width = if self.mode == Mode::Edit {
                    self.current_edit_buffer()
                        .map(|buffer| {
                            buffer
                                .lines()
                                .iter()
                                .map(|line| display_width(line, TAB_WIDTH))
                                .max()
                                .unwrap_or(0)
                        })
                        .unwrap_or(0)
                } else {
                    0
                };
                self.scroll
                    .keep_column_visible(cursor_column, content_width, width);
                let safe_horizontal_offset = self.current_edit_buffer().map(|buffer| {
                    display_boundary_at_or_after(
                        buffer.current_line(),
                        self.scroll.horizontal_offset(),
                        TAB_WIDTH,
                    )
                });
                if let Some(offset) = safe_horizontal_offset {
                    self.scroll.set_horizontal_offset(offset);
                }
                let mut scrollbar_state = self.scroll.scrollbar_state(line_count, height);
                let lines = self.lines();
                frame.render_widget(
                    Paragraph::new(lines)
                        .scroll((
                            terminal_offset(self.scroll.offset()),
                            terminal_offset(self.scroll.horizontal_offset()),
                        ))
                        .block(Block::bordered().title("jjc diff")),
                    rows[0],
                );
                frame.render_stateful_widget(
                    Scrollbar::new(ScrollbarOrientation::VerticalRight),
                    scrollbar_area(rows[0]),
                    &mut scrollbar_state,
                );
                frame.render_widget(Paragraph::new(self.status()), rows[1]);
                if self.mode == Mode::Edit && self.current_edit_buffer().is_some() {
                    let x = rows[0].x + 1 + self.scroll.visible_column(cursor_column, width) as u16;
                    let y = rows[0].y
                        + 1
                        + self.scroll.visible_line(self.cursor_line_index(), height) as u16;
                    frame.set_cursor_position((x, y));
                }
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

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> io::Result<bool> {
        if self.mode == Mode::Edit {
            self.handle_edit_key(key);
            return Ok(false);
        }

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_up(),
            KeyCode::Char(']') => self.move_next_file(),
            KeyCode::Char('[') => self.move_prev_file(),
            KeyCode::Char('n') => self.move_line_down(),
            KeyCode::Char('p') => self.move_line_up(),
            KeyCode::PageDown => self.move_line_by(10),
            KeyCode::PageUp => self.move_line_by(-10),
            KeyCode::Char(' ') => self.toggle_current(),
            KeyCode::Char('x') => self.toggle_current_line(),
            KeyCode::Char('S') => self.select_current_file(true),
            KeyCode::Char('D') => self.select_current_file(false),
            KeyCode::Char('f') => self.toggle_current_function(),
            KeyCode::Char('e') => self.enter_edit_mode(),
            KeyCode::Char('u') => self.undo(),
            KeyCode::Char('r') => self.redo(),
            KeyCode::Char('w') => {
                self.write_output()?;
                return Ok(true);
            }
            KeyCode::Char('q') => {
                return Err(io::Error::new(io::ErrorKind::Interrupted, "diff canceled"));
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_edit_key(&mut self, key: crossterm::event::KeyEvent) {
        if self.edit_vim.mode() == VimMode::Normal && key.code == KeyCode::Esc {
            self.mode = Mode::Select;
            return;
        }
        let Some(entry) = self.entries.get(self.cursor) else {
            self.mode = Mode::Select;
            return;
        };
        let Some(buffer) = self.files[entry.file].manual_output.as_mut() else {
            self.mode = Mode::Select;
            return;
        };
        self.edit_vim.handle_key(buffer, key);
    }

    fn lines(&mut self) -> Vec<Line<'static>> {
        if self.files.is_empty() {
            return vec![Line::from("no changed files")];
        }

        if self.mode == Mode::Edit {
            let Some(entry) = self.entries.get(self.cursor) else {
                return vec![Line::from("no editable file")];
            };
            let file = &mut self.files[entry.file];
            let mut lines = vec![Line::from(Span::styled(
                format!("editing {}", file.path.display()),
                Style::new().add_modifier(Modifier::BOLD),
            ))];
            if let Some(buffer) = &file.manual_output {
                lines.extend(
                    file.manual_cache
                        .lines(&file.path, buffer.lines(), &self.config),
                );
            }
            return lines;
        }

        let mut lines = Vec::new();
        let mut entry_index = 0;
        for (file_index, file) in self.files.iter_mut().enumerate() {
            lines.push(Line::from(Span::styled(
                format!("{}  {}", file.path.display(), file.selection_summary()),
                Style::new().add_modifier(Modifier::BOLD),
            )));
            if let Some(reason) = &file.unsupported {
                lines.push(Line::from(Span::styled(
                    format!("  unsupported: {reason}"),
                    Style::new().add_modifier(Modifier::DIM),
                )));
                continue;
            }
            let left_display_lines = display_lines(&file.left_lines);
            let right_display_lines = display_lines(&file.right_lines);
            let left_styled = file
                .left_cache
                .lines(&file.path, &left_display_lines, &self.config);
            let right_styled =
                file.right_cache
                    .lines(&file.path, &right_display_lines, &self.config);
            for (hunk_index, hunk) in file.hunks.iter().enumerate() {
                let marker = if hunk.selected { "[x]" } else { "[ ]" };
                let prefix = if self
                    .entries
                    .get(self.cursor)
                    .is_some_and(|entry| entry.file == file_index && entry.hunk == hunk_index)
                {
                    ">"
                } else {
                    " "
                };
                lines.push(Line::from(format!("{prefix} {marker} {}", hunk.summary)));
                if self
                    .entries
                    .get(self.cursor)
                    .is_some_and(|entry| entry.file == file_index && entry.hunk == hunk_index)
                {
                    for (line_index, row) in hunk.rows.iter().enumerate() {
                        let cursor = if line_index == self.line_cursor {
                            ">"
                        } else {
                            " "
                        };
                        let marker = match row.kind {
                            DiffRowKind::Equal { .. } => "[=]",
                            _ if row.selected => "[x]",
                            _ => "[ ]",
                        };
                        let prefix = match row.kind {
                            DiffRowKind::Equal { .. } => format!("  {cursor} {marker}  "),
                            DiffRowKind::Delete { .. } => format!("  {cursor} {marker} -"),
                            DiffRowKind::Insert { .. } => format!("  {cursor} {marker} +"),
                        };
                        lines.push(match row.kind {
                            DiffRowKind::Equal { new_index } => {
                                line_with_prefix(&right_styled, new_index, prefix)
                            }
                            DiffRowKind::Delete { old_index } => {
                                line_with_prefix(&left_styled, old_index, prefix)
                            }
                            DiffRowKind::Insert { new_index } => {
                                line_with_prefix(&right_styled, new_index, prefix)
                            }
                        });
                    }
                }
                entry_index += 1;
            }
        }
        if entry_index == 0 {
            lines.push(Line::from("no text hunks"));
        }
        lines
    }

    fn status(&self) -> String {
        match self.mode {
            Mode::Select => {
                "j/k hunk  [/ ] file  n/p/PgUp/PgDn line  space/x toggle  S/D file  f function  e edit output  u/r undo redo  w write  q cancel".to_owned()
            }
            Mode::Edit
                if self
                    .current_edit_buffer()
                    .is_some_and(|buffer| is_large_content(buffer.lines())) =>
            {
                "EDIT OUTPUT  PLAIN LARGE FILE  syntax disabled  Esc select".to_owned()
            }
            Mode::Edit if self.edit_vim.mode() == VimMode::Normal => {
                "EDIT OUTPUT NORMAL  i/a/o insert  h/j/k/l/w/b/e move  x/dd delete  Esc select".to_owned()
            }
            Mode::Edit => "EDIT OUTPUT INSERT  Esc normal".to_owned(),
        }
    }

    fn cursor_mode(&self) -> VimMode {
        match self.mode {
            Mode::Select => VimMode::Normal,
            Mode::Edit => self.edit_vim.mode(),
        }
    }

    fn move_down(&mut self) {
        if !self.entries.is_empty() {
            self.cursor = (self.cursor + 1).min(self.entries.len() - 1);
            self.line_cursor = 0;
        }
    }

    fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
        self.line_cursor = 0;
    }

    fn move_next_file(&mut self) {
        self.move_file(1);
    }

    fn move_prev_file(&mut self) {
        self.move_file(-1);
    }

    fn move_file(&mut self, delta: isize) {
        let Some(current_file) = self.current_file_index() else {
            return;
        };
        let target_file = current_file
            .saturating_add_signed(delta)
            .min(self.files.len() - 1);
        if let Some((index, _)) = self
            .entries
            .iter()
            .enumerate()
            .find(|(_, entry)| entry.file == target_file)
        {
            self.cursor = index;
            self.line_cursor = 0;
        }
    }

    fn move_line_down(&mut self) {
        if let Some(hunk) = self.current_hunk() {
            self.line_cursor = (self.line_cursor + 1).min(hunk.rows.len().saturating_sub(1));
        }
    }

    fn move_line_up(&mut self) {
        self.line_cursor = self.line_cursor.saturating_sub(1);
    }

    fn move_line_by(&mut self, delta: isize) {
        if let Some(hunk) = self.current_hunk() {
            self.line_cursor = self
                .line_cursor
                .saturating_add_signed(delta)
                .min(hunk.rows.len().saturating_sub(1));
        }
    }

    fn toggle_current(&mut self) {
        if let Some(entry) = self.entries.get(self.cursor).copied() {
            self.push_undo();
            let file = &mut self.files[entry.file];
            file.manual_output = None;
            let hunk = &mut file.hunks[entry.hunk];
            hunk.set_selected(!hunk.selected);
        }
    }

    fn toggle_current_line(&mut self) {
        if let Some(entry) = self.entries.get(self.cursor).copied() {
            self.push_undo();
            let file = &mut self.files[entry.file];
            file.manual_output = None;
            let hunk = &mut file.hunks[entry.hunk];
            hunk.toggle_line(self.line_cursor);
        }
    }

    fn toggle_current_function(&mut self) {
        let Some(entry) = self.entries.get(self.cursor).copied() else {
            return;
        };
        let Some(function) = self.files[entry.file].hunks[entry.hunk].function.clone() else {
            self.toggle_current();
            return;
        };
        self.push_undo();
        let selected = !self.files[entry.file].hunks[entry.hunk].selected;
        let file = &mut self.files[entry.file];
        file.manual_output = None;
        for hunk in &mut file.hunks {
            if hunk.function.as_deref() == Some(function.as_str()) {
                hunk.set_selected(selected);
            }
        }
    }

    fn select_current_file(&mut self, selected: bool) {
        let Some(file_index) = self.current_file_index() else {
            return;
        };
        self.push_undo();
        let file = &mut self.files[file_index];
        file.manual_output = None;
        for hunk in &mut file.hunks {
            hunk.set_selected(selected);
        }
    }

    fn enter_edit_mode(&mut self) {
        let Some(entry) = self.entries.get(self.cursor).copied() else {
            return;
        };
        let file = &mut self.files[entry.file];
        if file.unsupported.is_some() || !matches!(file.kind, DiffFileKind::ModifiedText) {
            return;
        }
        if file.manual_output.is_none() {
            file.manual_output = Some(TextBuffer::from_text(&file.render_selection()));
        }
        self.edit_vim.set_normal();
        self.mode = Mode::Edit;
    }

    fn push_undo(&mut self) {
        self.undo.push(self.snapshot());
        self.redo.clear();
    }

    fn undo(&mut self) {
        let Some(snapshot) = self.undo.pop() else {
            return;
        };
        self.redo.push(self.snapshot());
        self.restore(snapshot);
    }

    fn redo(&mut self) {
        let Some(snapshot) = self.redo.pop() else {
            return;
        };
        self.undo.push(self.snapshot());
        self.restore(snapshot);
    }

    fn snapshot(&self) -> SelectionSnapshot {
        SelectionSnapshot(
            self.files
                .iter()
                .map(|file| {
                    file.hunks
                        .iter()
                        .map(Hunk::selected_lines)
                        .collect::<Vec<_>>()
                })
                .collect(),
        )
    }

    fn restore(&mut self, snapshot: SelectionSnapshot) {
        for (file, file_snapshot) in self.files.iter_mut().zip(snapshot.0) {
            for (hunk, hunk_snapshot) in file.hunks.iter_mut().zip(file_snapshot) {
                hunk.restore_selected_lines(&hunk_snapshot);
            }
        }
    }

    fn current_hunk(&self) -> Option<&Hunk> {
        let entry = self.entries.get(self.cursor)?;
        self.files.get(entry.file)?.hunks.get(entry.hunk)
    }

    fn current_file_index(&self) -> Option<usize> {
        self.entries.get(self.cursor).map(|entry| entry.file)
    }

    fn cursor_line_index(&self) -> usize {
        if self.mode == Mode::Edit {
            return self
                .current_edit_buffer()
                .map(|buffer| buffer.cursor_y() + 1)
                .unwrap_or(0);
        }

        let current = self.entries.get(self.cursor);
        let mut index = 0;
        for (file_index, file) in self.files.iter().enumerate() {
            index += 1;
            if file.unsupported.is_some() {
                index += 1;
                continue;
            }
            for (hunk_index, hunk) in file.hunks.iter().enumerate() {
                let is_current = current
                    .is_some_and(|entry| entry.file == file_index && entry.hunk == hunk_index);
                if is_current {
                    return index + 1 + self.line_cursor.min(hunk.rows.len().saturating_sub(1));
                }
                index += 1;
            }
        }
        0
    }

    fn line_count(&self) -> usize {
        if self.files.is_empty() {
            return 1;
        }
        if self.mode == Mode::Edit {
            let Some(entry) = self.entries.get(self.cursor) else {
                return 1;
            };
            return 1 + self.files[entry.file]
                .manual_output
                .as_ref()
                .map(|buffer| buffer.lines().len())
                .unwrap_or(0);
        }

        let current = self.entries.get(self.cursor);
        let mut count = 0;
        let mut hunk_count = 0;
        for (file_index, file) in self.files.iter().enumerate() {
            count += 1;
            if file.unsupported.is_some() {
                count += 1;
                continue;
            }
            for (hunk_index, hunk) in file.hunks.iter().enumerate() {
                count += 1;
                hunk_count += 1;
                if current.is_some_and(|entry| entry.file == file_index && entry.hunk == hunk_index)
                {
                    count += hunk.rows.len();
                }
            }
        }
        if hunk_count == 0 {
            count += 1;
        }
        count
    }

    fn current_edit_buffer(&self) -> Option<&TextBuffer> {
        let entry = self.entries.get(self.cursor)?;
        self.files.get(entry.file)?.manual_output.as_ref()
    }

    #[cfg(test)]
    fn current_edit_buffer_mut(&mut self) -> Option<&mut TextBuffer> {
        let entry = self.entries.get(self.cursor)?;
        self.files.get_mut(entry.file)?.manual_output.as_mut()
    }

    fn write_output(&self) -> io::Result<()> {
        for file in &self.files {
            if let Some(reason) = &file.unsupported {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unsupported diff file {}: {reason}", file.path.display()),
                ));
            }
        }

        let entries = self
            .files
            .iter()
            .map(DiffFile::materialize)
            .collect::<io::Result<Vec<_>>>()?;
        for entry in &entries {
            validate_output_entry(entry)?;
        }
        for file in &self.files {
            validate_output_path(&self.output, &file.path)?;
        }
        for (file, entry) in self.files.iter().zip(entries) {
            write_tree_entry(&self.output.join(&file.path), entry)?;
        }
        Ok(())
    }
}

impl DiffFile {
    fn load(path: PathBuf, left_entry: TreeEntry, right_entry: TreeEntry) -> io::Result<Self> {
        match (left_entry.kind(), right_entry.kind()) {
            (TreeEntryKind::File, TreeEntryKind::File) => {
                Self::load_modified(path, left_entry, right_entry)
            }
            (TreeEntryKind::Missing, TreeEntryKind::File) => {
                Self::load_added(path, left_entry, right_entry)
            }
            (TreeEntryKind::File, TreeEntryKind::Missing) => {
                Self::load_deleted(path, left_entry, right_entry)
            }
            (TreeEntryKind::Symlink, TreeEntryKind::Symlink)
            | (TreeEntryKind::Missing, TreeEntryKind::Symlink)
            | (TreeEntryKind::Symlink, TreeEntryKind::Missing)
            | (TreeEntryKind::File, TreeEntryKind::Symlink)
            | (TreeEntryKind::Symlink, TreeEntryKind::File) => {
                Ok(Self::entry_change(path, left_entry, right_entry))
            }
            _ => {
                let reason = format!(
                    "tree entry change {} -> {} is not supported",
                    left_entry.kind().name(),
                    right_entry.kind().name()
                );
                Ok(Self::unsupported(path, left_entry, right_entry, reason))
            }
        }
    }

    fn load_modified(
        path: PathBuf,
        left_entry: TreeEntry,
        right_entry: TreeEntry,
    ) -> io::Result<Self> {
        let (left_bytes, left_executable) = left_entry.file().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "left entry is not a file")
        })?;
        let (right_bytes, right_executable) = right_entry.file().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "right entry is not a file")
        })?;
        let mut metadata_hunks = Vec::new();
        if left_executable != right_executable {
            metadata_hunks.push(executable_hunk(left_executable, right_executable));
        }

        match (str::from_utf8(left_bytes), str::from_utf8(right_bytes)) {
            (Ok(left), Ok(right)) => {
                let left_lines = split_keep_newline(left);
                let right_lines = split_keep_newline(right);
                let mut hunks = if left_bytes == right_bytes {
                    Vec::new()
                } else {
                    hunks(&path, left, right)
                };
                hunks.extend(metadata_hunks);
                Ok(Self::new(
                    path,
                    DiffFileKind::ModifiedText,
                    left_entry,
                    right_entry,
                    left_lines,
                    right_lines,
                    hunks,
                ))
            }
            _ => {
                let mut hunks = if left_bytes == right_bytes {
                    Vec::new()
                } else {
                    vec![whole_file_hunk("binary file", HunkRole::Content)]
                };
                hunks.extend(metadata_hunks);
                Ok(Self::new(
                    path,
                    DiffFileKind::ModifiedBinary,
                    left_entry,
                    right_entry,
                    Vec::new(),
                    Vec::new(),
                    hunks,
                ))
            }
        }
    }

    fn load_added(
        path: PathBuf,
        left_entry: TreeEntry,
        right_entry: TreeEntry,
    ) -> io::Result<Self> {
        let (right_bytes, _) = right_entry.file().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "added entry is not a file")
        })?;
        let (kind, right_lines, summary) = match str::from_utf8(right_bytes) {
            Ok(right) => (
                DiffFileKind::AddedText,
                split_keep_newline(right),
                "added file",
            ),
            Err(_) => (DiffFileKind::AddedBinary, Vec::new(), "added binary file"),
        };
        Ok(Self::new(
            path,
            kind,
            left_entry,
            right_entry,
            Vec::new(),
            right_lines,
            vec![whole_file_hunk(summary, HunkRole::Entry)],
        ))
    }

    fn load_deleted(
        path: PathBuf,
        left_entry: TreeEntry,
        right_entry: TreeEntry,
    ) -> io::Result<Self> {
        let (left_bytes, _) = left_entry.file().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "deleted entry is not a file")
        })?;
        let (kind, left_lines, summary) = match str::from_utf8(left_bytes) {
            Ok(left) => (
                DiffFileKind::DeletedText,
                split_keep_newline(left),
                "deleted file",
            ),
            Err(_) => (
                DiffFileKind::DeletedBinary,
                Vec::new(),
                "deleted binary file",
            ),
        };
        Ok(Self::new(
            path,
            kind,
            left_entry,
            right_entry,
            left_lines,
            Vec::new(),
            vec![whole_file_hunk(summary, HunkRole::Entry)],
        ))
    }

    fn new(
        path: PathBuf,
        kind: DiffFileKind,
        left_entry: TreeEntry,
        right_entry: TreeEntry,
        left_lines: Vec<String>,
        right_lines: Vec<String>,
        hunks: Vec<Hunk>,
    ) -> Self {
        Self {
            path,
            kind,
            left_entry,
            right_entry,
            left_lines,
            right_lines,
            hunks,
            manual_output: None,
            left_cache: StyledTextCache::default(),
            right_cache: StyledTextCache::default(),
            manual_cache: StyledTextCache::default(),
            unsupported: None,
        }
    }

    fn entry_change(path: PathBuf, left_entry: TreeEntry, right_entry: TreeEntry) -> Self {
        let summary = match (left_entry.kind(), right_entry.kind()) {
            (TreeEntryKind::Symlink, TreeEntryKind::Symlink) => "symlink target changed".to_owned(),
            (TreeEntryKind::Missing, TreeEntryKind::Symlink) => "added symlink".to_owned(),
            (TreeEntryKind::Symlink, TreeEntryKind::Missing) => "deleted symlink".to_owned(),
            (left, right) => format!("file type changed: {} -> {}", left.name(), right.name()),
        };
        Self::new(
            path,
            DiffFileKind::EntryChange,
            left_entry,
            right_entry,
            Vec::new(),
            Vec::new(),
            vec![whole_file_hunk(summary, HunkRole::Entry)],
        )
    }

    fn unsupported(
        path: PathBuf,
        left_entry: TreeEntry,
        right_entry: TreeEntry,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            path,
            kind: DiffFileKind::ModifiedText,
            left_entry,
            right_entry,
            left_lines: Vec::new(),
            right_lines: Vec::new(),
            hunks: Vec::new(),
            manual_output: None,
            left_cache: StyledTextCache::default(),
            right_cache: StyledTextCache::default(),
            manual_cache: StyledTextCache::default(),
            unsupported: Some(reason.into()),
        }
    }

    fn materialize(&self) -> io::Result<TreeEntry> {
        match self.kind {
            DiffFileKind::ModifiedText => Ok(TreeEntry::File {
                contents: self
                    .manual_output
                    .as_ref()
                    .map(|output| output.to_text())
                    .unwrap_or_else(|| self.render_selection())
                    .into_bytes(),
                executable: self.selected_executable()?,
            }),
            DiffFileKind::ModifiedBinary => {
                let (left, _) = self.left_entry.file().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "left entry is not a file")
                })?;
                let (right, _) = self.right_entry.file().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "right entry is not a file")
                })?;
                let contents = if self.role_selected(HunkRole::Content) {
                    right.to_vec()
                } else {
                    left.to_vec()
                };
                Ok(TreeEntry::File {
                    contents,
                    executable: self.selected_executable()?,
                })
            }
            DiffFileKind::AddedText
            | DiffFileKind::DeletedText
            | DiffFileKind::AddedBinary
            | DiffFileKind::DeletedBinary
            | DiffFileKind::EntryChange => {
                if self.role_selected(HunkRole::Entry) {
                    Ok(self.right_entry.clone())
                } else {
                    Ok(self.left_entry.clone())
                }
            }
        }
    }

    #[cfg(test)]
    fn render(&self) -> Option<Vec<u8>> {
        match self.materialize().unwrap() {
            TreeEntry::File { contents, .. } => Some(contents),
            _ => None,
        }
    }

    fn role_selected(&self, role: HunkRole) -> bool {
        self.hunks
            .iter()
            .find(|hunk| hunk.role == role)
            .is_none_or(|hunk| hunk.selected)
    }

    fn selected_executable(&self) -> io::Result<bool> {
        let (_, left) = self.left_entry.file().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "left entry is not a file")
        })?;
        let (_, right) = self.right_entry.file().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "right entry is not a file")
        })?;
        Ok(if self.role_selected(HunkRole::Executable) {
            right
        } else {
            left
        })
    }

    fn render_selection(&self) -> String {
        match self.kind {
            DiffFileKind::ModifiedText => {
                render_hunks(&self.left_lines, &self.right_lines, &self.hunks)
            }
            DiffFileKind::AddedText => {
                if self.role_selected(HunkRole::Entry) {
                    self.right_lines.concat()
                } else {
                    String::new()
                }
            }
            DiffFileKind::DeletedText => {
                if self.role_selected(HunkRole::Entry) {
                    String::new()
                } else {
                    self.left_lines.concat()
                }
            }
            _ => String::new(),
        }
    }

    fn selection_summary(&self) -> String {
        if self.unsupported.is_some() {
            return "[unsupported]".to_owned();
        }
        let total = self.hunks.len();
        let selected = self.hunks.iter().filter(|hunk| hunk.selected).count();
        format!("[{selected}/{total} hunks selected]")
    }
}

impl TreeEntry {
    fn kind(&self) -> TreeEntryKind {
        match self {
            Self::Missing => TreeEntryKind::Missing,
            Self::File { .. } => TreeEntryKind::File,
            Self::Symlink { .. } => TreeEntryKind::Symlink,
            Self::Directory => TreeEntryKind::Directory,
            Self::Other => TreeEntryKind::Other,
        }
    }

    fn file(&self) -> Option<(&[u8], bool)> {
        match self {
            Self::File {
                contents,
                executable,
            } => Some((contents, *executable)),
            _ => None,
        }
    }
}

impl TreeEntryKind {
    fn name(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::File => "normal file",
            Self::Symlink => "symlink",
            Self::Directory => "directory",
            Self::Other => "special file",
        }
    }
}

fn read_tree_entry(path: &Path) -> io::Result<TreeEntry> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err)
            if matches!(
                err.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            return Ok(TreeEntry::Missing);
        }
        Err(err) => return Err(err),
    };
    let file_type = metadata.file_type();
    if file_type.is_file() {
        Ok(TreeEntry::File {
            contents: fs::read(path)?,
            executable: metadata_is_executable(&metadata),
        })
    } else if file_type.is_symlink() {
        Ok(TreeEntry::Symlink {
            target: fs::read_link(path)?,
        })
    } else if file_type.is_dir() {
        Ok(TreeEntry::Directory)
    } else {
        Ok(TreeEntry::Other)
    }
}

#[cfg(unix)]
fn metadata_is_executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;

    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn metadata_is_executable(_metadata: &fs::Metadata) -> bool {
    false
}

fn validate_output_entry(entry: &TreeEntry) -> io::Result<()> {
    match entry {
        TreeEntry::Directory | TreeEntry::Other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cannot materialize an unsupported tree entry",
        )),
        #[cfg(not(unix))]
        TreeEntry::Symlink { .. } => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "symlink diff output is only supported on Unix",
        )),
        _ => Ok(()),
    }
}

fn validate_output_path(root: &Path, relative: &Path) -> io::Result<()> {
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unsafe diff output path: {}", relative.display()),
        ));
    }

    let root_metadata = fs::symlink_metadata(root)?;
    if !root_metadata.file_type().is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("diff output root is not a directory: {}", root.display()),
        ));
    }

    let mut current = root.to_owned();
    if let Some(parent) = relative.parent() {
        for component in parent.components() {
            let Component::Normal(component) = component else {
                continue;
            };
            current.push(component);
            match fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.file_type().is_dir() => {}
                Ok(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "refusing to traverse non-directory diff output ancestor: {}",
                            current.display()
                        ),
                    ));
                }
                Err(err) if err.kind() == io::ErrorKind::NotFound => break,
                Err(err) => return Err(err),
            }
        }
    }

    let target = root.join(relative);
    match fs::symlink_metadata(&target) {
        Ok(metadata) if metadata.file_type().is_dir() => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "refusing to replace directory diff output entry: {}",
                target.display()
            ),
        )),
        Ok(_) => Ok(()),
        Err(err)
            if matches!(
                err.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            Ok(())
        }
        Err(err) => Err(err),
    }
}

fn write_tree_entry(path: &Path, entry: TreeEntry) -> io::Result<()> {
    match entry {
        TreeEntry::Missing => remove_output_entry(path),
        TreeEntry::File {
            contents,
            executable,
        } => {
            remove_output_entry(path)?;
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, contents)?;
            set_executable(path, executable)
        }
        TreeEntry::Symlink { target } => {
            remove_output_entry(path)?;
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            write_symlink(&target, path)
        }
        TreeEntry::Directory | TreeEntry::Other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("cannot write unsupported tree entry: {}", path.display()),
        )),
    }
}

fn remove_output_entry(path: &Path) -> io::Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };
    let file_type = metadata.file_type();
    if file_type.is_file() || file_type.is_symlink() {
        fs::remove_file(path)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "refusing to replace unsupported output entry: {}",
                path.display()
            ),
        ))
    }
}

#[cfg(unix)]
fn set_executable(path: &Path, executable: bool) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::symlink_metadata(path)?.permissions();
    let mode = permissions.mode();
    permissions.set_mode(if executable {
        mode | 0o111
    } else {
        mode & !0o111
    });
    fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn set_executable(_path: &Path, _executable: bool) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn write_symlink(target: &Path, path: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, path)
}

#[cfg(not(unix))]
fn write_symlink(_target: &Path, _path: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "symlink diff output is only supported on Unix",
    ))
}

fn display_lines(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .map(|line| line.trim_end().to_owned())
        .collect()
}

fn line_with_prefix(
    lines: &[Line<'static>],
    index: usize,
    prefix: impl Into<String>,
) -> Line<'static> {
    let mut line = lines.get(index).cloned().unwrap_or_default();
    line.spans.insert(0, Span::raw(prefix.into()));
    line
}

fn changed_paths(left: &Path, right: &Path) -> io::Result<Vec<ChangedPath>> {
    let mut paths = BTreeSet::new();
    collect_paths(left, left, &mut paths)?;
    collect_paths(right, right, &mut paths)?;
    let mut changed = Vec::new();
    for path in paths {
        let left_entry = read_tree_entry(&left.join(&path))?;
        let right_entry = read_tree_entry(&right.join(&path))?;
        if left_entry != right_entry {
            changed.push(ChangedPath {
                path,
                left: left_entry,
                right: right_entry,
            });
        }
    }
    Ok(changed)
}

fn collect_paths(root: &Path, dir: &Path, paths: &mut BTreeSet<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_dir() {
            collect_paths(root, &path, paths)?;
        } else if path
            .file_name()
            .is_some_and(|name| name != "JJ-INSTRUCTIONS")
        {
            let relative = path.strip_prefix(root).map_err(io::Error::other)?;
            paths.insert(relative.to_owned());
        }
    }
    Ok(())
}

fn hunks(path: &Path, left: &str, right: &str) -> Vec<Hunk> {
    let diff = TextDiff::configure()
        .algorithm(Algorithm::Patience)
        .diff_lines(left, right);
    diff.grouped_ops(3)
        .into_iter()
        .filter_map(|ops| hunk_from_ops(path, right, &ops))
        .collect()
}

fn hunk_from_ops(path: &Path, right: &str, ops: &[DiffOp]) -> Option<Hunk> {
    let mut old_start = usize::MAX;
    let mut old_end = 0;
    let mut new_start = usize::MAX;
    let mut new_end = 0;

    for op in ops {
        let (_, old, new) = op.as_tag_tuple();
        old_start = old_start.min(old.start);
        old_end = old_end.max(old.end);
        new_start = new_start.min(new.start);
        new_end = new_end.max(new.end);
    }

    if old_start == usize::MAX {
        return None;
    }

    let mut rows = Vec::new();
    let mut next_group = 0;
    for op in ops {
        let (tag, old, new) = op.as_tag_tuple();
        match tag {
            DiffTag::Equal => {
                for new_index in new {
                    rows.push(DiffRow {
                        kind: DiffRowKind::Equal { new_index },
                        selected: true,
                        group: None,
                    });
                }
            }
            DiffTag::Delete => {
                for old_index in old {
                    rows.push(DiffRow {
                        kind: DiffRowKind::Delete { old_index },
                        selected: true,
                        group: Some(next_group),
                    });
                    next_group += 1;
                }
            }
            DiffTag::Insert => {
                for new_index in new {
                    rows.push(DiffRow {
                        kind: DiffRowKind::Insert { new_index },
                        selected: true,
                        group: Some(next_group),
                    });
                    next_group += 1;
                }
            }
            DiffTag::Replace => {
                let len = old.len().max(new.len());
                for offset in 0..len {
                    let group = Some(next_group);
                    next_group += 1;
                    if offset < old.len() {
                        rows.push(DiffRow {
                            kind: DiffRowKind::Delete {
                                old_index: old.start + offset,
                            },
                            selected: true,
                            group,
                        });
                    }
                    if offset < new.len() {
                        rows.push(DiffRow {
                            kind: DiffRowKind::Insert {
                                new_index: new.start + offset,
                            },
                            selected: true,
                            group,
                        });
                    }
                }
            }
        }
    }

    if rows
        .iter()
        .all(|row| matches!(row.kind, DiffRowKind::Equal { .. }))
    {
        return None;
    }

    let function = syntax::function_for_line(path, right, new_start + 1);
    let function_suffix = function
        .as_ref()
        .map(|name| format!(" fn {name}"))
        .unwrap_or_default();

    Some(Hunk {
        role: HunkRole::Content,
        old_start,
        old_end,
        new_start,
        new_end,
        selected: true,
        rows,
        function,
        summary: format!(
            "-{} +{}{}",
            range_summary(old_start, old_end),
            range_summary(new_start, new_end),
            function_suffix
        ),
    })
}

fn range_summary(start: usize, end: usize) -> String {
    if start == end {
        format!("{start},0")
    } else {
        format!("{},{}", start + 1, end - start)
    }
}

fn whole_file_hunk(summary: impl Into<String>, role: HunkRole) -> Hunk {
    Hunk {
        role,
        old_start: 0,
        old_end: 0,
        new_start: 0,
        new_end: 0,
        selected: true,
        rows: Vec::new(),
        function: None,
        summary: summary.into(),
    }
}

fn executable_hunk(left: bool, right: bool) -> Hunk {
    let left = if left { "+x" } else { "-x" };
    let right = if right { "+x" } else { "-x" };
    whole_file_hunk(
        format!("executable bit changed: {left} -> {right}"),
        HunkRole::Executable,
    )
}

fn render_hunks(left: &[String], right: &[String], hunks: &[Hunk]) -> String {
    let mut output = Vec::new();
    let mut new_cursor = 0;
    for hunk in hunks {
        if hunk.role != HunkRole::Content {
            continue;
        }
        output.extend_from_slice(&right[new_cursor..hunk.new_start]);
        if hunk.all_selected() {
            output.extend_from_slice(&right[hunk.new_start..hunk.new_end]);
        } else if !hunk.selected {
            output.extend_from_slice(&left[hunk.old_start..hunk.old_end]);
        } else {
            output.extend(render_partial_hunk(left, right, hunk));
        }
        new_cursor = hunk.new_end;
    }
    output.extend_from_slice(&right[new_cursor..]);
    output.concat()
}

fn render_partial_hunk(left: &[String], right: &[String], hunk: &Hunk) -> Vec<String> {
    let mut output = Vec::new();
    for row in &hunk.rows {
        match row.kind {
            DiffRowKind::Equal { new_index } => output.push(right[new_index].clone()),
            DiffRowKind::Delete { old_index } if !row.selected => {
                output.push(left[old_index].clone());
            }
            DiffRowKind::Insert { new_index } if row.selected => {
                output.push(right[new_index].clone());
            }
            _ => {}
        }
    }
    output
}

impl DiffRowKind {
    fn changed(self) -> bool {
        !matches!(self, Self::Equal { .. })
    }
}

impl Hunk {
    fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
        for row in &mut self.rows {
            if row.kind.changed() {
                row.selected = selected;
            }
        }
    }

    fn toggle_line(&mut self, index: usize) {
        let Some(group) = self.rows.get(index).and_then(|row| row.group) else {
            return;
        };
        let selected = !self.rows[index].selected;
        for row in &mut self.rows {
            if row.group == Some(group) {
                row.selected = selected;
            }
        }
        self.selected = self.any_selected();
    }

    fn selected_lines(&self) -> Vec<bool> {
        if self.rows.is_empty() {
            return vec![self.selected];
        }
        self.rows
            .iter()
            .map(|row| {
                if row.kind.changed() {
                    row.selected
                } else {
                    true
                }
            })
            .collect()
    }

    fn restore_selected_lines(&mut self, selected_lines: &[bool]) {
        if self.rows.is_empty() {
            if let Some(selected) = selected_lines.first() {
                self.selected = *selected;
            }
            return;
        }
        for (row, selected) in self.rows.iter_mut().zip(selected_lines) {
            if row.kind.changed() {
                row.selected = *selected;
            }
        }
        self.selected = self.any_selected();
    }

    fn any_selected(&self) -> bool {
        self.rows
            .iter()
            .any(|row| row.kind.changed() && row.selected)
    }

    fn all_selected(&self) -> bool {
        self.selected
            && self
                .rows
                .iter()
                .all(|row| !row.kind.changed() || row.selected)
    }
}

fn split_keep_newline(text: &str) -> Vec<String> {
    text.split_inclusive('\n').map(str::to_owned).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use std::time::SystemTime;
    use std::time::UNIX_EPOCH;

    #[test]
    fn selected_hunks_render_right_side() {
        let left = "a\nold\nc\n";
        let right = "a\nnew\nc\n";
        let left_lines = split_keep_newline(left);
        let right_lines = split_keep_newline(right);
        let hunks = hunks(Path::new("file.txt"), left, right);

        assert_eq!(render_hunks(&left_lines, &right_lines, &hunks), right);
    }

    #[test]
    fn unselected_hunk_restores_left_side() {
        let left = "a\nold\nc\n";
        let right = "a\nnew\nc\n";
        let left_lines = split_keep_newline(left);
        let right_lines = split_keep_newline(right);
        let mut hunks = hunks(Path::new("file.txt"), left, right);
        hunks[0].selected = false;

        assert_eq!(render_hunks(&left_lines, &right_lines, &hunks), left);
    }

    #[test]
    fn inserted_hunk_can_be_unselected() {
        let left = "a\nc\n";
        let right = "a\nb\nc\n";
        let left_lines = split_keep_newline(left);
        let right_lines = split_keep_newline(right);
        let mut hunks = hunks(Path::new("file.txt"), left, right);
        hunks[0].selected = false;

        assert_eq!(render_hunks(&left_lines, &right_lines, &hunks), left);
    }

    #[test]
    fn inserted_line_can_be_unselected_inside_hunk() {
        let left = "a\nc\n";
        let right = "a\nb\nc\n";
        let left_lines = split_keep_newline(left);
        let right_lines = split_keep_newline(right);
        let mut hunks = hunks(Path::new("file.txt"), left, right);
        hunks[0].toggle_line(1);

        assert_eq!(render_hunks(&left_lines, &right_lines, &hunks), left);
    }

    #[test]
    fn deleted_line_is_an_explicit_diff_row() {
        let left = "a\nold\nc\n";
        let right = "a\nc\n";
        let left_lines = split_keep_newline(left);
        let right_lines = split_keep_newline(right);
        let mut hunks = hunks(Path::new("file.txt"), left, right);

        assert_eq!(
            hunks[0].rows.iter().map(|row| row.kind).collect::<Vec<_>>(),
            vec![
                DiffRowKind::Equal { new_index: 0 },
                DiffRowKind::Delete { old_index: 1 },
                DiffRowKind::Equal { new_index: 1 },
            ]
        );
        hunks[0].toggle_line(1);

        assert_eq!(render_hunks(&left_lines, &right_lines, &hunks), left);
    }

    #[test]
    fn replacement_line_can_fall_back_to_old_line() {
        let left = "a\nold\nc\n";
        let right = "a\nnew\nc\n";
        let left_lines = split_keep_newline(left);
        let right_lines = split_keep_newline(right);
        let mut hunks = hunks(Path::new("file.txt"), left, right);

        assert_eq!(
            hunks[0].rows.iter().map(|row| row.kind).collect::<Vec<_>>(),
            vec![
                DiffRowKind::Equal { new_index: 0 },
                DiffRowKind::Delete { old_index: 1 },
                DiffRowKind::Insert { new_index: 1 },
                DiffRowKind::Equal { new_index: 2 },
            ]
        );
        assert_eq!(hunks[0].rows[1].group, hunks[0].rows[2].group);
        hunks[0].toggle_line(1);

        assert_eq!(
            render_hunks(&left_lines, &right_lines, &hunks),
            "a\nold\nc\n"
        );
    }

    #[test]
    fn write_output_applies_unselected_hunks() {
        let root = std::env::temp_dir().join(format!(
            "jjc-diff-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let left = root.join("left");
        let right = root.join("right");
        let output = root.join("output");
        fs::create_dir_all(&left).unwrap();
        fs::create_dir_all(&right).unwrap();
        fs::create_dir_all(&output).unwrap();
        fs::write(left.join("file.txt"), "a\nold\nc\n").unwrap();
        fs::write(right.join("file.txt"), "a\nnew\nc\n").unwrap();
        fs::write(output.join("file.txt"), "a\nnew\nc\n").unwrap();

        let mut app = DiffApp::open(left, right, output.clone()).unwrap();
        app.toggle_current();
        app.write_output().unwrap();

        assert_eq!(
            fs::read_to_string(output.join("file.txt")).unwrap(),
            "a\nold\nc\n"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn write_output_uses_manual_file_edit() {
        let root = std::env::temp_dir().join(format!(
            "jjc-diff-manual-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let left = root.join("left");
        let right = root.join("right");
        let output = root.join("output");
        fs::create_dir_all(&left).unwrap();
        fs::create_dir_all(&right).unwrap();
        fs::create_dir_all(&output).unwrap();
        fs::write(left.join("file.txt"), "a\nold\nc\n").unwrap();
        fs::write(right.join("file.txt"), "a\nnew\nc\n").unwrap();

        let mut app = DiffApp::open(left, right, output.clone()).unwrap();
        app.enter_edit_mode();
        app.current_edit_buffer_mut().unwrap().set_text("manual\n");
        app.write_output().unwrap();

        assert_eq!(
            fs::read_to_string(output.join("file.txt")).unwrap(),
            "manual\n"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn write_output_supports_added_text_file() {
        let root = std::env::temp_dir().join(format!(
            "jjc-diff-added-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let left = root.join("left");
        let right = root.join("right");
        let output = root.join("output");
        fs::create_dir_all(&left).unwrap();
        fs::create_dir_all(&right).unwrap();
        fs::create_dir_all(&output).unwrap();
        fs::write(right.join("file.txt"), "new\n").unwrap();

        let mut app = DiffApp::open(left.clone(), right.clone(), output.clone()).unwrap();
        app.write_output().unwrap();
        assert_eq!(
            fs::read_to_string(output.join("file.txt")).unwrap(),
            "new\n"
        );

        app.toggle_current();
        app.write_output().unwrap();
        assert!(!output.join("file.txt").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn write_output_supports_deleted_text_file() {
        let root = std::env::temp_dir().join(format!(
            "jjc-diff-deleted-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let left = root.join("left");
        let right = root.join("right");
        let output = root.join("output");
        fs::create_dir_all(&left).unwrap();
        fs::create_dir_all(&right).unwrap();
        fs::create_dir_all(&output).unwrap();
        fs::write(left.join("file.txt"), "old\n").unwrap();
        fs::write(output.join("file.txt"), "old\n").unwrap();

        let mut app = DiffApp::open(left.clone(), right.clone(), output.clone()).unwrap();
        app.write_output().unwrap();
        assert!(!output.join("file.txt").exists());

        app.toggle_current();
        app.write_output().unwrap();
        assert_eq!(
            fs::read_to_string(output.join("file.txt")).unwrap(),
            "old\n"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn write_output_supports_binary_file_choices() {
        let root = std::env::temp_dir().join(format!(
            "jjc-diff-binary-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let left = root.join("left");
        let right = root.join("right");
        let output = root.join("output");
        fs::create_dir_all(&left).unwrap();
        fs::create_dir_all(&right).unwrap();
        fs::create_dir_all(&output).unwrap();
        fs::write(left.join("file.bin"), [0, 1]).unwrap();
        fs::write(right.join("file.bin"), [0xff, 2]).unwrap();

        let mut app = DiffApp::open(left.clone(), right.clone(), output.clone()).unwrap();
        app.write_output().unwrap();
        assert_eq!(fs::read(output.join("file.bin")).unwrap(), vec![0xff, 2]);

        app.toggle_current();
        app.write_output().unwrap();
        assert_eq!(fs::read(output.join("file.bin")).unwrap(), vec![0, 1]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn file_navigation_and_file_selection_affect_only_current_file() {
        let root = std::env::temp_dir().join(format!(
            "jjc-diff-file-nav-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let left = root.join("left");
        let right = root.join("right");
        let output = root.join("output");
        fs::create_dir_all(&left).unwrap();
        fs::create_dir_all(&right).unwrap();
        fs::create_dir_all(&output).unwrap();
        fs::write(left.join("a.txt"), "old-a\n").unwrap();
        fs::write(right.join("a.txt"), "new-a\n").unwrap();
        fs::write(left.join("b.txt"), "old-b\n").unwrap();
        fs::write(right.join("b.txt"), "new-b\n").unwrap();

        let mut app = DiffApp::open(left, right, output.clone()).unwrap();
        app.move_next_file();
        app.select_current_file(false);
        app.write_output().unwrap();

        assert_eq!(fs::read_to_string(output.join("a.txt")).unwrap(), "new-a\n");
        assert_eq!(fs::read_to_string(output.join("b.txt")).unwrap(), "old-b\n");
        assert!(
            app.lines()
                .iter()
                .any(|line| line.to_string().contains("b.txt  [0/1 hunks selected]"))
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn write_output_rejects_unsupported_files() {
        let root = std::env::temp_dir().join(format!(
            "jjc-diff-unsupported-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let left = root.join("left");
        let right = root.join("right");
        let output = root.join("output");
        fs::create_dir_all(&left).unwrap();
        fs::create_dir_all(&right).unwrap();
        fs::create_dir_all(&output).unwrap();
        fs::create_dir(left.join("path")).unwrap();
        fs::write(right.join("path"), "file\n").unwrap();

        let app = DiffApp::open(left, right, output).unwrap();
        let err = app.write_output().unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("path"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn q_cancels_diff_edit() {
        let root = std::env::temp_dir().join(format!(
            "jjc-diff-cancel-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let left = root.join("left");
        let right = root.join("right");
        let output = root.join("output");
        fs::create_dir_all(&left).unwrap();
        fs::create_dir_all(&right).unwrap();
        fs::create_dir_all(&output).unwrap();
        fs::write(left.join("file.txt"), "a\nold\nc\n").unwrap();
        fs::write(right.join("file.txt"), "a\nnew\nc\n").unwrap();

        let mut app = DiffApp::open(left, right, output).unwrap();
        let err = app
            .handle_key(crossterm::event::KeyEvent::new(
                KeyCode::Char('q'),
                KeyModifiers::NONE,
            ))
            .unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::Interrupted);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn manual_file_edit_can_be_driven_by_keys() {
        let root = std::env::temp_dir().join(format!(
            "jjc-diff-manual-keys-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let left = root.join("left");
        let right = root.join("right");
        let output = root.join("output");
        fs::create_dir_all(&left).unwrap();
        fs::create_dir_all(&right).unwrap();
        fs::create_dir_all(&output).unwrap();
        fs::write(left.join("file.txt"), "a\nold\nc\n").unwrap();
        fs::write(right.join("file.txt"), "a\nnew\nc\n").unwrap();

        let mut app = DiffApp::open(left, right, output.clone()).unwrap();
        app.handle_key(crossterm::event::KeyEvent::new(
            KeyCode::Char('e'),
            KeyModifiers::NONE,
        ))
        .unwrap();
        app.handle_key(crossterm::event::KeyEvent::new(
            KeyCode::Char('i'),
            KeyModifiers::NONE,
        ))
        .unwrap();
        app.handle_key(crossterm::event::KeyEvent::new(
            KeyCode::Char('X'),
            KeyModifiers::NONE,
        ))
        .unwrap();
        app.handle_key(crossterm::event::KeyEvent::new(
            KeyCode::Esc,
            KeyModifiers::NONE,
        ))
        .unwrap();
        app.handle_key(crossterm::event::KeyEvent::new(
            KeyCode::Esc,
            KeyModifiers::NONE,
        ))
        .unwrap();
        assert!(
            app.handle_key(crossterm::event::KeyEvent::new(
                KeyCode::Char('w'),
                KeyModifiers::NONE,
            ))
            .unwrap()
        );

        assert_eq!(
            fs::read_to_string(output.join("file.txt")).unwrap(),
            "Xa\nnew\nc\n"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn diff_lines_highlight_non_rust_code() {
        let root = std::env::temp_dir().join(format!(
            "jjc-diff-highlight-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let left = root.join("left");
        let right = root.join("right");
        let output = root.join("output");
        fs::create_dir_all(&left).unwrap();
        fs::create_dir_all(&right).unwrap();
        fs::create_dir_all(&output).unwrap();
        fs::write(left.join("app.py"), "def main():\n    return \"old\"\n").unwrap();
        fs::write(
            right.join("app.py"),
            "def main():\n    return \"new\" # greet\n",
        )
        .unwrap();

        let mut app = DiffApp::open(left, right, output).unwrap();
        let lines = app.lines();

        assert!(has_span(
            &lines,
            "return",
            crate::syntax::HighlightClass::Keyword
        ));
        assert!(has_span(
            &lines,
            "\"new\"",
            crate::syntax::HighlightClass::String
        ));
        assert!(has_span(
            &lines,
            "# greet",
            crate::syntax::HighlightClass::Comment
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rust_hunks_are_labeled_with_function() {
        let left = "fn demo() {\n    let value = 1;\n}\n";
        let right = "fn demo() {\n    let value = 2;\n}\n";
        let hunks = hunks(Path::new("lib.rs"), left, right);

        assert_eq!(hunks[0].function.as_deref(), Some("demo"));
        assert!(hunks[0].summary.contains("fn demo"));
    }

    #[test]
    fn invalid_rust_still_gets_line_hunks() {
        let left = "let value = ;\n";
        let right = "let value = 2;\n";
        let hunks = hunks(Path::new("lib.rs"), left, right);

        assert_eq!(hunks.len(), 1);
        assert!(hunks[0].function.is_none());
        assert!(!hunks[0].summary.contains("fn "));
    }

    #[test]
    fn function_toggle_changes_all_hunks_in_function() {
        let root = std::env::temp_dir().join(format!(
            "jjc-diff-function-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let left = root.join("left");
        let right = root.join("right");
        let output = root.join("output");
        fs::create_dir_all(&left).unwrap();
        fs::create_dir_all(&right).unwrap();
        fs::create_dir_all(&output).unwrap();
        fs::write(
            left.join("lib.rs"),
            "fn demo() {\n    let a = 1;\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n    let b = 1;\n}\n",
        )
        .unwrap();
        fs::write(
            right.join("lib.rs"),
            "fn demo() {\n    let a = 2;\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n    let b = 2;\n}\n",
        )
        .unwrap();

        let mut app = DiffApp::open(left, right, output).unwrap();
        assert_eq!(app.files[0].hunks.len(), 2);
        app.toggle_current_function();

        assert!(app.files[0].hunks.iter().all(|hunk| !hunk.selected));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn undo_and_redo_restore_selection_state() {
        let root = std::env::temp_dir().join(format!(
            "jjc-diff-undo-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let left = root.join("left");
        let right = root.join("right");
        let output = root.join("output");
        fs::create_dir_all(&left).unwrap();
        fs::create_dir_all(&right).unwrap();
        fs::create_dir_all(&output).unwrap();
        fs::write(left.join("file.txt"), "a\nold\nc\n").unwrap();
        fs::write(right.join("file.txt"), "a\nnew\nc\n").unwrap();

        let mut app = DiffApp::open(left, right, output).unwrap();
        app.line_cursor = 1;
        app.toggle_current_line();
        assert_eq!(app.files[0].render(), Some(b"a\nold\nc\n".to_vec()));
        app.undo();
        assert_eq!(app.files[0].render(), Some(b"a\nnew\nc\n".to_vec()));
        app.redo();
        assert_eq!(app.files[0].render(), Some(b"a\nold\nc\n".to_vec()));

        fs::remove_dir_all(root).unwrap();
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
