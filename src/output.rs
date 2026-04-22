use ratatui::DefaultTerminal;

pub enum OutputMode {
    Interactive { terminal: DefaultTerminal },
    Ci,
}

impl OutputMode {
    pub fn is_ci(&self) -> bool {
        matches!(self, Self::Ci)
    }
}
