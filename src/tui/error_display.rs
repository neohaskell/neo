use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Paragraph, Widget, Wrap},
};
use crate::theme::Theme;
use crate::tui::mascot::Mascot;

pub struct ErrorDisplay<'a> {
    theme: &'a Theme,
    error: &'a str,
    help: Option<&'a str>,
}

impl<'a> ErrorDisplay<'a> {
    pub fn new(theme: &'a Theme, error: &'a str) -> Self {
        Self { theme, error, help: None }
    }

    pub fn with_help(mut self, help: &'a str) -> Self {
        self.help = Some(help);
        self
    }
}

impl<'a> Widget for ErrorDisplay<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(40), // Mascot area
                Constraint::Min(0),      // Error area
            ])
            .split(area);

        // Render Mascot
        let mascot = Mascot::new(self.theme);
        mascot.render(chunks[0], buf);

        let error_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Padding
                Constraint::Length(2), // Error message (wrapped)
                Constraint::Length(1), // Spacer
                Constraint::Min(0),    // Help
            ])
            .split(chunks[1]);

        let error_line = Line::from(vec![
            Span::styled("✗ ", self.theme.style_error()),
            Span::styled(self.error, self.theme.style_error()),
        ]);
        
        Paragraph::new(error_line)
            .wrap(Wrap { trim: true })
            .render(error_chunks[1], buf);

        if let Some(help) = self.help {
            let help_text = format!("help: {}", help);
            let help_line = Line::from(Span::styled(help_text, self.theme.style_muted()));
            Paragraph::new(help_line)
                .wrap(Wrap { trim: true })
                .render(error_chunks[3], buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    #[test]
    fn test_error_display_render() {
        let theme = Theme::neo();
        let widget = ErrorDisplay::new(&theme, "Build failed!").with_help("Check your code.");
        let area = Rect::new(0, 0, 80, 10);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        
        let content = buf.content().iter().map(|c| c.symbol()).collect::<String>();
        assert!(content.contains("✗"));
        assert!(content.contains("Build failed!"));
        assert!(content.contains("help: Check your code."));
    }
}
