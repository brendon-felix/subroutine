use ratatui::style::{Color, Modifier, Style};

/// Theme for the TUI application
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    /// Background color
    pub background: Color,
    /// Text color for normal text
    pub text: Color,
    /// Text color for dimmed/secondary text
    pub text_dimmed: Color,
    /// Color for borders
    pub border: Color,
    /// Color for action items
    pub action: Color,
    /// Color for event items
    pub event: Color,
    /// Color for selected items
    pub selected: Color,
    /// Color for input mode borders
    pub input_border: Color,
    /// Color for help text
    pub help: Color,
    /// Color for empty state text
    pub empty: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Color::Rgb(25, 25, 25),
            text: Color::White,
            text_dimmed: Color::DarkGray,
            border: Color::DarkGray,
            action: Color::Green,
            event: Color::Cyan,
            selected: Color::White,
            input_border: Color::DarkGray,
            help: Color::DarkGray,
            empty: Color::DarkGray,
        }
    }
}

impl Theme {
    /// Get the default theme
    pub fn default_theme() -> Self {
        Self::default()
    }

    /// Style for normal text
    pub fn text_style(&self) -> Style {
        Style::default().fg(self.text)
    }

    /// Style for dimmed/secondary text
    pub fn dimmed_style(&self) -> Style {
        Style::default().fg(self.text_dimmed)
    }

    /// Style for selected items
    pub fn selected_style(&self) -> Style {
        Style::default()
            .fg(self.selected)
            .add_modifier(Modifier::BOLD)
    }

    /// Style for action items
    pub fn action_style(&self, selected: bool) -> Style {
        if selected {
            Style::default()
                .fg(self.selected)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(self.action)
        }
    }

    /// Style for event items
    pub fn event_style(&self, selected: bool) -> Style {
        if selected {
            Style::default()
                .fg(self.selected)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(self.event)
        }
    }

    /// Style for borders
    pub fn border_style(&self) -> Style {
        Style::default().fg(self.border)
    }

    /// Style for action borders
    pub fn action_border_style(&self, selected: bool) -> Style {
        if selected {
            Style::default().fg(self.action)
        } else {
            Style::default().fg(self.border).dim()
        }
    }

    /// Style for event borders
    pub fn event_border_style(&self, selected: bool) -> Style {
        if selected {
            Style::default().fg(self.event)
        } else {
            Style::default().fg(self.border)
        }
    }

    /// Style for help text
    pub fn help_style(&self) -> Style {
        Style::default().fg(self.help)
    }

    /// Style for empty state text
    pub fn empty_style(&self) -> Style {
        Style::default().fg(self.empty)
    }
}

/// Input mode styling
#[derive(Debug, Clone, Copy)]
pub enum InputMode {
    Normal,
    InsertAction,
    InsertEvent,
}

impl InputMode {
    pub fn is_insert(&self) -> bool {
        matches!(self, Self::InsertAction | Self::InsertEvent)
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::InsertAction => " New action: ",
            Self::InsertEvent => " New event: ",
            Self::Normal => "",
        }
    }

    pub fn border_color(&self, theme: &Theme) -> Color {
        match self {
            Self::InsertAction => theme.action,
            Self::InsertEvent => theme.event,
            Self::Normal => theme.input_border,
        }
    }

    pub fn border_style(&self, theme: &Theme) -> Style {
        Style::default().fg(self.border_color(theme))
    }
}
