use ratatui::layout::Margin;
use ratatui::layout::Rect;
use ratatui::widgets::ScrollbarState;

#[derive(Default)]
pub struct ViewScroll {
    offset: usize,
}

impl ViewScroll {
    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn keep_visible(&mut self, cursor: usize, content_len: usize, viewport_len: usize) {
        let viewport_len = viewport_len.max(1);
        let cursor = cursor.min(content_len.saturating_sub(1));
        if cursor < self.offset {
            self.offset = cursor;
        } else if cursor >= self.offset + viewport_len {
            self.offset = cursor + 1 - viewport_len;
        }
        self.offset = self.offset.min(content_len.saturating_sub(viewport_len));
    }

    pub fn visible_line(&self, cursor: usize, viewport_len: usize) -> usize {
        cursor
            .saturating_sub(self.offset)
            .min(viewport_len.saturating_sub(1))
    }

    pub fn scrollbar_state(&self, content_len: usize, viewport_len: usize) -> ScrollbarState {
        ScrollbarState::new(content_len)
            .position(self.offset)
            .viewport_content_length(viewport_len)
    }
}

pub fn scrollbar_area(area: Rect) -> Rect {
    area.inner(Margin {
        vertical: 1,
        horizontal: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_cursor_visible_inside_viewport() {
        let mut scroll = ViewScroll::default();
        scroll.keep_visible(9, 20, 5);
        assert_eq!(scroll.offset(), 5);

        scroll.keep_visible(3, 20, 5);
        assert_eq!(scroll.offset(), 3);
    }
}
