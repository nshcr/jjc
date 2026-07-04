use std::collections::BTreeSet;
use std::fs;
use std::io;
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
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use similar::Algorithm;
use similar::DiffOp;
use similar::DiffTag;
use similar::TextDiff;

use crate::buffer::TextBuffer;
use crate::input;
use crate::syntax;
use crate::vim::Vim;
use crate::vim::VimMode;

pub struct DiffApp {
    output: PathBuf,
    files: Vec<DiffFile>,
    entries: Vec<Entry>,
    cursor: usize,
    line_cursor: usize,
    mode: Mode,
    edit_vim: Vim,
    undo: Vec<SelectionSnapshot>,
    redo: Vec<SelectionSnapshot>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Mode {
    Select,
    Edit,
}

struct DiffFile {
    path: PathBuf,
    left_lines: Vec<String>,
    right_lines: Vec<String>,
    hunks: Vec<Hunk>,
    manual_output: Option<TextBuffer>,
    unsupported: Option<String>,
}

#[derive(Clone, Copy)]
struct Entry {
    file: usize,
    hunk: usize,
}

struct Hunk {
    old_start: usize,
    old_end: usize,
    new_start: usize,
    new_end: usize,
    selected: bool,
    line_choices: Vec<LineChoice>,
    function: Option<String>,
    summary: String,
}

enum LineChoice {
    Equal,
    Changed {
        selected: bool,
        old_index: Option<usize>,
    },
}

#[derive(Clone, Eq, PartialEq)]
struct SelectionSnapshot(Vec<Vec<Vec<bool>>>);

impl DiffApp {
    pub fn open(left: PathBuf, right: PathBuf, output: PathBuf) -> io::Result<Self> {
        let paths = changed_paths(&left, &right)?;
        let mut files = Vec::new();
        let mut entries = Vec::new();

        for path in paths {
            let file = DiffFile::load(&left, &right, path)?;
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
            mode: Mode::Select,
            edit_vim: Vim::new(),
            undo: Vec::new(),
            redo: Vec::new(),
        })
    }

