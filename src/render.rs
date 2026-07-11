use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::io;
use std::path::Path;

use crossterm::cursor::SetCursorStyle;
use crossterm::execute;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::config::AppConfig;
use crate::syntax;
use crate::syntax::HighlightSpan;
use crate::vim::VimMode;

pub const TAB_WIDTH: usize = 4;
pub const MAX_HIGHLIGHT_BYTES: usize = 512 * 1024;

#[derive(Default)]
pub struct StyledTextCache {
    fingerprint: Option<u64>,
    rendered: Vec<Line<'static>>,
    #[cfg(test)]
    rebuilds: usize,
}

impl StyledTextCache {
    pub fn lines(
        &mut self,
        path: &Path,
        lines: &[String],
        config: &AppConfig,
    ) -> Vec<Line<'static>> {
        self.lines_with_jj_instructions(path, lines, config, false)
    }

    pub fn lines_with_jj_instructions(
        &mut self,
        path: &Path,
        lines: &[String],
        config: &AppConfig,
        dim_jj_instructions: bool,
    ) -> Vec<Line<'static>> {
        let fingerprint = content_fingerprint(path, lines, config, dim_jj_instructions);
        if self.fingerprint != Some(fingerprint) {
            self.rendered =
                StyledText::with_jj_instructions(path, lines, config, dim_jj_instructions).lines();
            self.fingerprint = Some(fingerprint);
            #[cfg(test)]
            {
                self.rebuilds += 1;
            }
        }
        self.rendered.clone()
    }

    pub fn text(&mut self, path: &Path, text: &str, config: &AppConfig) -> Vec<Line<'static>> {
        let fingerprint = text_fingerprint(path, text, config);
        if self.fingerprint != Some(fingerprint) {
            let lines = string_lines(text);
            self.rendered = StyledText::new(path, &lines, config).lines();
            self.fingerprint = Some(fingerprint);
            #[cfg(test)]
            {
                self.rebuilds += 1;
            }
        }
        self.rendered.clone()
    }

    #[cfg(test)]
    fn rebuilds(&self) -> usize {
        self.rebuilds
    }
}

pub struct StyledText<'a> {
    lines: &'a [String],
    config: &'a AppConfig,
    highlighted: Option<Vec<Vec<HighlightSpan>>>,
    dim_jj_instructions: bool,
}

impl<'a> StyledText<'a> {
    pub fn new(path: &Path, lines: &'a [String], config: &'a AppConfig) -> Self {
        Self::with_jj_instructions(path, lines, config, false)
    }

    pub fn with_jj_instructions(
        path: &Path,
        lines: &'a [String],
        config: &'a AppConfig,
        dim_jj_instructions: bool,
    ) -> Self {
        let highlighted = (config.syntax.enabled && !is_large_content(lines))
            .then(|| syntax::highlight_lines(path, lines))
            .flatten();
        Self {
            lines,
            config,
            highlighted,
            dim_jj_instructions,
        }
    }

    pub fn lines(&self) -> Vec<Line<'static>> {
        self.lines
            .iter()
            .enumerate()
            .map(|(index, line)| self.line(index, line))
            .collect()
    }

    pub fn line(&self, index: usize, line: &str) -> Line<'static> {
        if self.dim_jj_instructions && line.starts_with("JJ:") {
            return Line::from(Span::styled(
                expand_tabs(line, TAB_WIDTH),
                Style::new().add_modifier(Modifier::DIM),
            ));
        }
        self.line_with_prefix(index, "", line)
    }

    pub fn line_with_prefix(
        &self,
        index: usize,
        prefix: impl Into<String>,
        line: &str,
    ) -> Line<'static> {
        let prefix = prefix.into();
        let mut column = 0;
        let mut spans = vec![Span::raw(expand_tabs_from(&prefix, TAB_WIDTH, &mut column))];
        let Some(highlighted) = self.highlighted.as_ref().and_then(|lines| lines.get(index)) else {
            spans.push(Span::raw(expand_tabs_from(line, TAB_WIDTH, &mut column)));
            return Line::from(spans);
        };
        spans.extend(highlighted.iter().map(|span| {
            let text = expand_tabs_from(&span.text, TAB_WIDTH, &mut column);
            match span.class {
                Some(class) => Span::styled(text, self.config.theme.style(class)),
                None => Span::raw(text),
            }
        }));
        Line::from(spans)
    }
}

pub fn display_width(text: &str, tab_width: usize) -> usize {
    let mut column = 0;
    let _ = expand_tabs_from(text, tab_width, &mut column);
    column
}

pub fn display_boundary_at_or_after(text: &str, target: usize, tab_width: usize) -> usize {
    let tab_width = tab_width.max(1);
    let mut column = 0;
    for grapheme in text.graphemes(true) {
        if column >= target {
            return column;
        }
        let width = if grapheme == "\t" {
            tab_width - (column % tab_width)
        } else {
            UnicodeWidthStr::width(grapheme)
        };
        let next = column + width;
        if target < next {
            return next;
        }
        column = next;
    }
    column
}

pub fn expand_tabs(text: &str, tab_width: usize) -> String {
    let mut column = 0;
    expand_tabs_from(text, tab_width, &mut column)
}

