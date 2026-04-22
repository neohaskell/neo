use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};
use crate::theme::Theme;
use crate::tui::mascot::Mascot;

pub struct SuccessDisplay<'a> {
    theme: &'a Theme,
    message: &'a str,
}

impl<'a> SuccessDisplay<'a> {
    pub fn new(theme: &'a Theme, message: &'a str) -> Self {
        Self { theme, message }
    }
}

impl<'a> Widget for SuccessDisplay<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(40), // Mascot area
                Constraint::Min(0),      // Message area
            ])
            .split(area);

        // Render Mascot
        let mascot = Mascot::new(self.theme);
        mascot.render(chunks[0], buf);

        let msg_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Padding
                Constraint::Length(1), // Message
                Constraint::Min(0),
            ])
            .split(chunks[1]);

        let success_line = Line::from(vec![
            Span::styled("✓ ", self.theme.style_success()),
            Span::styled(self.message, self.theme.style_success()),
        ]);
        
        Paragraph::new(success_line).render(msg_chunks[1], buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    #[test]
    fn test_success_display_render() {
        let theme = Theme::neo();
        let widget = SuccessDisplay::new(&theme, "Build succeeded!");
        let area = Rect::new(0, 0, 80, 10);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        
        let content = buf.content().iter().map(|c| c.symbol()).collect::<String>();
        assert!(content.contains("✓"));
        assert!(content.contains("Build succeeded!"));
    }
}