    pub fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
        loop {
            terminal.draw(|frame| {
                let rows = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(1)])
                    .split(frame.area());
                frame.render_widget(
                    Paragraph::new(self.lines())
                        .block(Block::new().title("jjc diff").borders(Borders::ALL)),
                    rows[0],
                );
                frame.render_widget(Paragraph::new(self.status()), rows[1]);
                if self.mode == Mode::Edit
                    && let Some(buffer) = self.current_edit_buffer()
                {
                    let x = rows[0].x
                        + 1
                        + buffer
                            .cursor_column()
                            .min(rows[0].width.saturating_sub(2) as usize)
                            as u16;
                    let y = rows[0].y
                        + 2
                        + buffer
                            .cursor_y()
                            .min(rows[0].height.saturating_sub(3) as usize)
                            as u16;
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
            KeyCode::Char('n') => self.move_line_down(),
            KeyCode::Char('p') => self.move_line_up(),
            KeyCode::Char(' ') => self.toggle_current(),
            KeyCode::Char('x') => self.toggle_current_line(),
            KeyCode::Char('f') => self.toggle_current_function(),
            KeyCode::Char('e') => self.enter_edit_mode(),
            KeyCode::Char('u') => self.undo(),
            KeyCode::Char('r') => self.redo(),
            KeyCode::Char('w') => {
                self.write_output()?;
                return Ok(true);
            }
            KeyCode::Char('q') => return Ok(true),
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

    fn lines(&self) -> Vec<Line<'_>> {
        if self.files.is_empty() {
            return vec![Line::from("no changed files")];
        }

        if self.mode == Mode::Edit {
            let Some(entry) = self.entries.get(self.cursor) else {
                return vec![Line::from("no editable file")];
            };
            let file = &self.files[entry.file];
            let mut lines = vec![Line::from(Span::styled(
                format!("editing {}", file.path.display()),
                Style::new().add_modifier(Modifier::BOLD),
            ))];
            if let Some(buffer) = &file.manual_output {
                lines.extend(
                    buffer
                        .lines()
                        .iter()
                        .take(200)
                        .map(|line| Line::from(line.as_str())),
                );
            }
            return lines;
        }

        let mut lines = Vec::new();
        let mut entry_index = 0;
        for (file_index, file) in self.files.iter().enumerate() {
            lines.push(Line::from(Span::styled(
                file.path.display().to_string(),
                Style::new().add_modifier(Modifier::BOLD),
            )));
            if let Some(reason) = &file.unsupported {
                lines.push(Line::from(Span::styled(
                    format!("  unsupported: {reason}"),
                    Style::new().add_modifier(Modifier::DIM),
                )));
                continue;
            }
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
                    for (line_index, choice) in hunk.line_choices.iter().enumerate() {
                        let cursor = if line_index == self.line_cursor {
                            ">"
                        } else {
                            " "
                        };
                        let marker = match choice {
                            LineChoice::Equal => "[=]",
                            LineChoice::Changed { selected: true, .. } => "[x]",
                            LineChoice::Changed {
                                selected: false, ..
                            } => "[ ]",
                        };
                        let text = file.right_lines[hunk.new_start + line_index].trim_end();
                        lines.push(Line::from(format!("  {cursor} {marker} +{text}")));
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

    fn status(&self) -> &'static str {
        match self.mode {
            Mode::Select => {
                "j/k hunk  n/p line  space/x toggle  f function  e edit output  u undo  r redo  w write  q quit"
            }
            Mode::Edit if self.edit_vim.mode() == VimMode::Normal => {
                "EDIT OUTPUT NORMAL  i/a/o insert  h/j/k/l/w/b/e move  x/dd delete  Esc select"
            }
            Mode::Edit => "EDIT OUTPUT INSERT  Esc normal",
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

    fn move_line_down(&mut self) {
        if let Some(hunk) = self.current_hunk() {
            self.line_cursor =
                (self.line_cursor + 1).min(hunk.line_choices.len().saturating_sub(1));
        }
    }

    fn move_line_up(&mut self) {
        self.line_cursor = self.line_cursor.saturating_sub(1);
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

    fn enter_edit_mode(&mut self) {
        let Some(entry) = self.entries.get(self.cursor).copied() else {
            return;
        };
        let file = &mut self.files[entry.file];
        if file.unsupported.is_some() {
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
            let path = self.output.join(&file.path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, file.render())?;
        }
        Ok(())
    }
}

impl DiffFile {
    fn load(left_root: &Path, right_root: &Path, path: PathBuf) -> io::Result<Self> {
        let left_path = left_root.join(&path);
        let right_path = right_root.join(&path);
        if !left_path.is_file() || !right_path.is_file() {
            return Ok(Self::unsupported(
                path,
                "only files present on both sides are supported",
            ));
        }

        let left_bytes = fs::read(&left_path)?;
        let right_bytes = fs::read(&right_path)?;
        let (left, right) = match (str::from_utf8(&left_bytes), str::from_utf8(&right_bytes)) {
            (Ok(left), Ok(right)) => (left, right),
            _ => return Ok(Self::unsupported(path, "binary or non-UTF-8 file")),
        };

        let left_lines = split_keep_newline(left);
        let right_lines = split_keep_newline(right);
        let hunks = hunks(&path, left, right);

        Ok(Self {
            path,
            left_lines,
            right_lines,
            hunks,
            manual_output: None,
            unsupported: None,
        })
    }

    fn unsupported(path: PathBuf, reason: impl Into<String>) -> Self {
        Self {
            path,
            left_lines: Vec::new(),
            right_lines: Vec::new(),
            hunks: Vec::new(),
            manual_output: None,
            unsupported: Some(reason.into()),
        }
    }

    fn render(&self) -> String {
        if let Some(output) = &self.manual_output {
            return output.to_text();
        }
        self.render_selection()
    }

    fn render_selection(&self) -> String {
        render_hunks(&self.left_lines, &self.right_lines, &self.hunks)
    }
}

fn changed_paths(left: &Path, right: &Path) -> io::Result<Vec<PathBuf>> {
    let mut paths = BTreeSet::new();
    collect_paths(left, left, &mut paths)?;
    collect_paths(right, right, &mut paths)?;
    Ok(paths
        .into_iter()
        .filter(|path| {
            let left_path = left.join(path);
            let right_path = right.join(path);
            left_path.exists() != right_path.exists()
                || (left_path.is_file()
                    && right_path.is_file()
                    && fs::read(left_path).ok() != fs::read(right_path).ok())
        })
        .collect())
}

fn collect_paths(root: &Path, dir: &Path, paths: &mut BTreeSet<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
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

    let mut line_choices = (new_start..new_end)
        .map(|_| LineChoice::Equal)
        .collect::<Vec<_>>();
    for op in ops {
        let (tag, old, new) = op.as_tag_tuple();
        match tag {
            DiffTag::Equal | DiffTag::Delete => {}
            DiffTag::Insert => {
                for new_index in new {
                    line_choices[new_index - new_start] = LineChoice::Changed {
                        selected: true,
                        old_index: None,
                    };
                }
            }
            DiffTag::Replace => {
                for (offset, new_index) in new.enumerate() {
                    let old_index = (offset < old.len()).then_some(old.start + offset);
                    line_choices[new_index - new_start] = LineChoice::Changed {
                        selected: true,
                        old_index,
                    };
                }
            }
        }
    }

    let function = syntax::function_for_line(path, right, new_start + 1);
    let function_suffix = function
        .as_ref()
        .map(|name| format!(" fn {name}"))
        .unwrap_or_default();

    Some(Hunk {
        old_start,
        old_end,
        new_start,
        new_end,
        selected: true,
        line_choices,
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

fn render_hunks(left: &[String], right: &[String], hunks: &[Hunk]) -> String {
    let mut output = Vec::new();
    let mut new_cursor = 0;
    for hunk in hunks {
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
    for (index, choice) in hunk.line_choices.iter().enumerate() {
        match choice {
            LineChoice::Equal => output.push(right[hunk.new_start + index].clone()),
            LineChoice::Changed { selected: true, .. } => {
                output.push(right[hunk.new_start + index].clone());
            }
            LineChoice::Changed {
                selected: false,
                old_index: Some(old_index),
            } => output.push(left[*old_index].clone()),
            LineChoice::Changed {
                selected: false,
                old_index: None,
            } => {}
        }
    }
    output
}

impl Hunk {
    fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
        for choice in &mut self.line_choices {
            if let LineChoice::Changed {
                selected: line_selected,
                ..
            } = choice
            {
                *line_selected = selected;
            }
        }
    }

    fn toggle_line(&mut self, index: usize) {
        if let Some(LineChoice::Changed { selected, .. }) = self.line_choices.get_mut(index) {
            *selected = !*selected;
            self.selected = self.any_selected();
        }
    }

    fn selected_lines(&self) -> Vec<bool> {
        self.line_choices
            .iter()
            .map(|choice| match choice {
                LineChoice::Equal => true,
                LineChoice::Changed { selected, .. } => *selected,
            })
            .collect()
    }

    fn restore_selected_lines(&mut self, selected_lines: &[bool]) {
        for (choice, selected) in self.line_choices.iter_mut().zip(selected_lines) {
            if let LineChoice::Changed {
                selected: line_selected,
                ..
            } = choice
            {
                *line_selected = *selected;
            }
        }
        self.selected = self.any_selected();
    }

    fn any_selected(&self) -> bool {
        self.line_choices.iter().any(|choice| match choice {
            LineChoice::Equal => false,
            LineChoice::Changed { selected, .. } => *selected,
        })
    }

    fn all_selected(&self) -> bool {
        self.selected
            && self.line_choices.iter().all(|choice| match choice {
                LineChoice::Equal => true,
                LineChoice::Changed { selected, .. } => *selected,
            })
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
    fn replacement_line_can_fall_back_to_old_line() {
        let left = "a\nold\nc\n";
        let right = "a\nnew\nc\n";
        let left_lines = split_keep_newline(left);
        let right_lines = split_keep_newline(right);
        let mut hunks = hunks(Path::new("file.txt"), left, right);
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
        fs::write(left.join("file.bin"), [0]).unwrap();
        fs::write(right.join("file.bin"), [0xff]).unwrap();

        let app = DiffApp::open(left, right, output).unwrap();
        let err = app.write_output().unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("file.bin"));
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
        assert_eq!(app.files[0].render(), "a\nold\nc\n");
        app.undo();
        assert_eq!(app.files[0].render(), "a\nnew\nc\n");
        app.redo();
        assert_eq!(app.files[0].render(), "a\nold\nc\n");

        fs::remove_dir_all(root).unwrap();
    }
}
