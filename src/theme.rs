use ratatui::style::{Color, Modifier, Style};

pub struct Theme {
    pub primary: Color,
    pub success: Color,
    pub error: Color,
    pub warning: Color,
    pub info: Color,
    pub muted: Color,
    pub text: Color,
    pub bg: Color,
    pub accent: Color,
}

impl Theme {
    pub fn neo() -> Self {
        Self {
            primary: Color::from_u32(0x0050E0D0), // Teal
            success: Color::from_u32(0x0066D96E), // Green
            error: Color::from_u32(0x00FF6B6B),   // Soft red
            warning: Color::from_u32(0x00FFD93D), // Amber
            info: Color::from_u32(0x006BC5F0),    // Sky blue
            muted: Color::from_u32(0x006C757D),   // Gray
            text: Color::from_u32(0x00F8F9FA),    // Near-white
            bg: Color::Reset,
            accent: Color::from_u32(0x00BB86FC),  // Purple
        }
    }

    pub fn style_error(&self) -> Style {
        Style::default().fg(self.error).add_modifier(Modifier::BOLD)
    }

    pub fn style_success(&self) -> Style {
        Style::default().fg(self.success).add_modifier(Modifier::BOLD)
    }

    pub fn style_warning(&self) -> Style {
        Style::default().fg(self.warning)
    }

    pub fn style_primary(&self) -> Style {
        Style::default().fg(self.primary)
    }

    pub fn style_text(&self) -> Style {
        Style::default().fg(self.text)
    }

    pub fn style_muted(&self) -> Style {
        Style::default().fg(self.muted)
    }

    pub fn style_accent(&self) -> Style {
        Style::default().fg(self.accent)
    }
}
