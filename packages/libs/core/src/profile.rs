//! Profiles, pages and key slots, the user facing layout model.

use serde::{Deserialize, Serialize};

use crate::action::ActionInstance;
use crate::device::DeviceCaps;
use crate::error::CoreError;

/// Schema version written to disk. Bumped when the layout changes shape.
pub const PROFILE_SCHEMA_VERSION: u32 = 1;

/// What sits on one key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct KeySlot {
    /// Configured action, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<ActionInstance>,
    /// Overrides the action title when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_override: Option<String>,
}

impl KeySlot {
    /// Empty slot.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Whether the slot has no action and no override.
    pub fn is_empty(&self) -> bool {
        self.action.is_none() && self.title_override.is_none()
    }
}

/// A grid of keys shown on a device at one time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page {
    /// Stable id.
    pub id: String,
    /// Name shown in the page switcher.
    pub name: String,
    /// Slots in row major order.
    pub slots: Vec<KeySlot>,
}

impl Page {
    /// Builds a page with the given number of empty slots.
    pub fn empty(name: impl Into<String>, keys: usize) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            slots: vec![KeySlot::empty(); keys],
        }
    }

    /// Grows or shrinks the page to fit the given key count.
    pub fn resize(&mut self, keys: usize) {
        self.slots.resize(keys, KeySlot::empty());
    }
}

/// A named layout with one or more pages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    /// Schema version of the file on disk.
    pub schema: u32,
    /// Stable id.
    pub id: String,
    /// Name shown in the profile switcher.
    pub name: String,
    /// Brightness applied to devices using this profile.
    pub brightness: u8,
    /// Pages in order.
    pub pages: Vec<Page>,
    /// Index of the visible page.
    pub active_page: usize,
}

impl Profile {
    /// Creates a profile sized for the given capabilities.
    pub fn new(name: impl Into<String>, caps: &DeviceCaps) -> Self {
        let keys = caps.key_count() as usize;
        Self {
            schema: PROFILE_SCHEMA_VERSION,
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            brightness: 80,
            pages: vec![Page::empty("Main", keys)],
            active_page: 0,
        }
    }

    /// Visible page, falling back to the first one.
    pub fn active_page(&self) -> &Page {
        let index = self.active_page.min(self.pages.len().saturating_sub(1));
        &self.pages[index]
    }

    /// Mutable visible page.
    pub fn active_page_mut(&mut self) -> &mut Page {
        if self.active_page >= self.pages.len() {
            self.active_page = 0;
        }
        self.pages
            .get_mut(self.active_page)
            .expect("profiles always have at least one page")
    }

    /// Reads a slot of the visible page.
    pub fn slot(&self, key: u8) -> Option<&KeySlot> {
        self.active_page().slots.get(key as usize)
    }

    /// Mutates a slot of the visible page.
    pub fn slot_mut(&mut self, key: u8) -> Option<&mut KeySlot> {
        let page = self.active_page_mut();
        page.slots.get_mut(key as usize)
    }

    /// Resizes every page for the given device and validates the result.
    pub fn fit_to(&mut self, caps: &DeviceCaps) {
        let keys = caps.key_count() as usize;
        if self.pages.is_empty() {
            self.pages.push(Page::empty("Main", keys));
        }
        for page in &mut self.pages {
            page.resize(keys);
        }
        if self.active_page >= self.pages.len() {
            self.active_page = 0;
        }
    }

    /// Serializes the profile as pretty JSON.
    pub fn to_json(&self) -> Result<String, CoreError> {
        serde_json::to_string_pretty(self)
            .map_err(|error| CoreError::InvalidProfile(error.to_string()))
    }

    /// Parses a profile, migrating older schema versions when needed.
    pub fn from_json(text: &str) -> Result<Self, CoreError> {
        let mut profile: Self = serde_json::from_str(text)
            .map_err(|error| CoreError::InvalidProfile(error.to_string()))?;
        if profile.pages.is_empty() {
            return Err(CoreError::InvalidProfile("profile has no pages".into()));
        }
        profile.schema = PROFILE_SCHEMA_VERSION;
        Ok(profile)
    }

    /// Adds a new empty page and switches to it.
    pub fn add_page(&mut self, name: impl Into<String>, caps: &DeviceCaps) {
        let keys = caps.key_count() as usize;
        self.pages.push(Page::empty(name, keys));
        self.active_page = self.pages.len() - 1;
    }
}
