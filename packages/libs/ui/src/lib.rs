//! Visual language of the desktop app_
//!
//! Colors and styles live here so screens do not invent their own palette.

use iced::widget::button::{self, Status};
use iced::widget::container;
use iced::widget::scrollable;
use iced::widget::text_input;
use iced::{Background, Border, Color, Shadow, Theme, Vector};

/// Brand colors, shared by the app, the icon set and the documentation site.
pub struct Colors;

impl Colors {
    /// Window background.
    pub const BACKGROUND: Color = Color::from_rgb(0.039, 0.047, 0.063);
    /// Side panels and cards.
    pub const PANEL: Color = Color::from_rgb(0.067, 0.078, 0.102);
    /// Raised surfaces.
    pub const SURFACE: Color = Color::from_rgb(0.090, 0.106, 0.137);
    /// Hairline borders.
    pub const BORDER: Color = Color::from_rgb(0.149, 0.173, 0.219);
    /// Primary text.
    pub const TEXT: Color = Color::from_rgb(0.914, 0.929, 0.961);
    /// Secondary text.
    pub const MUTED: Color = Color::from_rgb(0.545, 0.576, 0.651);
    /// Brand accent.
    pub const ACCENT: Color = Color::from_rgb(0.427, 0.522, 1.000);
    /// Positive feedback.
    pub const SUCCESS: Color = Color::from_rgb(0.310, 0.827, 0.647);
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

/// White tile that holds the pairing QR code.
pub fn qr_tile() -> impl Fn(&Theme) -> container::Style {
    |_theme| container::Style {
        background: Some(Background::Color(Color::WHITE)),
        border: Border {
            color: Color::WHITE,
            width: 0.0,
            radius: 16.0.into(),
        },
        ..Default::default()
    }
}

/// Dark backdrop drawn behind a modal.
///
/// It has no shadow on purpose: shadow layers get composited again on every
/// redraw, which made the whole window steadily darker as the mouse moved.
pub fn scrim() -> impl Fn(&Theme) -> container::Style {
    |_theme| container::Style {
        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.72))),
        ..Default::default()
    }
}

/// Floating card that holds a modal dialog.
pub fn modal() -> impl Fn(&Theme) -> container::Style {
    |_theme| container::Style {
        background: Some(Background::Color(Colors::SURFACE)),
        border: Border {
            color: Colors::BORDER,
            width: 1.0,
            radius: 18.0.into(),
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
            radius: 14.0.into(),
        },
        ..Default::default()
    }
}

/// Floating card used by dropdown menus.
pub fn dropdown() -> impl Fn(&Theme) -> container::Style {
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

/// Frame around a sidebar item that can be clicked, right-clicked or selected.
pub fn nav_item(selected: bool, hovered: bool) -> impl Fn(&Theme) -> container::Style {
    move |_theme| container::Style {
        background: Some(Background::Color(if selected {
            Colors::ACCENT
        } else if hovered {
            Colors::BORDER
        } else {
            Color::TRANSPARENT
        })),
        border: Border {
            color: Colors::BORDER,
            width: 0.0,
            radius: 12.0.into(),
        },
        ..Default::default()
    }
}

/// Frame around a key, highlighted when selected.
pub fn key_frame(selected: bool, hovered: bool) -> impl Fn(&Theme) -> container::Style {
    move |_theme| container::Style {
        background: Some(Background::Color(if selected {
            Colors::ACCENT
        } else if hovered {
            Color::from_rgba(Colors::BORDER.r, Colors::BORDER.g, Colors::BORDER.b, 0.6)
        } else {
            Color::TRANSPARENT
        })),
        border: Border {
            color: if selected || hovered {
                Colors::ACCENT
            } else {
                Colors::BORDER
            },
            width: if selected { 2.0 } else { 1.0 },
            radius: 14.0.into(),
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

/// Card used by action palette entries, with a hover state.
pub fn palette_card(hovered: bool) -> impl Fn(&Theme) -> container::Style {
    move |_theme| container::Style {
        background: Some(Background::Color(if hovered {
            Colors::BORDER
        } else {
            Colors::SURFACE
        })),
        border: Border {
            color: if hovered {
                Colors::ACCENT
            } else {
                Colors::BORDER
            },
            width: 1.0,
            radius: 12.0.into(),
        },
        ..Default::default()
    }
}

/// Text input that follows the dark surface palette.
pub fn input() -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
    |_theme, status| text_input::Style {
        background: Background::Color(Colors::SURFACE),
        border: Border {
            color: match status {
                text_input::Status::Hovered | text_input::Status::Focused { .. } => Colors::ACCENT,
                _ => Colors::BORDER,
            },
            width: 1.0,
            radius: 10.0.into(),
        },
        icon: Colors::MUTED,
        placeholder: Colors::MUTED,
        value: Colors::TEXT,
        selection: Colors::ACCENT,
    }
}

/// Thin scrollbar that sits in its own gutter instead of over the content.
pub fn scrollable_style() -> impl Fn(&Theme, scrollable::Status) -> scrollable::Style {
    |_theme, status| {
        let active = matches!(
            status,
            scrollable::Status::Hovered { .. } | scrollable::Status::Dragged { .. }
        );
        let rail = scrollable::Rail {
            background: Some(Background::Color(Colors::PANEL)),
            border: Border::default(),
            scroller: scrollable::Scroller {
                background: Background::Color(if active {
                    Colors::ACCENT
                } else {
                    Colors::BORDER
                }),
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 4.0.into(),
                },
            },
        };
        scrollable::Style {
            container: container::Style::default(),
            vertical_rail: rail,
            horizontal_rail: rail,
            gap: None,
            auto_scroll: scrollable::AutoScroll {
                background: Background::Color(Colors::SURFACE),
                border: Border::default(),
                shadow: Shadow::default(),
                icon: Colors::TEXT,
            },
        }
    }
}
