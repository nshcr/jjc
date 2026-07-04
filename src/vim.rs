use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;

use crate::buffer::BufferRange;
use crate::buffer::EditCommand;
use crate::buffer::TextBuffer;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum VimMode {
    Normal,
    Insert,
}

#[derive(Default, Clone)]
struct Clipboard {
    text: String,
    linewise: bool,
}

pub struct Vim {
    mode: VimMode,
    pending: Option<Pending>,
    last_find: Option<FindMotion>,
    clipboard: Clipboard,
    undo: Vec<TextBuffer>,
    redo: Vec<TextBuffer>,
}

impl Default for Vim {
    fn default() -> Self {
        Self {
            mode: VimMode::Normal,
            pending: None,
            last_find: None,
            clipboard: Clipboard::default(),
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }
}

impl Vim {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mode(&self) -> VimMode {
        self.mode
    }

    pub fn set_normal(&mut self) {
        self.mode = VimMode::Normal;
        self.pending = None;
    }

    pub fn handle_key(&mut self, buffer: &mut TextBuffer, key: KeyEvent) -> bool {
        match self.mode {
            VimMode::Normal => self.handle_normal(buffer, key),
            VimMode::Insert => self.handle_insert(buffer, key),
        }
    }

    fn handle_normal(&mut self, buffer: &mut TextBuffer, key: KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return match key.code {
                KeyCode::Char('r') => {
                    self.redo(buffer);
                    true
                }
                _ => false,
            };
        }

        let code = key.code;
        if let Some(pending) = self.pending.take() {
            return match pending {
                Pending::Operator(Operator::Delete) if code == KeyCode::Char('d') => {
                    self.delete_range(buffer, buffer.current_line_range(), true);
                    true
                }
                Pending::Operator(Operator::Change) if code == KeyCode::Char('c') => {
                    self.delete_range(buffer, buffer.current_line_range(), true);
                    self.mode = VimMode::Insert;
                    true
                }
                Pending::Operator(Operator::Yank) if code == KeyCode::Char('y') => {
                    self.yank_line(buffer);
                    true
                }
                Pending::G if code == KeyCode::Char('u') => {
                    self.pending = Some(Pending::Operator(Operator::Lower));
                    true
                }
                Pending::G if code == KeyCode::Char('U') => {
                    self.pending = Some(Pending::Operator(Operator::Upper));
                    true
                }
                Pending::G if code == KeyCode::Char('~') => {
                    self.pending = Some(Pending::Operator(Operator::Toggle));
                    true
                }
                Pending::G if code == KeyCode::Char('g') => {
                    buffer.apply(EditCommand::MoveFileStart);
                    true
                }
                Pending::G if code == KeyCode::Char('_') => {
                    buffer.apply(EditCommand::MoveLastNonBlank);
                    true
                }
                Pending::G if code == KeyCode::Char('e') => {
                    buffer.apply(EditCommand::MoveWordEndBackward);
                    true
                }
                Pending::G if code == KeyCode::Char('E') => {
                    buffer.apply(EditCommand::MoveBigWordEndBackward);
                    true
                }
                Pending::Operator(Operator::Lower) if code == KeyCode::Char('u') => {
                    self.case_range(buffer, buffer.current_line_range(), CaseChange::Lower);
                    true
                }
                Pending::Operator(Operator::Upper) if code == KeyCode::Char('U') => {
                    self.case_range(buffer, buffer.current_line_range(), CaseChange::Upper);
                    true
                }
                Pending::Operator(Operator::Toggle) if code == KeyCode::Char('~') => {
                    self.case_range(buffer, buffer.current_line_range(), CaseChange::Toggle);
                    true
                }
                Pending::Operator(operator) => self.apply_operator_motion(buffer, operator, code),
                Pending::G => self.handle_normal(buffer, key),
                Pending::Find(kind) => match code {
                    KeyCode::Char(target) => {
                        self.apply_find(buffer, FindMotion { kind, target });
                        true
                    }
                    _ => false,
                },
                Pending::OperatorFind { operator, kind } => match code {
                    KeyCode::Char(target) => self
                        .range_for_find(buffer, FindMotion { kind, target })
                        .is_some_and(|range| self.apply_operator_range(buffer, operator, range)),
                    _ => false,
                },
                Pending::Replace => match code {
                    KeyCode::Char(c) => {
                        self.replace_char(buffer, c);
                        true
                    }
                    _ => false,
                },
            };
        }

