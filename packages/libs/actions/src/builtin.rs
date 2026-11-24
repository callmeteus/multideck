//! The action set shipped with MultiDeck.

use multideck_core::{ActionDef, ActionInstance, FieldKind, SettingField};
use multideck_platform::MediaKey;
use serde_json::Value;

use crate::{ActionError, Effect, RegisteredAction};

const HOTKEY_FIELDS: &[SettingField] = &[SettingField {
    key: "hotkey",
    label_key: "field_hotkey",
    kind: FieldKind::Text,
    default: "ctrl+alt+p",
}];

const OPEN_FIELDS: &[SettingField] = &[SettingField {
    key: "target",
    label_key: "field_target",
    kind: FieldKind::Text,
    default: "https://elgato.com",
}];

const TEXT_FIELDS: &[SettingField] = &[SettingField {
    key: "text",
    label_key: "field_text",
    kind: FieldKind::Text,
    default: "Hello from MultiDeck",
}];

const MEDIA_FIELDS: &[SettingField] = &[SettingField {
    key: "media",
    label_key: "field_media",
    kind: FieldKind::Select(&[
        "play_pause",
        "next",
        "previous",
        "volume_up",
        "volume_down",
        "mute",
    ]),
    default: "play_pause",
}];

const COMMAND_FIELDS: &[SettingField] = &[SettingField {
    key: "command",
    label_key: "field_command",
    kind: FieldKind::Text,
    default: "echo MultiDeck",
}];

const DELAY_FIELDS: &[SettingField] = &[SettingField {
    key: "seconds",
    label_key: "field_seconds",
    kind: FieldKind::Number,
    default: "1",
}];

const FOLDER_FIELDS: &[SettingField] = &[SettingField {
    key: "page",
    label_key: "field_page",
    kind: FieldKind::Number,
    default: "1",
}];

const PROFILE_FIELDS: &[SettingField] = &[SettingField {
    key: "profile",
    label_key: "field_profile",
    kind: FieldKind::Text,
    default: "",
}];

const BRIGHTNESS_FIELDS: &[SettingField] = &[SettingField {
    key: "percent",
    label_key: "field_percent",
    kind: FieldKind::Number,
    default: "80",
}];

const HOTKEY: ActionDef = ActionDef {
    kind: "hotkey",
    label_key: "action_hotkey",
    description_key: "action_hotkey_desc",
    icon: "hotkey",
    fields: HOTKEY_FIELDS,
    title_key: "action_hotkey_title",
};

const OPEN: ActionDef = ActionDef {
    kind: "open",
    label_key: "action_open",
    description_key: "action_open_desc",
    icon: "open",
    fields: OPEN_FIELDS,
    title_key: "action_open_title",
};

const TEXT: ActionDef = ActionDef {
    kind: "text",
    label_key: "action_text",
    description_key: "action_text_desc",
    icon: "text",
    fields: TEXT_FIELDS,
    title_key: "action_text_title",
};

const MEDIA: ActionDef = ActionDef {
    kind: "media",
    label_key: "action_media",
    description_key: "action_media_desc",
    icon: "media",
    fields: MEDIA_FIELDS,
    title_key: "action_media_title",
};

const COMMAND: ActionDef = ActionDef {
    kind: "command",
    label_key: "action_command",
    description_key: "action_command_desc",
    icon: "command",
    fields: COMMAND_FIELDS,
    title_key: "action_command_title",
};

const DELAY: ActionDef = ActionDef {
    kind: "delay",
    label_key: "action_delay",
    description_key: "action_delay_desc",
    icon: "delay",
    fields: DELAY_FIELDS,
    title_key: "action_delay_title",
};

const MULTI: ActionDef = ActionDef {
    kind: "multi",
    label_key: "action_multi",
    description_key: "action_multi_desc",
    icon: "multi",
    fields: &[],
    title_key: "action_multi_title",
};

const FOLDER: ActionDef = ActionDef {
    kind: "folder",
    label_key: "action_folder",
    description_key: "action_folder_desc",
    icon: "folder",
    fields: FOLDER_FIELDS,
    title_key: "action_folder_title",
};

const PROFILE: ActionDef = ActionDef {
    kind: "profile",
    label_key: "action_profile",
    description_key: "action_profile_desc",
    icon: "profile",
    fields: PROFILE_FIELDS,
    title_key: "action_profile_title",
};

