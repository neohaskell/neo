use ratatui::{
    layout::Rect,
    text::Text,
    widgets::{Paragraph, Widget},
};
use crate::theme::Theme;

pub struct Mascot<'a> {
    theme: &'a Theme,
}

impl<'a> Mascot<'a> {
    pub fn new(theme: &'a Theme) -> Self {
        Self { theme }
    }
}

impl<'a> Widget for Mascot<'a> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let ascii_art = include_str!("../../assets/neo_mascot.txt");
        let text = Text::styled(ascii_art, self.theme.style_primary());
        Paragraph::new(text).render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    #[test]
    fn test_mascot_render() {
        let theme = Theme::neo();
        let widget = Mascot::new(&theme);
        let area = Rect::new(0, 0, 80, 20);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        
        // Check that something was rendered (non-empty cells)
        let non_empty = buf.content().iter().filter(|c| c.symbol() != " ").count();
        assert!(non_empty > 0);
    }
}
