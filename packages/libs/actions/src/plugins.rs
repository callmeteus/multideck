//! YAML plugins that add actions at runtime.
//!
//! A plugin is a `.yml` or `.yaml` file in `MULTIDECK_PLUGINS` or in the user
//! config folder (`%APPDATA%/multideck/plugins` on Windows,
//! `$XDG_CONFIG_HOME|~/.config/multideck/plugins` elsewhere):
//!
//! ```yaml
//! name: My tools
//! actions:
//!   - id: my-tool
//!     title: My tool
//!     description: Runs my tool with the selected mode
//!     icon: command
//!     tags: [tool, cli]
//!     command: my-tool
//!     args: ["--mode", "{mode}"]
//!     fields:
//!       - key: mode
//!         label: Mode
//!         default: fast
//!         kind: select
//!         options: [fast, full]
//! ```

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use multideck_core::{ActionDef, ActionInstance, FieldKind, SettingField};
use serde::Deserialize;

use crate::ActionError;

/// One action loaded from a plugin file.
pub struct PluginAction {
    /// Metadata shown by the palette and the inspector.
    pub def: &'static ActionDef,
    /// Executable or shell line prefix.
    pub command: String,
    /// Arguments, with `{field}` placeholders.
    pub args: Vec<String>,
}

impl PluginAction {
    /// Expands the placeholders and runs the command through the shell.
    pub fn run(&self, instance: &ActionInstance) -> Result<Vec<crate::Effect>, ActionError> {
        let mut parts = Vec::with_capacity(self.args.len() + 1);
        parts.push(self.command.clone());
        for arg in &self.args {
            parts.push(expand(arg, instance, self.def));
        }
        let line = parts.join(" ");
        multideck_platform::run_command(&line)?;
        Ok(Vec::new())
    }
}

fn expand(template: &str, instance: &ActionInstance, def: &ActionDef) -> String {
    let mut output = template.to_string();
    for field in def.fields {
        let value = instance.string(field.key).unwrap_or(field.default);
        output = output.replace(&format!("{{{}}}", field.key), value);
    }
    output
}

#[derive(Debug, Deserialize)]
struct PluginFile {
    #[serde(default)]
    #[allow(dead_code)]
    name: Option<String>,
    #[serde(default)]
    actions: Vec<PluginEntry>,
}

#[derive(Debug, Deserialize)]
struct PluginEntry {
    id: String,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    fields: Vec<PluginField>,
}

#[derive(Debug, Deserialize)]
struct PluginField {
    key: String,
    #[serde(default)]
    label: String,
    #[serde(default)]
    default: String,
    #[serde(default = "default_kind")]
    kind: String,
    #[serde(default)]
    options: Vec<String>,
}

fn default_kind() -> String {
    "text".to_string()
}

/// Returns every action loaded from plugins, loading them on first use.
pub fn actions() -> &'static [PluginAction] {
    static ACTIONS: OnceLock<Vec<PluginAction>> = OnceLock::new();
    ACTIONS.get_or_init(load)
}

/// Looks up a plugin action by kind.
pub fn action(kind: &str) -> Option<&'static PluginAction> {
    actions().iter().find(|action| action.def.kind == kind)
}

fn load() -> Vec<PluginAction> {
    let mut loaded = Vec::new();
    let mut seen = HashSet::new();
    for directory in directories() {
        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        let mut files: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                matches!(
                    path.extension().and_then(|value| value.to_str()),
                    Some("yml") | Some("yaml")
                )
            })
            .collect();
        files.sort();

        for path in files {
            match read_plugin(&path) {
                Ok(actions) => {
                    for action in actions {
                        if !seen.insert(action.def.kind) {
                            tracing::warn!(kind = action.def.kind, "duplicate plugin action");
                            continue;
                        }
                        loaded.push(action);
                    }
                }
                Err(error) => tracing::warn!(%error, path = %path.display(), "plugin not loaded"),
            }
        }
    }
    loaded
}

fn read_plugin(path: &Path) -> Result<Vec<PluginAction>, String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let file: PluginFile = serde_yaml::from_str(&text).map_err(|error| error.to_string())?;
    Ok(file
        .actions
        .into_iter()
        .map(|entry| PluginAction {
            def: leak_def(&entry),
            command: entry.command.clone(),
            args: entry.args.clone(),
        })
        .collect())
}

fn leak_def(entry: &PluginEntry) -> &'static ActionDef {
    let icon = entry
        .icon
        .clone()
        .filter(|name| multideck_render::icon_path(name).is_some())
        .unwrap_or_else(|| "command".to_string());
    let fields: Vec<SettingField> = entry
        .fields
        .iter()
        .map(|field| SettingField {
            key: leak(field.key.clone()),
            label_key: leak(if field.label.is_empty() {
                field.key.clone()
            } else {
                field.label.clone()
            }),
            kind: match field.kind.as_str() {
                "path" => FieldKind::Path,
                "number" => FieldKind::Number,
                "select" => {
                    let options: Vec<&'static str> = field
                        .options
                        .iter()
                        .map(|option| leak(option.clone()))
                        .collect();
                    FieldKind::Select(Box::leak(options.into_boxed_slice()))
                }
                _ => FieldKind::Text,
            },
            default: leak(field.default.clone()),
        })
        .collect();

    Box::leak(Box::new(ActionDef {
        kind: leak(entry.id.clone()),
        label_key: leak(entry.title.clone()),
        description_key: leak(entry.description.clone()),
        icon: leak(icon),
        tags: Box::leak(
            entry
                .tags
                .iter()
                .map(|tag| leak(tag.clone()))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ),
        fields: Box::leak(fields.into_boxed_slice()),
        title_key: leak(entry.title.clone()),
    }))
}

fn leak(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

fn directories() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if let Ok(path) = std::env::var("MULTIDECK_PLUGINS") {
        directories.push(PathBuf::from(path));
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        directories.push(PathBuf::from(appdata).join("multideck").join("plugins"));
    }
    if let Ok(config) = std::env::var("XDG_CONFIG_HOME") {
        directories.push(PathBuf::from(config).join("multideck").join("plugins"));
    } else if let Ok(home) = std::env::var("HOME") {
        directories.push(
            PathBuf::from(home)
                .join(".config")
                .join("multideck")
                .join("plugins"),
        );
    }
    directories
}