        match code {
            KeyCode::Char('i') => self.mode = VimMode::Insert,
            KeyCode::Char('I') => {
                buffer.apply(EditCommand::MoveFirstNonBlank);
                self.mode = VimMode::Insert;
            }
            KeyCode::Char('a') => {
                buffer.apply(EditCommand::MoveRightInsert);
                self.mode = VimMode::Insert;
            }
            KeyCode::Char('A') => {
                buffer.apply(EditCommand::MoveLineEnd);
                buffer.apply(EditCommand::MoveRightInsert);
                self.mode = VimMode::Insert;
            }
            KeyCode::Char('o') => {
                self.record(buffer);
                buffer.apply(EditCommand::OpenLineBelow);
                self.mode = VimMode::Insert;
            }
            KeyCode::Char('O') => {
                self.record(buffer);
                buffer.apply(EditCommand::OpenLineAbove);
                self.mode = VimMode::Insert;
            }
            KeyCode::Char('h') | KeyCode::Left => buffer.apply(EditCommand::MoveLeft),
            KeyCode::Char('j') | KeyCode::Down => buffer.apply(EditCommand::MoveDown),
            KeyCode::Char('k') | KeyCode::Up => buffer.apply(EditCommand::MoveUp),
            KeyCode::Char('l') | KeyCode::Right => buffer.apply(EditCommand::MoveRight),
            KeyCode::Char('0') => buffer.apply(EditCommand::MoveLineStart),
            KeyCode::Char('^') => buffer.apply(EditCommand::MoveFirstNonBlank),
            KeyCode::Char('$') => buffer.apply(EditCommand::MoveLineEnd),
            KeyCode::Char('G') => buffer.apply(EditCommand::MoveFileEnd),
            KeyCode::Char('w') => buffer.apply(EditCommand::MoveWordForward),
            KeyCode::Char('b') => buffer.apply(EditCommand::MoveWordBackward),
            KeyCode::Char('e') => buffer.apply(EditCommand::MoveWordEnd),
            KeyCode::Char('W') => buffer.apply(EditCommand::MoveBigWordForward),
            KeyCode::Char('B') => buffer.apply(EditCommand::MoveBigWordBackward),
            KeyCode::Char('E') => buffer.apply(EditCommand::MoveBigWordEnd),
            KeyCode::Char('f') => self.pending = Some(Pending::Find(FindKind::ForwardOn)),
            KeyCode::Char('F') => self.pending = Some(Pending::Find(FindKind::BackwardOn)),
            KeyCode::Char('t') => self.pending = Some(Pending::Find(FindKind::ForwardBefore)),
            KeyCode::Char('T') => self.pending = Some(Pending::Find(FindKind::BackwardAfter)),
            KeyCode::Char(';') => self.repeat_find(buffer, false),
            KeyCode::Char(',') => self.repeat_find(buffer, true),
            KeyCode::Char('x') => self.mutate(buffer, EditCommand::DeleteChar),
            KeyCode::Char('X') => self.mutate(buffer, EditCommand::DeleteCharBefore),
            KeyCode::Char('D') => self.mutate(buffer, EditCommand::DeleteToLineEnd),
            KeyCode::Char('C') => {
                self.mutate(buffer, EditCommand::DeleteToLineEnd);
                self.mode = VimMode::Insert;
            }
            KeyCode::Char('S') => {
                self.yank_line(buffer);
                self.record(buffer);
                buffer.apply(EditCommand::ChangeLine);
                self.mode = VimMode::Insert;
            }
            KeyCode::Char('s') => {
                self.delete_range(buffer, buffer.range_char_forward(), false);
                self.mode = VimMode::Insert;
            }
            KeyCode::Char('r') => self.pending = Some(Pending::Replace),
            KeyCode::Char('Y') => self.yank_line(buffer),
            KeyCode::Char('J') => self.mutate(buffer, EditCommand::JoinLineBelow),
            KeyCode::Char('~') => self.mutate(buffer, EditCommand::ToggleCharCase),
            KeyCode::Char('p') => self.paste_after(buffer),
            KeyCode::Char('P') => self.paste_before(buffer),
            KeyCode::Char('u') => self.undo(buffer),
            KeyCode::Char('d') => self.pending = Some(Pending::Operator(Operator::Delete)),
            KeyCode::Char('c') => self.pending = Some(Pending::Operator(Operator::Change)),
            KeyCode::Char('y') => self.pending = Some(Pending::Operator(Operator::Yank)),
            KeyCode::Char('g') => self.pending = Some(Pending::G),
            _ => return false,
        }
        true
    }

