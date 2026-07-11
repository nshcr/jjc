use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone)]
pub struct TextBuffer {
    lines: Vec<String>,
    trailing_newline: bool,
    cursor_x: usize,
    cursor_y: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BufferRange {
    Char {
        line: usize,
        start: usize,
        end: usize,
    },
    Line {
        line: usize,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum EditCommand {
    MoveLeft,
    MoveRight,
    MoveRightInsert,
    MoveLineStart,
    MoveFirstNonBlank,
    MoveLineEnd,
    MoveLastNonBlank,
    MoveFileStart,
    MoveFileEnd,
    MoveWordForward,
    MoveWordBackward,
    MoveWordEnd,
    MoveWordEndBackward,
    MoveBigWordForward,
    MoveBigWordBackward,
    MoveBigWordEnd,
    MoveBigWordEndBackward,
    MoveUp,
    MoveDown,
    InsertChar(char),
    InsertText(String),
    InsertNewline,
    OpenLineBelow,
    OpenLineAbove,
    Backspace,
    DeleteChar,
    DeleteCharBefore,
    ReplaceChar(char),
    DeleteLine,
    DeleteToLineStart,
    DeleteToLineEnd,
    ChangeLine,
    JoinLineBelow,
    ToggleCharCase,
    LowercaseLine,
    UppercaseLine,
    ToggleLineCase,
    ReplaceAll(String),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AgentSuggestion {
    replacement: String,
}

impl AgentSuggestion {
    pub fn replace_all(replacement: impl Into<String>) -> Self {
        Self {
            replacement: replacement.into(),
        }
    }

    pub fn into_command(self) -> EditCommand {
        EditCommand::ReplaceAll(self.replacement)
    }
}

impl TextBuffer {
    pub fn from_text(content: &str) -> Self {
        let (lines, trailing_newline) = split_text(content);
        Self {
            lines,
            trailing_newline,
            cursor_x: 0,
            cursor_y: 0,
        }
    }

    pub fn to_text(&self) -> String {
        join_text(&self.lines, self.trailing_newline)
    }

    pub fn set_text(&mut self, content: &str) {
        *self = Self::from_text(content);
    }

    pub fn apply(&mut self, command: EditCommand) {
        match command {
            EditCommand::MoveLeft => self.move_left(),
            EditCommand::MoveRight => self.move_right(),
            EditCommand::MoveRightInsert => self.move_right_insert(),
            EditCommand::MoveLineStart => self.move_line_start(),
            EditCommand::MoveFirstNonBlank => self.move_first_nonblank(),
            EditCommand::MoveLineEnd => self.move_line_end(),
            EditCommand::MoveLastNonBlank => self.move_last_nonblank(),
            EditCommand::MoveFileStart => self.move_file_start(),
            EditCommand::MoveFileEnd => self.move_file_end(),
            EditCommand::MoveWordForward => self.move_word_forward(),
            EditCommand::MoveWordBackward => self.move_word_backward(),
            EditCommand::MoveWordEnd => self.move_word_end(),
            EditCommand::MoveWordEndBackward => self.move_word_end_backward(),
            EditCommand::MoveBigWordForward => self.move_big_word_forward(),
            EditCommand::MoveBigWordBackward => self.move_big_word_backward(),
            EditCommand::MoveBigWordEnd => self.move_big_word_end(),
            EditCommand::MoveBigWordEndBackward => self.move_big_word_end_backward(),
            EditCommand::MoveUp => self.move_up(),
            EditCommand::MoveDown => self.move_down(),
            EditCommand::InsertChar(c) => self.insert_char(c),
            EditCommand::InsertText(text) => {
                for c in text.chars() {
                    if c == '\n' {
                        self.insert_newline();
                    } else {
                        self.insert_char(c);
                    }
                }
            }
            EditCommand::InsertNewline => self.insert_newline(),
            EditCommand::OpenLineBelow => self.open_line_below(),
            EditCommand::OpenLineAbove => self.open_line_above(),
            EditCommand::Backspace => self.backspace(),
            EditCommand::DeleteChar => self.delete_char(),
            EditCommand::DeleteCharBefore => self.delete_char_before(),
            EditCommand::ReplaceChar(c) => self.replace_char(c),
            EditCommand::DeleteLine => self.delete_line(),
            EditCommand::DeleteToLineStart => self.delete_to_line_start(),
            EditCommand::DeleteToLineEnd => self.delete_to_line_end(),
            EditCommand::ChangeLine => self.change_line(),
            EditCommand::JoinLineBelow => self.join_line_below(),
            EditCommand::ToggleCharCase => self.toggle_char_case(),
            EditCommand::LowercaseLine => self.lowercase_line(),
            EditCommand::UppercaseLine => self.uppercase_line(),
            EditCommand::ToggleLineCase => self.toggle_line_case(),
            EditCommand::ReplaceAll(content) => self.set_text(&content),
        }
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    pub fn cursor_y(&self) -> usize {
        self.cursor_y
    }

    pub fn move_to_line(&mut self, line: usize) {
        self.cursor_y = line.min(self.lines.len() - 1);
        self.clamp_cursor();
    }

    pub fn current_line(&self) -> &str {
        &self.lines[self.cursor_y]
    }

    pub fn current_line_range(&self) -> BufferRange {
        BufferRange::Line {
            line: self.cursor_y,
        }
    }

    pub fn range_to_line_end(&self) -> BufferRange {
        BufferRange::Char {
            line: self.cursor_y,
            start: self.cursor_x,
            end: self.lines[self.cursor_y].len(),
        }
    }

    pub fn range_to_word_forward(&self) -> BufferRange {
        let line = &self.lines[self.cursor_y];
        BufferRange::Char {
            line: self.cursor_y,
            start: self.cursor_x,
            end: next_word_start(line, self.cursor_x).unwrap_or(line.len()),
        }
    }

    pub fn range_char_forward(&self) -> BufferRange {
        let line = &self.lines[self.cursor_y];
        BufferRange::Char {
            line: self.cursor_y,
            start: self.cursor_x,
            end: next_boundary(line, self.cursor_x).min(line.len()),
        }
    }

    pub fn range_to_column(&self, column: usize, inclusive: bool) -> BufferRange {
        let line = &self.lines[self.cursor_y];
        let start = self.cursor_x.min(column);
        let mut end = self.cursor_x.max(column);
        if inclusive {
            end = next_boundary(line, end).min(line.len());
        }
        BufferRange::Char {
            line: self.cursor_y,
            start,
            end,
        }
    }

    pub fn range_to_char_column(&self, column: usize, inclusive: bool) -> BufferRange {
        self.range_to_column(
            char_column_to_byte(&self.lines[self.cursor_y], column),
            inclusive,
        )
    }

    pub fn range_inner_word(&self) -> Option<BufferRange> {
        let line = &self.lines[self.cursor_y];
        let (start, end) = word_span_at_or_after(line, self.cursor_x)?;
        Some(BufferRange::Char {
            line: self.cursor_y,
            start,
            end,
        })
    }

    pub fn range_text(&self, range: BufferRange) -> String {
        match range {
            BufferRange::Char { line, start, end } => self.lines[line][start..end].to_owned(),
            BufferRange::Line { line } => self.lines[line].clone(),
        }
    }

    pub fn delete_range(&mut self, range: BufferRange) {
        match range {
            BufferRange::Char { line, start, end } => {
                self.lines[line].drain(start..end);
                self.cursor_y = line;
                self.cursor_x = start.min(self.lines[line].len());
                self.clamp_cursor();
            }
            BufferRange::Line { line } => {
                self.cursor_y = line;
                self.delete_line();
            }
        }
    }

    pub fn replace_lines(&mut self, start: usize, end: usize, replacement: &[String]) {
        let start = start.min(self.lines.len());
        let end = end.min(self.lines.len()).max(start);
        self.lines.splice(start..end, replacement.iter().cloned());
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.cursor_y = start.min(self.lines.len() - 1);
        self.cursor_x = 0;
    }

    pub fn lowercase_range(&mut self, range: BufferRange) {
        self.replace_range(range, str::to_lowercase);
    }

    pub fn uppercase_range(&mut self, range: BufferRange) {
        self.replace_range(range, str::to_uppercase);
    }

    pub fn toggle_range_case(&mut self, range: BufferRange) {
        self.replace_range(range, |text| text.chars().map(toggle_case).collect());
    }

    pub fn char_before_cursor(&self) -> Option<char> {
        self.lines[self.cursor_y][..self.cursor_x]
            .chars()
            .next_back()
    }

    pub fn cursor_column(&self) -> usize {
        self.lines[self.cursor_y][..self.cursor_x].chars().count()
    }

    pub fn cursor_byte(&self) -> usize {
        self.cursor_x
    }

    pub fn move_left(&mut self) {
        self.cursor_x = prev_boundary(&self.lines[self.cursor_y], self.cursor_x);
    }

    pub fn move_right(&mut self) {
        let next = next_boundary(&self.lines[self.cursor_y], self.cursor_x);
        self.cursor_x = next.min(last_char_boundary(&self.lines[self.cursor_y]));
    }

    pub fn move_right_insert(&mut self) {
        self.cursor_x = next_boundary(&self.lines[self.cursor_y], self.cursor_x);
    }

    pub fn move_line_start(&mut self) {
        self.cursor_x = 0;
    }

    pub fn move_to_char_column(&mut self, column: usize) {
        self.cursor_x = char_column_to_byte(&self.lines[self.cursor_y], column);
    }

    pub fn move_first_nonblank(&mut self) {
        self.cursor_x = self.lines[self.cursor_y]
            .grapheme_indices(true)
            .find(|(_, grapheme)| !grapheme_is_whitespace(grapheme))
            .map(|(index, _)| index)
            .unwrap_or(0);
    }

    pub fn move_line_end(&mut self) {
        self.cursor_x = last_char_boundary(&self.lines[self.cursor_y]);
    }

    pub fn move_last_nonblank(&mut self) {
        self.cursor_x = self.lines[self.cursor_y]
            .grapheme_indices(true)
            .rev()
            .find(|(_, grapheme)| !grapheme_is_whitespace(grapheme))
            .map(|(index, _)| index)
            .unwrap_or(0);
    }

    pub fn move_file_start(&mut self) {
        self.cursor_y = 0;
        self.cursor_x = 0;
    }

    pub fn move_file_end(&mut self) {
        self.cursor_y = self.lines.len() - 1;
        self.move_line_end();
    }

    pub fn move_up(&mut self) {
        self.cursor_y = self.cursor_y.saturating_sub(1);
        self.clamp_cursor();
    }

    pub fn move_down(&mut self) {
        self.cursor_y = (self.cursor_y + 1).min(self.lines.len() - 1);
        self.clamp_cursor();
    }

    pub fn move_word_forward(&mut self) {
        if let Some(index) = next_word_start(&self.lines[self.cursor_y], self.cursor_x) {
            self.cursor_x = index;
        } else if self.cursor_y + 1 < self.lines.len() {
            self.cursor_y += 1;
            self.cursor_x = 0;
            self.move_first_nonblank();
        } else {
            self.move_line_end();
        }
    }

    pub fn move_word_backward(&mut self) {
        if let Some(index) = previous_word_start(&self.lines[self.cursor_y], self.cursor_x) {
            self.cursor_x = index;
        } else if self.cursor_y > 0 {
            self.cursor_y -= 1;
            self.move_line_end();
        } else {
            self.cursor_x = 0;
        }
    }

    pub fn move_word_end(&mut self) {
        if let Some(index) = next_word_end(&self.lines[self.cursor_y], self.cursor_x) {
            self.cursor_x = index;
        } else if self.cursor_y + 1 < self.lines.len() {
            self.cursor_y += 1;
            self.cursor_x = 0;
            self.move_word_end();
        } else {
            self.move_line_end();
        }
    }

    pub fn move_word_end_backward(&mut self) {
        if let Some(index) = previous_word_end(&self.lines[self.cursor_y], self.cursor_x) {
            self.cursor_x = index;
        } else if self.cursor_y > 0 {
            self.cursor_y -= 1;
            self.move_line_end();
        } else {
            self.cursor_x = 0;
        }
    }

    pub fn move_big_word_forward(&mut self) {
        if let Some(index) = next_big_word_start(&self.lines[self.cursor_y], self.cursor_x) {
            self.cursor_x = index;
        } else if self.cursor_y + 1 < self.lines.len() {
            self.cursor_y += 1;
            self.cursor_x = 0;
            self.move_first_nonblank();
        } else {
            self.move_line_end();
        }
    }

    pub fn move_big_word_backward(&mut self) {
        if let Some(index) = previous_big_word_start(&self.lines[self.cursor_y], self.cursor_x) {
            self.cursor_x = index;
        } else if self.cursor_y > 0 {
            self.cursor_y -= 1;
            self.move_line_end();
        } else {
            self.cursor_x = 0;
        }
    }

    pub fn move_big_word_end(&mut self) {
        if let Some(index) = next_big_word_end(&self.lines[self.cursor_y], self.cursor_x) {
            self.cursor_x = index;
        } else if self.cursor_y + 1 < self.lines.len() {
            self.cursor_y += 1;
            self.cursor_x = 0;
            self.move_big_word_end();
        } else {
            self.move_line_end();
        }
    }

    pub fn move_big_word_end_backward(&mut self) {
        if let Some(index) = previous_big_word_end(&self.lines[self.cursor_y], self.cursor_x) {
            self.cursor_x = index;
        } else if self.cursor_y > 0 {
            self.cursor_y -= 1;
            self.move_line_end();
        } else {
            self.cursor_x = 0;
        }
    }

    pub fn insert_char(&mut self, c: char) {
        self.lines[self.cursor_y].insert(self.cursor_x, c);
        self.cursor_x += c.len_utf8();
    }

    pub fn insert_newline(&mut self) {
        let tail = self.lines[self.cursor_y].split_off(self.cursor_x);
        self.cursor_y += 1;
        self.cursor_x = 0;
        self.lines.insert(self.cursor_y, tail);
    }

    pub fn open_line_below(&mut self) {
        self.cursor_y += 1;
        self.cursor_x = 0;
        self.lines.insert(self.cursor_y, String::new());
    }

    pub fn open_line_above(&mut self) {
        self.cursor_x = 0;
        self.lines.insert(self.cursor_y, String::new());
    }

    pub fn backspace(&mut self) {
        if self.cursor_x > 0 {
            let previous = prev_boundary(&self.lines[self.cursor_y], self.cursor_x);
            self.lines[self.cursor_y].drain(previous..self.cursor_x);
            self.cursor_x = previous;
        } else if self.cursor_y > 0 {
            let line = self.lines.remove(self.cursor_y);
            self.cursor_y -= 1;
            self.cursor_x = self.lines[self.cursor_y].len();
            self.lines[self.cursor_y].push_str(&line);
        }
    }

    pub fn delete_char(&mut self) {
        if self.cursor_x < self.lines[self.cursor_y].len() {
            let end = next_boundary(&self.lines[self.cursor_y], self.cursor_x);
            self.lines[self.cursor_y].drain(self.cursor_x..end);
        }
    }

    pub fn delete_char_before(&mut self) {
        if self.cursor_x > 0 {
            let previous = prev_boundary(&self.lines[self.cursor_y], self.cursor_x);
            self.lines[self.cursor_y].drain(previous..self.cursor_x);
            self.cursor_x = previous;
        }
    }

    pub fn replace_char(&mut self, c: char) {
        if self.cursor_x < self.lines[self.cursor_y].len() {
            let end = next_boundary(&self.lines[self.cursor_y], self.cursor_x);
            self.lines[self.cursor_y].replace_range(self.cursor_x..end, &c.to_string());
        }
    }

    pub fn delete_line(&mut self) {
        if self.lines.len() == 1 {
            self.lines[0].clear();
        } else {
            self.lines.remove(self.cursor_y);
            self.clamp_cursor();
        }
    }

    pub fn take_current_line(&mut self) -> String {
        let line = self.lines[self.cursor_y].clone();
        self.delete_line();
        line
    }

    pub fn insert_line_below(&mut self, line: String) {
        let index = (self.cursor_y + 1).min(self.lines.len());
        self.lines.insert(index, line);
        self.cursor_y = index;
        self.cursor_x = 0;
    }

    pub fn insert_line_above(&mut self, line: String) {
        self.lines.insert(self.cursor_y, line);
        self.cursor_x = 0;
    }

    pub fn delete_to_line_end(&mut self) {
        self.lines[self.cursor_y].truncate(self.cursor_x);
    }

    pub fn delete_to_line_start(&mut self) {
        self.lines[self.cursor_y].drain(..self.cursor_x);
        self.cursor_x = 0;
    }

    pub fn change_line(&mut self) {
        self.lines[self.cursor_y].clear();
        self.cursor_x = 0;
    }

    pub fn join_line_below(&mut self) {
        if self.cursor_y + 1 >= self.lines.len() {
            return;
        }
        let next = self.lines.remove(self.cursor_y + 1);
        let line = &mut self.lines[self.cursor_y];
        if !line.is_empty() && !line.ends_with(char::is_whitespace) && !next.trim().is_empty() {
            line.push(' ');
        }
        line.push_str(next.trim_start());
        self.cursor_x = last_char_boundary(line);
    }

    pub fn toggle_char_case(&mut self) {
        if self.cursor_x >= self.lines[self.cursor_y].len() {
            return;
        }
        let end = next_boundary(&self.lines[self.cursor_y], self.cursor_x);
        let replacement = self.lines[self.cursor_y][self.cursor_x..end]
            .chars()
            .map(toggle_case)
            .collect::<String>();
        self.lines[self.cursor_y].replace_range(self.cursor_x..end, &replacement);
        self.move_right_insert();
    }

    pub fn lowercase_line(&mut self) {
        self.lines[self.cursor_y] = self.lines[self.cursor_y].to_lowercase();
        self.clamp_cursor();
    }

    pub fn uppercase_line(&mut self) {
        self.lines[self.cursor_y] = self.lines[self.cursor_y].to_uppercase();
        self.clamp_cursor();
    }

    pub fn toggle_line_case(&mut self) {
        self.lines[self.cursor_y] = self.lines[self.cursor_y].chars().map(toggle_case).collect();
        self.clamp_cursor();
    }

    fn clamp_cursor(&mut self) {
        self.cursor_y = self.cursor_y.min(self.lines.len() - 1);
        self.cursor_x = self.cursor_x.min(self.lines[self.cursor_y].len());
        self.cursor_x = floor_boundary(&self.lines[self.cursor_y], self.cursor_x);
    }

    fn replace_range(&mut self, range: BufferRange, replacement: impl FnOnce(&str) -> String) {
        match range {
            BufferRange::Char { line, start, end } => {
                let replacement = replacement(&self.lines[line][start..end]);
                self.lines[line].replace_range(start..end, &replacement);
                self.cursor_y = line;
                self.cursor_x =
                    floor_boundary(&self.lines[line], start.min(self.lines[line].len()));
            }
            BufferRange::Line { line } => {
                self.lines[line] = replacement(&self.lines[line]);
                self.cursor_y = line;
                self.clamp_cursor();
            }
        }
    }
}

fn split_text(content: &str) -> (Vec<String>, bool) {
    let trailing_newline = content.ends_with('\n');
    let body = content.strip_suffix('\n').unwrap_or(content);
    let mut lines = if body.is_empty() {
        vec![String::new()]
    } else {
        body.split('\n').map(str::to_owned).collect()
    };
    if lines.is_empty() {
        lines.push(String::new());
    }
    (lines, trailing_newline)
}

fn join_text(lines: &[String], trailing_newline: bool) -> String {
    let mut content = lines.join("\n");
    if trailing_newline {
        content.push('\n');
    }
    content
}

fn prev_boundary(line: &str, cursor: usize) -> usize {
    line.grapheme_indices(true)
        .map(|(index, _)| index)
        .take_while(|index| *index < cursor)
        .last()
        .unwrap_or(0)
}

fn next_boundary(line: &str, cursor: usize) -> usize {
    line.grapheme_indices(true)
        .map(|(index, _)| index)
        .find(|index| *index > cursor)
        .unwrap_or(line.len())
}

fn last_char_boundary(line: &str) -> usize {
    line.grapheme_indices(true)
        .map(|(index, _)| index)
        .next_back()
        .unwrap_or(0)
}

fn floor_boundary(line: &str, cursor: usize) -> usize {
    if cursor >= line.len() {
        return line.len();
    }
    line.grapheme_indices(true)
        .map(|(index, _)| index)
        .take_while(|index| *index <= cursor)
        .last()
        .unwrap_or(0)
}

fn char_column_to_byte(line: &str, column: usize) -> usize {
    let byte = line
        .char_indices()
        .map(|(index, _)| index)
        .nth(column)
        .unwrap_or(line.len());
    floor_boundary(line, byte)
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn grapheme_is_word(grapheme: &str) -> bool {
    grapheme.chars().next().is_some_and(is_word)
}

fn grapheme_is_whitespace(grapheme: &str) -> bool {
    grapheme.chars().all(char::is_whitespace)
}

fn word_span_at_or_after(line: &str, cursor: usize) -> Option<(usize, usize)> {
    let mut start = None;
    for (index, grapheme) in line.grapheme_indices(true) {
        if grapheme_is_word(grapheme) {
            start.get_or_insert(index);
        } else if let Some(word_start) = start.take()
            && cursor <= index
        {
            return Some((word_start, index));
        }
    }
    start
        .filter(|_| cursor <= line.len())
        .map(|start| (start, line.len()))
}

fn next_word_start(line: &str, cursor: usize) -> Option<usize> {
    let mut seen_non_word = false;
    for (index, grapheme) in line
        .grapheme_indices(true)
        .filter(|(index, _)| *index > cursor)
    {
        if grapheme_is_word(grapheme) {
            if seen_non_word {
                return Some(index);
            }
        } else {
            seen_non_word = true;
        }
    }
    None
}

fn previous_word_start(line: &str, cursor: usize) -> Option<usize> {
    let mut starts = Vec::new();
    let mut in_word = false;
    for (index, grapheme) in line.grapheme_indices(true) {
        if index >= cursor {
            break;
        }
        if grapheme_is_word(grapheme) {
            if !in_word {
                starts.push(index);
            }
            in_word = true;
        } else {
            in_word = false;
        }
    }
    starts.into_iter().last()
}

fn next_word_end(line: &str, cursor: usize) -> Option<usize> {
    let mut in_word = false;
    let mut last_word = None;
    for (index, grapheme) in line
        .grapheme_indices(true)
        .filter(|(index, _)| *index > cursor)
    {
        if grapheme_is_word(grapheme) {
            in_word = true;
            last_word = Some(index);
        } else if in_word {
            return last_word;
        }
    }
    last_word
}

fn previous_word_end(line: &str, cursor: usize) -> Option<usize> {
    let mut previous = None;
    let mut in_word = false;
    for (index, grapheme) in line.grapheme_indices(true) {
        if index >= cursor {
            break;
        }
        if grapheme_is_word(grapheme) {
            in_word = true;
            previous = Some(index);
        } else if in_word {
            in_word = false;
        }
    }
    previous
}

fn next_big_word_start(line: &str, cursor: usize) -> Option<usize> {
    let mut seen_space = false;
    for (index, grapheme) in line
        .grapheme_indices(true)
        .filter(|(index, _)| *index > cursor)
    {
        if grapheme_is_whitespace(grapheme) {
            seen_space = true;
        } else if seen_space {
            return Some(index);
        }
    }
    None
}

fn previous_big_word_start(line: &str, cursor: usize) -> Option<usize> {
    let mut starts = Vec::new();
    let mut in_word = false;
    for (index, grapheme) in line.grapheme_indices(true) {
        if index >= cursor {
            break;
        }
        if grapheme_is_whitespace(grapheme) {
            in_word = false;
        } else {
            if !in_word {
                starts.push(index);
            }
            in_word = true;
        }
    }
    starts.into_iter().last()
}

fn next_big_word_end(line: &str, cursor: usize) -> Option<usize> {
    let mut in_word = false;
    let mut last_word = None;
    for (index, grapheme) in line
        .grapheme_indices(true)
        .filter(|(index, _)| *index > cursor)
    {
        if grapheme_is_whitespace(grapheme) {
            if in_word {
                return last_word;
            }
        } else {
            in_word = true;
            last_word = Some(index);
        }
    }
    last_word
}

fn previous_big_word_end(line: &str, cursor: usize) -> Option<usize> {
    let mut previous = None;
    for (index, grapheme) in line.grapheme_indices(true) {
        if index >= cursor {
            break;
        }
        if !grapheme_is_whitespace(grapheme) {
            previous = Some(index);
        }
    }
    previous
}

fn toggle_case(c: char) -> String {
    if c.is_lowercase() {
        c.to_uppercase().collect()
    } else if c.is_uppercase() {
        c.to_lowercase().collect()
    } else {
        c.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_final_newline() {
        for content in ["", "a", "a\n", "a\n\n", "\n"] {
            let buffer = TextBuffer::from_text(content);
            assert_eq!(buffer.to_text(), content);
        }
    }

    #[test]
    fn moves_on_utf8_boundaries() {
        assert_eq!(next_boundary("a中", 0), 1);
        assert_eq!(next_boundary("a中", 1), 4);
        assert_eq!(prev_boundary("a中", 4), 1);
        assert_eq!(floor_boundary("中", 2), 0);
    }

    #[test]
    fn moves_and_deletes_on_grapheme_boundaries() {
        let combined = "e\u{301}x";
        assert_eq!(next_boundary(combined, 0), 3);
        assert_eq!(prev_boundary(combined, 3), 0);
        assert_eq!(floor_boundary(combined, 1), 0);

        let mut buffer = TextBuffer::from_text(combined);
        buffer.apply(EditCommand::DeleteChar);
        assert_eq!(buffer.to_text(), "x");
    }

    #[test]
    fn inner_word_delete_does_not_split_a_grapheme() {
        use crate::vim::Vim;
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let mut buffer = TextBuffer::from_text("e\u{301}x");
        let mut vim = Vim::new();
        for command in ['d', 'i', 'w'] {
            assert!(vim.handle_key(
                &mut buffer,
                KeyEvent::new(KeyCode::Char(command), KeyModifiers::NONE),
            ));
        }

        assert_eq!(buffer.to_text(), "");
    }

    #[test]
    fn word_and_big_word_helpers_return_grapheme_boundaries() {
        let line = "e\u{301} x";
        let boundaries = line
            .grapheme_indices(true)
            .map(|(index, _)| index)
            .chain(std::iter::once(line.len()))
            .collect::<Vec<_>>();
        let word_span = word_span_at_or_after(line, 0).unwrap();
        let offsets = [
            word_span.0,
            word_span.1,
            next_word_start(line, 0).unwrap(),
            previous_word_start(line, 4).unwrap(),
            next_word_end(line, 0).unwrap(),
            previous_word_end(line, 4).unwrap(),
            next_big_word_start(line, 0).unwrap(),
            previous_big_word_start(line, 4).unwrap(),
            next_big_word_end(line, 0).unwrap(),
            previous_big_word_end(line, 4).unwrap(),
        ];

        assert_eq!(word_span, (0, 3));
        assert!(offsets.iter().all(|offset| boundaries.contains(offset)));

        let mut buffer = TextBuffer::from_text("e\u{301}");
        buffer.apply(EditCommand::MoveLastNonBlank);
        assert_eq!(buffer.cursor_byte(), 0);
    }

    #[test]
    fn applies_structured_edit_commands() {
        let mut buffer = TextBuffer::from_text("");

        buffer.apply(EditCommand::InsertText("hello\nworld".into()));
        buffer.apply(EditCommand::MoveUp);
        buffer.apply(EditCommand::MoveRightInsert);
        buffer.apply(EditCommand::InsertChar('!'));

        assert_eq!(buffer.to_text(), "hello!\nworld");
    }

    #[test]
    fn agent_suggestion_is_explicit_buffer_edit() {
        let mut buffer = TextBuffer::from_text("old\n");
        let suggestion = AgentSuggestion::replace_all("new\n");

        buffer.apply(suggestion.into_command());

        assert_eq!(buffer.to_text(), "new\n");
    }

    #[test]
    fn replaces_line_range_and_moves_cursor_to_replacement() {
        let mut buffer = TextBuffer::from_text("a\n<<<<<<<\nleft\n=======\nright\n>>>>>>>\nz\n");

        buffer.replace_lines(1, 6, &["left".to_owned()]);

        assert_eq!(buffer.to_text(), "a\nleft\nz\n");
        assert_eq!(buffer.cursor_y(), 1);
    }
}
