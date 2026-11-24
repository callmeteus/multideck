//! Visual language of the desktop app_
//!
//! Colors and styles live here so screens do not invent their own palette.

use iced::widget::button::{self, Status};
use iced::widget::container;
use iced::{Background, Border, Color, Shadow, Theme, Vector};

/// Brand colors, shared by the app, the icon set and the documentation site.
pub struct Colors;

impl Colors {
    /// Window background.
    pub const BACKGROUND: Color = Color::from_rgb(0.055, 0.063, 0.078);
    /// Side panels and cards.
    pub const PANEL: Color = Color::from_rgb(0.086, 0.098, 0.122);
    /// Raised surfaces.
    pub const SURFACE: Color = Color::from_rgb(0.118, 0.133, 0.165);
    /// Hairline borders.
    pub const BORDER: Color = Color::from_rgb(0.180, 0.200, 0.243);
    /// Primary text.
    pub const TEXT: Color = Color::from_rgb(0.930, 0.945, 0.965);
    /// Secondary text.
    pub const MUTED: Color = Color::from_rgb(0.560, 0.596, 0.655);
    /// Brand accent.
    pub const ACCENT: Color = Color::from_rgb(0.360, 0.545, 1.000);
    /// Positive feedback.
    pub const SUCCESS: Color = Color::from_rgb(0.376, 0.847, 0.627);
}

/// Base iced theme used by the app_
pub fn theme() -> Theme {
    Theme::Dark
}

/// Applies the window background.
pub fn page() -> impl Fn(&Theme) -> container::Style {
    |_theme| container::Style {
        background: Some(Background::Color(Colors::BACKGROUND)),
        ..Default::default()
    }
}

/// Applies a side panel background.
pub fn panel() -> impl Fn(&Theme) -> container::Style {
    |_theme| container::Style {
        background: Some(Background::Color(Colors::PANEL)),
        border: Border {
            color: Colors::BORDER,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

/// Applies a soft card background.
pub fn card() -> impl Fn(&Theme) -> container::Style {
    |_theme| container::Style {
        background: Some(Background::Color(Colors::SURFACE)),
        border: Border {
            color: Colors::BORDER,
            width: 1.0,
            radius: 12.0.into(),
        },
        ..Default::default()
    }
}

/// Frame around a key, highlighted when selected.
pub fn key_frame(selected: bool) -> impl Fn(&Theme) -> container::Style {
    move |_theme| container::Style {
        background: Some(Background::Color(if selected {
            Colors::ACCENT
        } else {
            Color::TRANSPARENT
        })),
        border: Border {
            color: if selected {
                Colors::ACCENT
            } else {
                Colors::BORDER
            },
            width: if selected { 2.0 } else { 1.0 },
            radius: 12.0.into(),
        },
        ..Default::default()
    }
}

/// Button that only draws its content, used for key images.
pub fn bare_button() -> impl Fn(&Theme, Status) -> button::Style {
    |_theme, _status| button::Style {
        background: None,
        text_color: Colors::TEXT,
        border: Border::default(),
        shadow: Shadow::default(),
        snap: true,
    }
}

/// Low emphasis button used by the palette and the toolbar.
pub fn ghost_button() -> impl Fn(&Theme, Status) -> button::Style {
    |_theme, status| button::Style {
        background: Some(Background::Color(match status {
            Status::Hovered => Colors::BORDER,
            Status::Pressed => Colors::ACCENT,
            _ => Colors::SURFACE,
        })),
        text_color: Colors::TEXT,
        border: Border {
            color: Colors::BORDER,
            width: 1.0,
            radius: 10.0.into(),
        },
        shadow: Shadow::default(),
        snap: true,
    }
}

/// High emphasis button for the primary action of a screen.
pub fn primary_button() -> impl Fn(&Theme, Status) -> button::Style {
    |_theme, status| button::Style {
        background: Some(Background::Color(match status {
            Status::Hovered => Color::from_rgb(0.44, 0.61, 1.0),
            Status::Pressed => Color::from_rgb(0.28, 0.44, 0.86),
            _ => Colors::ACCENT,
        })),
        text_color: Color::WHITE,
        border: Border {
            color: Colors::ACCENT,
            width: 1.0,
            radius: 10.0.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.25),
            offset: Vector::new(0.0, 2.0),
            blur_radius: 8.0,
        },
        snap: true,
    }
}

/// Toggle style for tabs, pages and language chips.
pub fn toggle_button(active: bool) -> impl Fn(&Theme, Status) -> button::Style {
    move |_theme, status| {
        let base = if active {
            Colors::ACCENT
        } else {
            Colors::SURFACE
        };
        let background = match status {
            Status::Hovered => {
                if active {
                    Color::from_rgb(0.44, 0.61, 1.0)
                } else {
                    Colors::BORDER
                }
            }
            _ => base,
        };
        button::Style {
            background: Some(Background::Color(background)),
            text_color: if active { Color::WHITE } else { Colors::MUTED },
            border: Border {
                color: if active {
                    Colors::ACCENT
                } else {
                    Colors::BORDER
                },
                width: 1.0,
                radius: 999.0.into(),
            },
            shadow: Shadow::default(),
            snap: true,
        }
    }
}
