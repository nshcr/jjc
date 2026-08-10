use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::BorderType;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;

pub fn status_line(
    mode: impl Into<String>,
    context: impl Into<String>,
    actions: impl Into<String>,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!(" {} ", mode.into()),
            Style::new()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(" {}  ", context.into())),
        Span::styled(actions.into(), Style::new().fg(Color::DarkGray)),
    ])
}

pub fn render_help(frame: &mut Frame<'_>, title: &str, body: &str) {
    let area = help_area(frame.area(), body.lines().count());
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(body.to_owned())
            .wrap(Wrap { trim: false })
            .block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .border_style(Style::new().fg(Color::Cyan))
                    .title(format!(" {title} "))
                    .title_style(Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ),
        area,
    );
}

fn help_area(outer: Rect, body_lines: usize) -> Rect {
    let width = if outer.width <= 2 {
        outer.width
    } else {
        outer.width.saturating_sub(2).min(88)
    };
    let desired_height = u16::try_from(body_lines)
        .unwrap_or(u16::MAX)
        .saturating_add(2);
    let available_height = if outer.height <= 2 {
        outer.height
    } else {
        outer.height.saturating_sub(2)
    };
    let height = desired_height.min(available_height);
    Rect::new(
        outer.x + outer.width.saturating_sub(width) / 2,
        outer.y + outer.height.saturating_sub(height) / 2,
        width,
        height,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_area_is_centered_and_bounded() {
        let outer = Rect::new(3, 5, 100, 24);
        let area = help_area(outer, 8);

        assert_eq!(area.width, 88);
        assert_eq!(area.height, 10);
        assert_eq!(area.x, 9);
        assert_eq!(area.y, 12);
        assert!(area.right() <= outer.right());
        assert!(area.bottom() <= outer.bottom());

        for tiny in [Rect::new(0, 0, 0, 0), Rect::new(2, 3, 1, 1)] {
            let area = help_area(tiny, 8);
            assert!(area.right() <= tiny.right());
            assert!(area.bottom() <= tiny.bottom());
        }
    }
}
