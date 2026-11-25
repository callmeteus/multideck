//! Actions a key can run, and the registry that discovers them.
//!
//! Actions register themselves with `inventory`, so adding one means adding a
//! file next to [`builtin`] and submitting it. There is no central list to
//! keep in sync.

mod builtin;
pub mod plugins;
mod present;

use multideck_core::{ActionDef, ActionInstance};
use multideck_platform::PlatformError;
use thiserror::Error;

pub use present::{Palette, empty, palette_for, present};

/// An effect that only the host application can apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Switch to another profile by id.
    SwitchProfile(String),
    /// Show another page of the active profile.
    SwitchPage(usize),
    /// Change device brightness.
    SetBrightness(u8),
    /// Wait before the next effect, in milliseconds.
    Delay(u64),
}

/// Failure while running an action_
#[derive(Debug, Error)]
pub enum ActionError {
    /// The kind has no registered executor.
    #[error("unknown action kind '{0}'")]
    UnknownKind(String),

    /// A required setting is missing.
    #[error("action '{kind}' needs the setting '{field}'")]
    MissingSetting {
        /// Action kind.
        kind: String,
        /// Missing setting.
        field: String,
    },

    /// A setting has an unusable value.
    #[error("action '{kind}' has an invalid value for '{field}': {reason}")]
    InvalidSetting {
        /// Action kind.
        kind: String,
        /// Setting name.
        field: String,
        /// Why it was rejected.
        reason: String,
    },

    /// The operating system refused the operation.
    #[error(transparent)]
    Platform(#[from] PlatformError),
}

/// One registered action: its metadata and how to run it.
pub struct RegisteredAction {
    /// Static description used by the palette and the inspector_
    pub def: ActionDef,
    /// Executor for one instance.
    pub run: fn(&ActionInstance) -> Result<Vec<Effect>, ActionError>,
}

inventory::collect!(RegisteredAction);

/// Every registered action, sorted by kind.
pub fn definitions() -> Vec<&'static ActionDef> {
    let mut definitions: Vec<&'static ActionDef> = inventory::iter::<RegisteredAction>
        .into_iter()
        .map(|action| &action.def)
        .collect();
    definitions.extend(plugins::actions().iter().map(|action| action.def));
    definitions.sort_by_key(|definition| definition.kind);
    definitions.dedup_by_key(|definition| definition.kind);
    definitions
}

/// Looks up an action by kind.
pub fn definition(kind: &str) -> Option<&'static ActionDef> {
    inventory::iter::<RegisteredAction>
        .into_iter()
        .find(|action| action.def.kind == kind)
        .map(|action| &action.def)
        .or_else(|| plugins::action(kind).map(|action| action.def))
}

fn executor(kind: &str) -> Option<&'static RegisteredAction> {
    inventory::iter::<RegisteredAction>
        .into_iter()
        .find(|action| action.def.kind == kind)
}

/// Runs one configured action instance.
pub fn execute(instance: &ActionInstance) -> Result<Vec<Effect>, ActionError> {
    if let Some(plugin) = plugins::action(&instance.kind) {
        tracing::debug!(kind = %instance.kind, id = %instance.id, "running plugin action");
        return plugin.run(instance);
    }
    let action =
        executor(&instance.kind).ok_or_else(|| ActionError::UnknownKind(instance.kind.clone()))?;
    tracing::debug!(kind = %instance.kind, id = %instance.id, "running action");
    (action.run)(instance)
}

/// Runs an action described by a kind and a JSON settings object.
pub fn execute_settings(
    kind: &str,
    settings: &serde_json::Value,
) -> Result<Vec<Effect>, ActionError> {
    let mut instance = ActionInstance::new(kind);
    instance.settings = settings.clone();
    execute(&instance)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_is_registered_and_documented() {
        let definitions = definitions();
        let kinds: Vec<&str> = definitions
            .iter()
            .map(|definition| definition.kind)
            .collect();
        for expected in [
            "brightness",
            "command",
            "delay",
            "folder",
            "hex",
            "hotkey",
            "media",
            "multi",
            "open",
            "profile",
            "text",
        ] {
            assert!(kinds.contains(&expected), "missing action {expected}");
        }
        for definition in definitions {
            assert!(!definition.label_key.is_empty());
            assert!(
                multideck_render::icon_path(definition.icon).is_some(),
                "{} has no icon",
                definition.kind
            );
        }
    }

    #[test]
    fn folder_and_brightness_return_effects() {
        let mut folder = ActionInstance::new("folder");
        folder.set("page", serde_json::json!("2"));
        assert_eq!(
            execute(&folder).expect("folder"),
            vec![Effect::SwitchPage(1)]
        );

        let mut brightness = ActionInstance::new("brightness");
        brightness.set("percent", serde_json::json!("45"));
        assert_eq!(
            execute(&brightness).expect("brightness"),
            vec![Effect::SetBrightness(45)]
        );

        let mut profile = ActionInstance::new("profile");
        profile.set("profile", serde_json::json!("work"));
        assert_eq!(
            execute(&profile).expect("profile"),
            vec![Effect::SwitchProfile("work".into())]
        );
    }

    #[test]
    fn delay_is_reported_as_an_effect() {
        let mut delay = ActionInstance::new("delay");
        delay.set("seconds", serde_json::json!("2"));
        assert_eq!(execute(&delay).expect("delay"), vec![Effect::Delay(2000)]);
    }

    #[test]
    fn multi_action_runs_children_in_order() {
        let settings = serde_json::json!({
            "steps": [
                { "kind": "brightness", "settings": { "percent": "10" } },
                { "kind": "folder", "settings": { "page": "3" } }
            ]
        });
        let effects = execute_settings("multi", &settings).expect("multi");
        assert_eq!(
            effects,
            vec![Effect::SetBrightness(10), Effect::SwitchPage(2)]
        );
    }

    #[test]
    fn unknown_kind_is_an_error() {
        let error = execute(&ActionInstance::new("nope")).expect_err("must fail");
        assert!(matches!(error, ActionError::UnknownKind(_)));
    }
}
