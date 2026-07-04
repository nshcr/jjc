use std::io;
use std::path::Path;

use crossterm::cursor::SetCursorStyle;
use crossterm::execute;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;

use crate::config::AppConfig;
use crate::syntax;
use crate::syntax::HighlightSpan;
use crate::vim::VimMode;

pub struct StyledText<'a> {
    lines: &'a [String],
    config: &'a AppConfig,
    highlighted: Option<Vec<Vec<HighlightSpan>>>,
}

impl<'a> StyledText<'a> {
    pub fn new(path: &Path, lines: &'a [String], config: &'a AppConfig) -> Self {
        let highlighted = config
            .syntax
            .enabled
            .then(|| syntax::highlight_lines(path, lines))
            .flatten();
        Self {
            lines,
            config,
            highlighted,
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
        if line.starts_with("JJ:") {
            return Line::from(Span::styled(
                line.to_owned(),
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
        let mut spans = vec![Span::raw(prefix.into())];
        let Some(highlighted) = self.highlighted.as_ref().and_then(|lines| lines.get(index)) else {
            spans.push(Span::raw(line.to_owned()));
            return Line::from(spans);
        };
        spans.extend(highlighted.iter().map(|span| match span.class {
            Some(class) => Span::styled(span.text.clone(), self.config.theme.style(class)),
            None => Span::raw(span.text.clone()),
        }));
        Line::from(spans)
    }
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
        let line = StyledText::new(Path::new("COMMIT_EDITMSG"), &lines, &config).lines()[0].clone();

        assert!(line.spans[0].style.add_modifier.contains(Modifier::DIM));
    }

    #[test]
    fn vim_cursor_style_follows_mode() {
        let mut output = Vec::new();

        set_vim_cursor_style(&mut output, VimMode::Normal).unwrap();
        set_vim_cursor_style(&mut output, VimMode::Insert).unwrap();

        assert_eq!(output, b"\x1b[2 q\x1b[6 q");
    }
}