fn expand_tabs_from(text: &str, tab_width: usize, column: &mut usize) -> String {
    let tab_width = tab_width.max(1);
    let mut expanded = String::with_capacity(text.len());
    for grapheme in text.graphemes(true) {
        if grapheme == "\t" {
            let spaces = tab_width - (*column % tab_width);
            expanded.extend(std::iter::repeat_n(' ', spaces));
            *column += spaces;
        } else {
            expanded.push_str(grapheme);
            *column += UnicodeWidthStr::width(grapheme);
        }
    }
    expanded
}

fn content_fingerprint(
    path: &Path,
    lines: &[String],
    config: &AppConfig,
    dim_jj_instructions: bool,
) -> u64 {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    lines.hash(&mut hasher);
    format!("{config:?}").hash(&mut hasher);
    dim_jj_instructions.hash(&mut hasher);
    hasher.finish()
}

fn text_fingerprint(path: &Path, text: &str, config: &AppConfig) -> u64 {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    text.hash(&mut hasher);
    format!("{config:?}").hash(&mut hasher);
    hasher.finish()
}

pub fn is_large_content(lines: &[String]) -> bool {
    lines
        .iter()
        .map(|line| line.len().saturating_add(1))
        .sum::<usize>()
        > MAX_HIGHLIGHT_BYTES
}

pub fn string_lines(text: &str) -> Vec<String> {
    text.lines().map(str::to_owned).collect()
}

pub fn set_vim_cursor_style(writer: &mut impl io::Write, mode: VimMode) -> io::Result<()> {
    let style = match mode {
        VimMode::Normal => SetCursorStyle::SteadyBlock,
        VimMode::Insert => SetCursorStyle::SteadyBar,
    };
    execute!(writer, style)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styled_text_preserves_plain_text_when_disabled() {
        let config = AppConfig {
            syntax: crate::config::SyntaxConfig { enabled: false },
            theme: crate::theme::Theme::default(),
        };
        let lines = vec!["fn main() {}".to_owned()];
        let styled = StyledText::new(Path::new("main.rs"), &lines, &config);

        assert_eq!(styled.lines()[0].to_string(), "fn main() {}");
    }

    #[test]
    fn styled_text_keeps_prefix_outside_highlighted_source() {
        let config = AppConfig::default();
        let lines = vec!["fn main() {}".to_owned()];
        let styled = StyledText::new(Path::new("main.rs"), &lines, &config);

        assert_eq!(
            styled.line_with_prefix(0, "+", &lines[0]).to_string(),
            "+fn main() {}"
        );
    }

    #[test]
    fn styled_text_dims_jj_comment_lines() {
        let config = AppConfig::default();
        let lines = vec!["JJ: keep this exact comment".to_owned()];
        let line = StyledText::with_jj_instructions(
            Path::new("message.jjdescription"),
            &lines,
            &config,
            true,
        )
        .lines()[0]
            .clone();

        assert!(line.spans[0].style.add_modifier.contains(Modifier::DIM));
    }

    #[test]
    fn generic_jj_prefix_is_not_dimmed() {
        let config = AppConfig::default();
        let lines = vec!["JJ: ordinary generic text".to_owned()];
        let line = StyledText::new(Path::new("notes.txt"), &lines, &config).lines()[0].clone();

        assert!(!line.spans[0].style.add_modifier.contains(Modifier::DIM));
    }

    #[test]
    fn vim_cursor_style_follows_mode() {
        let mut output = Vec::new();

        set_vim_cursor_style(&mut output, VimMode::Normal).unwrap();
        set_vim_cursor_style(&mut output, VimMode::Insert).unwrap();

        assert_eq!(output, b"\x1b[2 q\x1b[6 q");
    }

    #[test]
    fn display_width_and_tab_expansion_follow_terminal_cells() {
        assert_eq!(display_width("a中\tb", TAB_WIDTH), 5);
        assert_eq!(expand_tabs("a中\tb", TAB_WIDTH), "a中 b");
        assert_eq!(display_width("e\u{301}", TAB_WIDTH), 1);
        assert_eq!(display_width("👩‍💻", TAB_WIDTH), 2);
        assert_eq!(display_boundary_at_or_after("中中", 1, TAB_WIDTH), 2);
        assert_eq!(display_boundary_at_or_after("中中", 2, TAB_WIDTH), 2);
        assert_eq!(display_boundary_at_or_after("a\tb", 2, TAB_WIDTH), 4);
    }

    #[test]
    fn styled_text_cache_rebuilds_only_after_content_changes() {
        let config = AppConfig::default();
        let mut cache = StyledTextCache::default();
        let mut lines = vec!["fn main() {}".to_owned()];

        cache.lines(Path::new("main.rs"), &lines, &config);
        cache.lines(Path::new("main.rs"), &lines, &config);
        assert_eq!(cache.rebuilds(), 1);

        lines[0].push_str(" // changed");
        cache.lines(Path::new("main.rs"), &lines, &config);
        assert_eq!(cache.rebuilds(), 2);

        let mut config = config;
        config.syntax.enabled = false;
        cache.lines(Path::new("main.rs"), &lines, &config);
        assert_eq!(cache.rebuilds(), 3);
    }

    #[test]
    fn large_content_uses_plain_text_fallback() {
        let config = AppConfig::default();
        let lines = vec!["x".repeat(MAX_HIGHLIGHT_BYTES + 1)];
        let styled = StyledText::new(Path::new("large.rs"), &lines, &config);

        assert!(is_large_content(&lines));
        assert!(styled.highlighted.is_none());
    }
}
