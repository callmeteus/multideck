//! Render input for a single key.

/// An 8 bit sRGB color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// Builds a color from channels.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self(r, g, b)
    }

    /// Hex representation without the leading `#`.
    pub fn hex(self) -> String {
        format!("{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

/// Everything the renderer needs to draw one key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyView {
    /// Key index inside the grid.
    pub index: u8,
    /// Main label.
    pub title: String,
    /// Optional second line.
    pub subtitle: Option<String>,
    /// SVG path data for the icon, without the `svg` wrapper.
    pub icon: Option<String>,
    /// Key background.
    pub background: Rgb,
    /// Text and icon color.
    pub foreground: Rgb,
    /// Accent used by the active indicator.
    pub accent: Rgb,
    /// Whether the action is in its inverted state.
    pub active: bool,
}

impl KeyView {
    /// Empty key view, used for unassigned slots.
    pub fn empty(index: u8) -> Self {
        Self {
            index,
            title: String::new(),
            subtitle: None,
            icon: None,
            background: Rgb::new(30, 33, 40),
            foreground: Rgb::new(120, 128, 140),
            accent: Rgb::new(90, 98, 110),
            active: false,
        }
    }

    /// Whether this key has no content beyond the empty state.
    pub fn is_empty(&self) -> bool {
        self.title.is_empty() && self.icon.is_none()
    }
}
