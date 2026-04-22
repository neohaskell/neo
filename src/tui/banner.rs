use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::Line,
    widgets::{Paragraph, Widget},
    buffer::Buffer,
};
use crate::theme::Theme;
use crate::tui::mascot::Mascot;

pub struct Banner<'a> {
    pub theme: &'a Theme,
    pub title: &'a str,
    pub subtitle: &'a str,
}

impl<'a> Banner<'a> {
    pub fn new(theme: &'a Theme, title: &'a str, subtitle: &'a str) -> Self {
        Self { theme, title, subtitle }
    }
}

impl<'a> Widget for Banner<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(40), // Mascot area
                Constraint::Min(0),      // Title area
            ])
            .split(area);

        // Render Mascot
        let mascot = Mascot::new(self.theme);
        mascot.render(chunks[0], buf);

        // Render Title
        let title_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Length(1), // Subtitle
            ])
            .split(chunks[1]);

        let title = Paragraph::new(Line::from(self.title))
            .style(self.theme.style_primary().add_modifier(ratatui::style::Modifier::BOLD));
        title.render(title_chunks[0], buf);
        
        let subtitle = Paragraph::new(Line::from(self.subtitle))
            .style(self.theme.style_muted());
        subtitle.render(title_chunks[1], buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    #[test]
    fn test_banner_render() {
        let theme = Theme::neo();
        let widget = Banner::new(&theme, "NEO", "Subtitle");
        let area = Rect::new(0, 0, 80, 10);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        
        let content = buf.content().iter().map(|c| c.symbol()).collect::<String>();
        assert!(content.contains("NEO"));
        assert!(content.contains("Subtitle"));
    }
}
