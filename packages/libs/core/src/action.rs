//! Action metadata and the per-key action instance.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Input widget used by the action inspector_
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    /// Single line text.
    Text,
    /// Filesystem path.
    Path,
    /// Integer number.
    Number,
    /// One of a fixed set of values.
    Select(&'static [&'static str]),
}

/// One configurable field of an action_
#[derive(Debug, Clone, Copy)]
pub struct SettingField {
    /// Key used inside `ActionInstance::settings`.
    pub key: &'static str,
    /// Translation key for the label.
    pub label_key: &'static str,
    /// Widget to render.
    pub kind: FieldKind,
    /// Value used when the action is created.
    pub default: &'static str,
}

/// Static description of an action, registered by a plugin or by the core set.
#[derive(Debug, Clone, Copy)]
pub struct ActionDef {
    /// Stable kind, for example `hotkey`.
    pub kind: &'static str,
    /// Translation key for the display name.
    pub label_key: &'static str,
    /// Translation key for the one line description.
    pub description_key: &'static str,
    /// Name of the icon in `multideck-render`.
    pub icon: &'static str,
    /// Settings exposed in the inspector_
    pub fields: &'static [SettingField],
    /// Translation key for the key title used when the user does not set one.
    pub title_key: &'static str,
}

/// A configured action placed on a key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionInstance {
    /// Unique instance id.
    pub id: String,
    /// Kind of action, matching an [`ActionDef`].
    pub kind: String,
    /// Settings as a JSON object.
    pub settings: Value,
    /// Internal state for toggle and multi actions_
    pub state: u8,
}

impl ActionInstance {
    /// Creates an instance with the given kind and an empty settings object.
    pub fn new(kind: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            kind: kind.into(),
            settings: Value::Object(serde_json::Map::new()),
            state: 0,
        }
    }

    /// Creates an instance and copies the defaults of the given definition.
    pub fn from_def(def: &ActionDef) -> Self {
        let mut instance = Self::new(def.kind);
        for field in def.fields {
            instance
                .settings
                .as_object_mut()
                .expect("settings are always an object")
                .insert(
                    field.key.to_string(),
                    Value::String(field.default.to_string()),
                );
        }
        instance
    }

    /// Sets one setting.
    pub fn set(&mut self, key: &str, value: Value) {
        if let Some(map) = self.settings.as_object_mut() {
            map.insert(key.to_string(), value);
        }
    }

    /// Reads one setting as a string.
    pub fn string(&self, key: &str) -> Option<&str> {
        self.settings.get(key).and_then(Value::as_str)
    }

    /// Reads one setting as a string, or the given fallback.
    pub fn string_or<'a>(&'a self, key: &str, fallback: &'a str) -> &'a str {
        self.string(key).unwrap_or(fallback)
    }

    /// Reads one setting as an integer.
    pub fn integer(&self, key: &str) -> Option<i64> {
        self.settings.get(key).and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
        })
    }
}
