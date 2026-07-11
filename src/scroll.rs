use ratatui::layout::Margin;
use ratatui::layout::Rect;
use ratatui::widgets::ScrollbarState;

#[derive(Default)]
pub struct ViewScroll {
    offset: usize,
    horizontal_offset: usize,
}

impl ViewScroll {
    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn horizontal_offset(&self) -> usize {
        self.horizontal_offset
    }

    pub fn set_horizontal_offset(&mut self, offset: usize) {
        self.horizontal_offset = offset;
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

    pub fn keep_column_visible(
        &mut self,
        cursor: usize,
        content_width: usize,
        viewport_width: usize,
    ) {
        let viewport_width = viewport_width.max(1);
        let cursor = cursor.min(content_width);
        let scrollable_width = content_width.max(cursor.saturating_add(1));
        if cursor < self.horizontal_offset {
            self.horizontal_offset = cursor;
        } else if cursor >= self.horizontal_offset + viewport_width {
            self.horizontal_offset = cursor + 1 - viewport_width;
        }
        self.horizontal_offset = self
            .horizontal_offset
            .min(scrollable_width.saturating_sub(viewport_width));
    }

    pub fn visible_column(&self, cursor: usize, viewport_width: usize) -> usize {
        cursor
            .saturating_sub(self.horizontal_offset)
            .min(viewport_width.saturating_sub(1))
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

pub fn terminal_offset(offset: usize) -> u16 {
    offset.min(u16::MAX as usize) as u16
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

    #[test]
    fn keeps_wide_cursor_visible_inside_horizontal_viewport() {
        let mut scroll = ViewScroll::default();
        scroll.keep_column_visible(12, 20, 5);
        assert_eq!(scroll.horizontal_offset(), 8);
        assert_eq!(scroll.visible_column(12, 5), 4);

        scroll.keep_column_visible(3, 20, 5);
        assert_eq!(scroll.horizontal_offset(), 3);
    }

    #[test]
    fn keeps_insert_cursor_visible_after_full_width_content() {
        let mut scroll = ViewScroll::default();

        scroll.keep_column_visible(5, 5, 5);

        assert_eq!(scroll.horizontal_offset(), 1);
        assert_eq!(scroll.visible_column(5, 5), 4);

        let mut normal_scroll = ViewScroll::default();
        normal_scroll.keep_column_visible(4, 5, 5);
        assert_eq!(normal_scroll.horizontal_offset(), 0);
        assert_eq!(normal_scroll.visible_column(4, 5), 4);
        assert_eq!(terminal_offset(usize::MAX), u16::MAX);
    }
}
