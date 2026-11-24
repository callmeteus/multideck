//! Device capabilities and the command/input contract shared by adapters.

use std::sync::Arc;

use image::RgbaImage;
use serde::{Deserialize, Serialize};

/// Pixel format a device accepts for key images.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyImageFormat {
    /// Windows bitmap, used by first generation Stream Decks.
    Bmp,
    /// JPEG, used by second generation Stream Decks.
    Jpeg,
    /// PNG.
    Png,
    /// Raw RGBA, used by the virtual device.
    Raw,
}

/// How a device is attached to the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    /// Runs inside the desktop process.
    Virtual,
    /// USB HID.
    Usb,
    /// Local network.
    Network,
}

/// Everything an adapter needs to know about the device it drives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceCaps {
    /// Grid columns.
    pub cols: u8,
    /// Grid rows.
    pub rows: u8,
    /// Image size expected per key, in pixels.
    pub key_image: (u16, u16),
    /// Number of rotary encoders.
    pub encoders: u8,
    /// Whether the device has a touch strip.
    pub touch_strip: bool,
    /// Image format the adapter should receive.
    pub format: KeyImageFormat,
}

impl Default for DeviceCaps {
    fn default() -> Self {
        Self {
            cols: 5,
            rows: 3,
            key_image: (96, 96),
            encoders: 0,
            touch_strip: false,
            format: KeyImageFormat::Raw,
        }
    }
}

impl DeviceCaps {
    /// Total number of keys on the grid.
    pub fn key_count(&self) -> u8 {
        self.cols.saturating_mul(self.rows)
    }

    /// Index of a key from its column and row.
    pub fn index_of(&self, col: u8, row: u8) -> Option<u8> {
        if col >= self.cols || row >= self.rows {
            return None;
        }
        Some(row * self.cols + col)
    }
}

/// Identity and capabilities of a connected device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceDescriptor {
    /// Stable id used by profiles and by the store.
    pub id: String,
    /// Human readable name shown in the UI.
    pub name: String,
    /// Model label, for example `Stream Deck MK.2`.
    pub model: String,
    /// Transport in use.
    pub transport: Transport,
    /// Grid and image capabilities.
    pub caps: DeviceCaps,
}

/// Commands sent from the core to a device adapter.
#[derive(Debug, Clone)]
pub enum DeviceCommand {
    /// Replace the image of one key.
    SetKeyImage {
        /// Key index.
        key: u8,
        /// Pixels to display.
        image: Arc<RgbaImage>,
    },
    /// Set panel brightness, 0 to 100.
    SetBrightness {
        /// Brightness percentage.
        percent: u8,
    },
    /// Clear every key and restore defaults.
    Reset,
}

/// Inputs reported by a device adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceInput {
    /// The key went down.
    KeyDown {
        /// Key index.
        key: u8,
    },
    /// The key went up.
    KeyUp {
        /// Key index.
        key: u8,
    },
    /// An encoder rotated.
    EncoderRotate {
        /// Encoder index.
        index: u8,
        /// Detents moved, negative is counter clockwise.
        ticks: i8,
    },
    /// An encoder was pressed.
    EncoderPress {
        /// Encoder index.
        index: u8,
    },
}