const BRIGHTNESS: ActionDef = ActionDef {
    kind: "brightness",
    label_key: "action_brightness",
    description_key: "action_brightness_desc",
    icon: "brightness",
    fields: BRIGHTNESS_FIELDS,
    title_key: "action_brightness_title",
};

inventory::submit! { RegisteredAction { def: HOTKEY, run: run_hotkey } }
inventory::submit! { RegisteredAction { def: OPEN, run: run_open } }
inventory::submit! { RegisteredAction { def: TEXT, run: run_text } }
inventory::submit! { RegisteredAction { def: MEDIA, run: run_media } }
inventory::submit! { RegisteredAction { def: COMMAND, run: run_command } }
inventory::submit! { RegisteredAction { def: DELAY, run: run_delay } }
inventory::submit! { RegisteredAction { def: MULTI, run: run_multi } }
inventory::submit! { RegisteredAction { def: FOLDER, run: run_folder } }
inventory::submit! { RegisteredAction { def: PROFILE, run: run_profile } }
inventory::submit! { RegisteredAction { def: BRIGHTNESS, run: run_brightness } }

fn required<'a>(instance: &'a ActionInstance, field: &str) -> Result<&'a str, ActionError> {
    instance
        .string(field)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ActionError::MissingSetting {
            kind: instance.kind.clone(),
            field: field.to_string(),
        })
}

fn integer(instance: &ActionInstance, field: &str) -> Result<i64, ActionError> {
    instance
        .integer(field)
        .ok_or_else(|| ActionError::InvalidSetting {
            kind: instance.kind.clone(),
            field: field.to_string(),
            reason: "expected a number".into(),
        })
}

fn run_hotkey(instance: &ActionInstance) -> Result<Vec<Effect>, ActionError> {
    multideck_platform::send_hotkey(required(instance, "hotkey")?)?;
    Ok(Vec::new())
}

fn run_open(instance: &ActionInstance) -> Result<Vec<Effect>, ActionError> {
    multideck_platform::open_target(required(instance, "target")?)?;
    Ok(Vec::new())
}

fn run_text(instance: &ActionInstance) -> Result<Vec<Effect>, ActionError> {
    multideck_platform::type_text(required(instance, "text")?)?;
    Ok(Vec::new())
}

fn run_media(instance: &ActionInstance) -> Result<Vec<Effect>, ActionError> {
    let value = required(instance, "media")?;
    let media = MediaKey::parse(value).ok_or_else(|| ActionError::InvalidSetting {
        kind: instance.kind.clone(),
        field: "media".into(),
        reason: format!("unknown media key '{value}'"),
    })?;
    multideck_platform::media_key(media)?;
    Ok(Vec::new())
}

fn run_command(instance: &ActionInstance) -> Result<Vec<Effect>, ActionError> {
    multideck_platform::run_command(required(instance, "command")?)?;
    Ok(Vec::new())
}

fn run_delay(instance: &ActionInstance) -> Result<Vec<Effect>, ActionError> {
    let seconds = integer(instance, "seconds")?.clamp(0, 3600);
    Ok(vec![Effect::Delay((seconds as u64) * 1000)])
}

fn run_multi(instance: &ActionInstance) -> Result<Vec<Effect>, ActionError> {
    let steps = instance
        .settings
        .get("steps")
        .and_then(Value::as_array)
        .cloned();
    let mut effects = Vec::new();
    for step in steps.unwrap_or_default() {
        let kind = step.get("kind").and_then(Value::as_str).unwrap_or_default();
        if kind.is_empty() {
            continue;
        }
        let settings = step.get("settings").cloned().unwrap_or(Value::Null);
        effects.extend(crate::execute_settings(kind, &settings)?);
    }
    Ok(effects)
}

fn run_folder(instance: &ActionInstance) -> Result<Vec<Effect>, ActionError> {
    let page = integer(instance, "page")?.max(1) as usize;
    Ok(vec![Effect::SwitchPage(page - 1)])
}

fn run_profile(instance: &ActionInstance) -> Result<Vec<Effect>, ActionError> {
    Ok(vec![Effect::SwitchProfile(
        required(instance, "profile")?.to_string(),
    )])
}

fn run_brightness(instance: &ActionInstance) -> Result<Vec<Effect>, ActionError> {
    let percent = integer(instance, "percent")?.clamp(0, 100) as u8;
    Ok(vec![Effect::SetBrightness(percent)])
}