    fn handle_insert(&mut self, buffer: &mut TextBuffer, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => {
                self.set_normal();
            }
            KeyCode::Char('[') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.set_normal();
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.set_normal();
            }
            KeyCode::Enter => self.mutate(buffer, EditCommand::InsertNewline),
            KeyCode::Backspace => self.mutate(buffer, EditCommand::Backspace),
            KeyCode::Delete => self.mutate(buffer, EditCommand::DeleteChar),
            KeyCode::Left => buffer.apply(EditCommand::MoveLeft),
            KeyCode::Right => buffer.apply(EditCommand::MoveRight),
            KeyCode::Up => buffer.apply(EditCommand::MoveUp),
            KeyCode::Down => buffer.apply(EditCommand::MoveDown),
            KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.mutate(buffer, EditCommand::Backspace);
            }
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.delete_insert_word(buffer);
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.mutate(buffer, EditCommand::DeleteToLineStart);
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.mutate(buffer, EditCommand::InsertChar(c));
            }
            _ => {}
        }
        true
    }

    fn mutate(&mut self, buffer: &mut TextBuffer, command: EditCommand) {
        self.record(buffer);
        buffer.apply(command);
    }

    fn record(&mut self, buffer: &TextBuffer) {
        self.undo.push(buffer.clone());
        self.redo.clear();
    }

    fn undo(&mut self, buffer: &mut TextBuffer) {
        if let Some(previous) = self.undo.pop() {
            self.redo.push(buffer.clone());
            *buffer = previous;
        }
    }

    fn redo(&mut self, buffer: &mut TextBuffer) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(buffer.clone());
            *buffer = next;
        }
    }

    fn range_for_motion(&self, buffer: &TextBuffer, code: KeyCode) -> Option<BufferRange> {
        match code {
            KeyCode::Char('w') => Some(buffer.range_to_word_forward()),
            KeyCode::Char('$') => Some(buffer.range_to_line_end()),
            KeyCode::Char('f') | KeyCode::Char('F') | KeyCode::Char('t') | KeyCode::Char('T') => {
                None
            }
            _ => None,
        }
    }

    fn apply_operator_motion(
        &mut self,
        buffer: &mut TextBuffer,
        operator: Operator,
        code: KeyCode,
    ) -> bool {
        let Some(range) = self.range_for_motion(buffer, code) else {
            if let KeyCode::Char(c @ ('f' | 'F' | 't' | 'T')) = code {
                self.pending = Some(Pending::OperatorFind {
                    operator,
                    kind: FindKind::from_command(c),
                });
                return true;
            }
            return false;
        };
        self.apply_operator_range(buffer, operator, range)
    }

    fn apply_operator_range(
        &mut self,
        buffer: &mut TextBuffer,
        operator: Operator,
        range: BufferRange,
    ) -> bool {
        match operator {
            Operator::Delete => self.delete_range(buffer, range, false),
            Operator::Change => {
                self.delete_range(buffer, range, false);
                self.mode = VimMode::Insert;
                true
            }
            Operator::Yank => self.yank_range(buffer, range, false),
            Operator::Lower => {
                self.case_range(buffer, range, CaseChange::Lower);
                true
            }
            Operator::Upper => {
                self.case_range(buffer, range, CaseChange::Upper);
                true
            }
            Operator::Toggle => {
                self.case_range(buffer, range, CaseChange::Toggle);
                true
            }
        }
    }

    fn apply_find(&mut self, buffer: &mut TextBuffer, motion: FindMotion) -> Option<usize> {
        let column = find_column(buffer.current_line(), buffer.cursor_column(), motion)?;
        buffer.move_to_char_column(column);
        self.last_find = Some(motion);
        Some(column)
    }

    fn repeat_find(&mut self, buffer: &mut TextBuffer, reverse: bool) {
        let Some(mut motion) = self.last_find else {
            return;
        };
        if reverse {
            motion.kind = motion.kind.reversed();
        }
        self.apply_find(buffer, motion);
    }

    fn range_for_find(&mut self, buffer: &TextBuffer, motion: FindMotion) -> Option<BufferRange> {
        let column = find_column(buffer.current_line(), buffer.cursor_column(), motion)?;
        self.last_find = Some(motion);
        Some(buffer.range_to_char_column(column, motion.kind.is_inclusive()))
    }

    fn replace_char(&mut self, buffer: &mut TextBuffer, c: char) {
        self.record(buffer);
        buffer.apply(EditCommand::ReplaceChar(c));
    }

    fn yank_line(&mut self, buffer: &TextBuffer) {
        self.clipboard = Clipboard {
            text: buffer.current_line().to_owned(),
            linewise: true,
        };
    }

    fn yank_range(&mut self, buffer: &TextBuffer, range: BufferRange, linewise: bool) -> bool {
        self.clipboard = Clipboard {
            text: buffer.range_text(range),
            linewise,
        };
        true
    }

    fn delete_range(
        &mut self,
        buffer: &mut TextBuffer,
        range: BufferRange,
        linewise: bool,
    ) -> bool {
        self.yank_range(buffer, range, linewise);
        self.record(buffer);
        buffer.delete_range(range);
        true
    }

    fn case_range(&mut self, buffer: &mut TextBuffer, range: BufferRange, change: CaseChange) {
        self.record(buffer);
        match change {
            CaseChange::Lower => buffer.lowercase_range(range),
            CaseChange::Upper => buffer.uppercase_range(range),
            CaseChange::Toggle => buffer.toggle_range_case(range),
        }
    }

    fn paste_after(&mut self, buffer: &mut TextBuffer) {
        if self.clipboard.text.is_empty() {
            return;
        }
        self.record(buffer);
        if self.clipboard.linewise {
            buffer.insert_line_below(self.clipboard.text.clone());
        } else {
            buffer.apply(EditCommand::MoveRightInsert);
            buffer.apply(EditCommand::InsertText(self.clipboard.text.clone()));
        }
    }

    fn paste_before(&mut self, buffer: &mut TextBuffer) {
        if self.clipboard.text.is_empty() {
            return;
        }
        self.record(buffer);
        if self.clipboard.linewise {
            buffer.insert_line_above(self.clipboard.text.clone());
        } else {
            buffer.apply(EditCommand::InsertText(self.clipboard.text.clone()));
        }
    }

    fn delete_insert_word(&mut self, buffer: &mut TextBuffer) {
        self.record(buffer);
        while buffer.char_before_cursor().is_some_and(char::is_whitespace) {
            buffer.apply(EditCommand::DeleteCharBefore);
        }
        while buffer
            .char_before_cursor()
            .is_some_and(|c| !c.is_whitespace())
        {
            buffer.apply(EditCommand::DeleteCharBefore);
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum CaseChange {
    Lower,
    Upper,
    Toggle,
}

#[derive(Debug, Clone, Copy)]
enum Pending {
    Operator(Operator),
    OperatorFind { operator: Operator, kind: FindKind },
    G,
    Find(FindKind),
    Replace,
}

#[derive(Debug, Clone, Copy)]
enum Operator {
    Delete,
    Change,
    Yank,
    Lower,
    Upper,
    Toggle,
}

#[derive(Debug, Clone, Copy)]
struct FindMotion {
    kind: FindKind,
    target: char,
}

#[derive(Debug, Clone, Copy)]
enum FindKind {
    ForwardOn,
    BackwardOn,
    ForwardBefore,
    BackwardAfter,
}

impl FindKind {
    fn from_command(c: char) -> Self {
        match c {
            'f' => Self::ForwardOn,
            'F' => Self::BackwardOn,
            't' => Self::ForwardBefore,
            'T' => Self::BackwardAfter,
            _ => unreachable!("not a find command"),
        }
    }

    fn reversed(self) -> Self {
        match self {
            Self::ForwardOn => Self::BackwardOn,
            Self::BackwardOn => Self::ForwardOn,
            Self::ForwardBefore => Self::BackwardAfter,
            Self::BackwardAfter => Self::ForwardBefore,
        }
    }

    fn is_inclusive(self) -> bool {
        true
    }
}

fn find_column(line: &str, cursor_column: usize, motion: FindMotion) -> Option<usize> {
    let chars = line.chars().collect::<Vec<_>>();
    let len = chars.len();
    match motion.kind {
        FindKind::ForwardOn => ((cursor_column + 1)..len).find(|&i| chars[i] == motion.target),
        FindKind::ForwardBefore => ((cursor_column + 1)..len)
            .find(|&i| chars[i] == motion.target)
            .map(|i| i.saturating_sub(1)),
        FindKind::BackwardOn => (0..cursor_column)
            .rev()
            .find(|&i| chars[i] == motion.target),
        FindKind::BackwardAfter => (0..cursor_column)
            .rev()
            .find(|&i| chars[i] == motion.target)
            .map(|i| (i + 1).min(len.saturating_sub(1))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_moves_deletes_pastes_and_undoes() {
        let mut vim = Vim::new();
        let mut buffer = TextBuffer::from_text("one two\nthree\n");

        vim.handle_key(&mut buffer, key('w'));
        vim.handle_key(&mut buffer, key('x'));
        assert_eq!(buffer.to_text(), "one wo\nthree\n");

        vim.handle_key(&mut buffer, key('u'));
        assert_eq!(buffer.to_text(), "one two\nthree\n");

        vim.handle_key(&mut buffer, ctrl('r'));
        assert_eq!(buffer.to_text(), "one wo\nthree\n");

        vim.handle_key(&mut buffer, key('y'));
        vim.handle_key(&mut buffer, key('y'));
        vim.handle_key(&mut buffer, key('p'));
        assert_eq!(buffer.to_text(), "one wo\none wo\nthree\n");
    }

    #[test]
    fn insert_supports_basic_control_deletes() {
        let mut vim = Vim::new();
        let mut buffer = TextBuffer::from_text("");

        vim.handle_key(&mut buffer, key('i'));
        for c in "hello world".chars() {
            vim.handle_key(&mut buffer, key(c));
        }
        vim.handle_key(&mut buffer, ctrl('w'));

        assert_eq!(buffer.to_text(), "hello ");
    }

    #[test]
    fn uppercase_companions_cover_common_normal_commands() {
        let mut vim = Vim::new();
        let mut buffer = TextBuffer::from_text("  one two\nthree four\n");

        vim.handle_key(&mut buffer, key('A'));
        vim.handle_key(&mut buffer, key('!'));
        vim.handle_key(&mut buffer, esc());
        assert_eq!(buffer.to_text(), "  one two!\nthree four\n");

        vim.handle_key(&mut buffer, key('I'));
        vim.handle_key(&mut buffer, key('#'));
        vim.handle_key(&mut buffer, esc());
        assert_eq!(buffer.to_text(), "  #one two!\nthree four\n");

        vim.handle_key(&mut buffer, key('Y'));
        vim.handle_key(&mut buffer, key('G'));
        vim.handle_key(&mut buffer, key('P'));
        assert_eq!(buffer.to_text(), "  #one two!\n  #one two!\nthree four\n");

        vim.handle_key(&mut buffer, key('J'));
        assert_eq!(buffer.to_text(), "  #one two!\n  #one two! three four\n");
    }

    #[test]
    fn case_commands_cover_char_and_line_forms() {
        let mut vim = Vim::new();
        let mut buffer = TextBuffer::from_text("AbC\nDeF\nxYz\n");

        vim.handle_key(&mut buffer, key('~'));
        assert_eq!(buffer.to_text(), "abC\nDeF\nxYz\n");

        vim.handle_key(&mut buffer, key('g'));
        vim.handle_key(&mut buffer, key('U'));
        vim.handle_key(&mut buffer, key('U'));
        assert_eq!(buffer.to_text(), "ABC\nDeF\nxYz\n");

        vim.handle_key(&mut buffer, key('j'));
        vim.handle_key(&mut buffer, key('g'));
        vim.handle_key(&mut buffer, key('u'));
        vim.handle_key(&mut buffer, key('u'));
        assert_eq!(buffer.to_text(), "ABC\ndef\nxYz\n");

        vim.handle_key(&mut buffer, key('j'));
        vim.handle_key(&mut buffer, key('g'));
        vim.handle_key(&mut buffer, key('~'));
        vim.handle_key(&mut buffer, key('~'));
        assert_eq!(buffer.to_text(), "ABC\ndef\nXyZ\n");
    }

    #[test]
    fn operators_consume_motion_ranges() {
        let mut vim = Vim::new();
        let mut buffer = TextBuffer::from_text("one two three\n");

        vim.handle_key(&mut buffer, key('d'));
        vim.handle_key(&mut buffer, key('w'));
        assert_eq!(buffer.to_text(), "two three\n");

        vim.handle_key(&mut buffer, key('c'));
        vim.handle_key(&mut buffer, key('$'));
        for c in "done".chars() {
            vim.handle_key(&mut buffer, key(c));
        }
        vim.handle_key(&mut buffer, esc());
        assert_eq!(buffer.to_text(), "done\n");
    }

    #[test]
    fn motion_ranges_support_yank_change_and_case() {
        let mut vim = Vim::new();
        let mut buffer = TextBuffer::from_text("AbC DeF\nnext\n");

        vim.handle_key(&mut buffer, key('g'));
        vim.handle_key(&mut buffer, key('u'));
        vim.handle_key(&mut buffer, key('w'));
        assert_eq!(buffer.to_text(), "abc DeF\nnext\n");

        vim.handle_key(&mut buffer, key('w'));
        vim.handle_key(&mut buffer, key('g'));
        vim.handle_key(&mut buffer, key('U'));
        vim.handle_key(&mut buffer, key('w'));
        assert_eq!(buffer.to_text(), "abc DEF\nnext\n");

        vim.handle_key(&mut buffer, key('0'));
        vim.handle_key(&mut buffer, key('y'));
        vim.handle_key(&mut buffer, key('$'));
        vim.handle_key(&mut buffer, key('G'));
        vim.handle_key(&mut buffer, key('p'));
        assert_eq!(buffer.to_text(), "abc DEF\nnextabc DEF\n");
    }

    #[test]
    fn find_motions_and_repeats_drive_normal_commands() {
        let mut vim = Vim::new();
        let mut buffer = TextBuffer::from_text("a,b,c,d\n");

        vim.handle_key(&mut buffer, key('f'));
        vim.handle_key(&mut buffer, key(','));
        vim.handle_key(&mut buffer, key('l'));

        vim.handle_key(&mut buffer, key(';'));
        vim.handle_key(&mut buffer, key('r'));
        vim.handle_key(&mut buffer, key('|'));
        assert_eq!(buffer.to_text(), "a,b|c,d\n");

        vim.handle_key(&mut buffer, key(','));
        vim.handle_key(&mut buffer, key('r'));
        vim.handle_key(&mut buffer, key('|'));
        assert_eq!(buffer.to_text(), "a|b|c,d\n");

        let mut vim = Vim::new();
        let mut buffer = TextBuffer::from_text("a,b,c,d\n");
        vim.handle_key(&mut buffer, key('t'));
        vim.handle_key(&mut buffer, key(','));
        vim.handle_key(&mut buffer, key('r'));
        vim.handle_key(&mut buffer, key('X'));
        assert_eq!(buffer.to_text(), "X,b,c,d\n");

        let mut vim = Vim::new();
        let mut buffer = TextBuffer::from_text("a,b,c\n");
        vim.handle_key(&mut buffer, key('f'));
        vim.handle_key(&mut buffer, key(','));
        vim.handle_key(&mut buffer, key('l'));
        vim.handle_key(&mut buffer, key('T'));
        vim.handle_key(&mut buffer, key(','));
        vim.handle_key(&mut buffer, key('r'));
        vim.handle_key(&mut buffer, key('Y'));
        assert_eq!(buffer.to_text(), "a,Y,c\n");
    }

    #[test]
    fn operators_consume_find_motions() {
        let mut vim = Vim::new();
        let mut buffer = TextBuffer::from_text("a,b)c=d_e;\n");

        vim.handle_key(&mut buffer, key('d'));
        vim.handle_key(&mut buffer, key('f'));
        vim.handle_key(&mut buffer, key(','));
        assert_eq!(buffer.to_text(), "b)c=d_e;\n");

        vim.handle_key(&mut buffer, key('c'));
        vim.handle_key(&mut buffer, key('t'));
        vim.handle_key(&mut buffer, key(')'));
        vim.handle_key(&mut buffer, key('X'));
        vim.handle_key(&mut buffer, esc());
        assert_eq!(buffer.to_text(), "X)c=d_e;\n");

        vim.handle_key(&mut buffer, key('f'));
        vim.handle_key(&mut buffer, key('='));
        vim.handle_key(&mut buffer, key('g'));
        vim.handle_key(&mut buffer, key('U'));
        vim.handle_key(&mut buffer, key('f'));
        vim.handle_key(&mut buffer, key('_'));
        assert_eq!(buffer.to_text(), "X)c=D_e;\n");

        vim.handle_key(&mut buffer, key('0'));
        vim.handle_key(&mut buffer, key('y'));
        vim.handle_key(&mut buffer, key('f'));
        vim.handle_key(&mut buffer, key('='));
        vim.handle_key(&mut buffer, key('G'));
        vim.handle_key(&mut buffer, key('p'));
        assert_eq!(buffer.to_text(), "X)c=D_e;X)c=\n");

        let mut vim = Vim::new();
        let mut buffer = TextBuffer::from_text("aa_bb_cc\n");
        vim.handle_key(&mut buffer, key('G'));
        vim.handle_key(&mut buffer, key('g'));
        vim.handle_key(&mut buffer, key('U'));
        vim.handle_key(&mut buffer, key('F'));
        vim.handle_key(&mut buffer, key('_'));
        assert_eq!(buffer.to_text(), "aa_bb_CC\n");

        let mut vim = Vim::new();
        let mut buffer = TextBuffer::from_text("Ab;Cd\n");
        vim.handle_key(&mut buffer, key('g'));
        vim.handle_key(&mut buffer, key('~'));
        vim.handle_key(&mut buffer, key('t'));
        vim.handle_key(&mut buffer, key(';'));
        assert_eq!(buffer.to_text(), "aB;Cd\n");
    }

    #[test]
    fn g_prefix_motions_cover_backward_word_end_and_last_nonblank() {
        let mut vim = Vim::new();
        let mut buffer = TextBuffer::from_text("abc def   \n");

        vim.handle_key(&mut buffer, key('G'));
        vim.handle_key(&mut buffer, key('g'));
        vim.handle_key(&mut buffer, key('_'));
        vim.handle_key(&mut buffer, key('r'));
        vim.handle_key(&mut buffer, key('!'));
        assert_eq!(buffer.to_text(), "abc de!   \n");

        vim.handle_key(&mut buffer, key('g'));
        vim.handle_key(&mut buffer, key('e'));
        vim.handle_key(&mut buffer, key('r'));
        vim.handle_key(&mut buffer, key('?'));
        assert_eq!(buffer.to_text(), "abc d?!   \n");

        let mut vim = Vim::new();
        let mut buffer = TextBuffer::from_text("abc def ghi \n");
        vim.handle_key(&mut buffer, key('G'));
        vim.handle_key(&mut buffer, key('g'));
        vim.handle_key(&mut buffer, key('E'));
        vim.handle_key(&mut buffer, key('r'));
        vim.handle_key(&mut buffer, key('?'));
        assert_eq!(buffer.to_text(), "abc def gh? \n");
    }

    #[test]
    fn substitute_replace_and_insert_exit_aliases_work() {
        let mut vim = Vim::new();
        let mut buffer = TextBuffer::from_text("abc\n");

        vim.handle_key(&mut buffer, key('s'));
        vim.handle_key(&mut buffer, key('X'));
        vim.handle_key(&mut buffer, ctrl('['));
        assert_eq!(buffer.to_text(), "Xbc\n");

        vim.handle_key(&mut buffer, key('0'));
        vim.handle_key(&mut buffer, key('r'));
        vim.handle_key(&mut buffer, key('Y'));
        assert_eq!(buffer.to_text(), "Ybc\n");

        vim.handle_key(&mut buffer, key('A'));
        vim.handle_key(&mut buffer, key('!'));
        vim.handle_key(&mut buffer, ctrl('c'));
        vim.handle_key(&mut buffer, key('u'));
        assert_eq!(buffer.to_text(), "Ybc\n");
    }

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn esc() -> KeyEvent {
        KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)
    }
}
