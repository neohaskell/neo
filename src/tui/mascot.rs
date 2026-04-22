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
