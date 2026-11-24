//! Turns an action instance into the view model used to draw a key.

use multideck_core::{ActionInstance, KeyView, Rgb};
use multideck_i18n::{tr_args, translate};
use multideck_platform::MediaKey;

/// Colors used for one action family.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// Key background.
    pub background: Rgb,
    /// Text and icon color.
    pub foreground: Rgb,
    /// Accent used when the action is active.
    pub accent: Rgb,
}

const FOREGROUND: Rgb = Rgb::new(240, 243, 248);

/// Returns the palette of an action kind, or the empty slot palette.
pub fn palette_for(kind: Option<&str>) -> Palette {
    let (background, accent) = match kind.unwrap_or_default() {
        "hotkey" => (Rgb::new(34, 40, 58), Rgb::new(122, 150, 255)),
        "open" => (Rgb::new(30, 50, 44), Rgb::new(96, 216, 160)),
        "text" => (Rgb::new(44, 45, 54), Rgb::new(190, 198, 220)),
        "media" => (Rgb::new(52, 34, 44), Rgb::new(255, 128, 170)),
        "command" => (Rgb::new(48, 42, 32), Rgb::new(255, 196, 96)),
        "delay" => (Rgb::new(36, 36, 46), Rgb::new(160, 168, 190)),
        "multi" => (Rgb::new(42, 34, 56), Rgb::new(186, 148, 255)),
        "folder" => (Rgb::new(28, 44, 58), Rgb::new(110, 200, 255)),
        "profile" => (Rgb::new(54, 46, 32), Rgb::new(255, 220, 130)),
        "brightness" => (Rgb::new(48, 44, 30), Rgb::new(255, 210, 110)),
        _ => (Rgb::new(28, 31, 38), Rgb::new(80, 88, 100)),
    };
    Palette {
        background,
        foreground: FOREGROUND,
        accent,
    }
}

/// Builds the view model for an action placed on a key.
pub fn present(instance: &ActionInstance, index: u8) -> KeyView {
    let definition = crate::definition(&instance.kind);
    let palette = palette_for(Some(&instance.kind));
    let title = definition
        .map(|definition| translate(definition.title_key))
        .unwrap_or_else(|| instance.kind.clone());
    let icon = definition
        .and_then(|definition| multideck_render::icon_path(definition.icon))
        .map(str::to_string);

    KeyView {
        index,
        title,
        subtitle: subtitle(instance),
        icon,
        background: palette.background,
        foreground: palette.foreground,
        accent: palette.accent,
        active: instance.state > 0,
    }
}

/// Builds the view model for a key without an action_
pub fn empty(index: u8) -> KeyView {
    let palette = palette_for(None);
    KeyView {
        index,
        title: String::new(),
        subtitle: None,
        icon: None,
        background: palette.background,
        foreground: palette.foreground,
        accent: palette.accent,
        active: false,
    }
}

fn subtitle(instance: &ActionInstance) -> Option<String> {
    let value = match instance.kind.as_str() {
        "hotkey" => instance.string("hotkey").map(|value| value.to_uppercase()),
        "open" => instance.string("target").map(short_target),
        "text" => instance.string("text").map(|value| truncate(value, 16)),
        "command" => instance.string("command").map(|value| truncate(value, 16)),
        "media" => instance.string("media").map(media_label),
        "delay" => instance
            .integer("seconds")
            .map(|seconds| format!("{seconds}s")),
        "folder" => instance
            .integer("page")
            .map(|page| tr_args!("profiles_page_name", "number" => page)),
        "profile" => instance
            .string("profile")
            .filter(|value| !value.is_empty())
            .map(|value| truncate(value, 16)),
        "brightness" => instance
            .integer("percent")
            .map(|percent| format!("{percent}%")),
        _ => None,
    };
    value
        .map(|value| truncate(&value, 18))
        .filter(|value| !value.is_empty())
}

fn media_label(value: &str) -> String {
    match MediaKey::parse(value) {
        Some(MediaKey::PlayPause) => translate("media_play_pause"),
        Some(MediaKey::Next) => translate("media_next"),
        Some(MediaKey::Previous) => translate("media_previous"),
        Some(MediaKey::VolumeUp) => translate("media_volume_up"),
        Some(MediaKey::VolumeDown) => translate("media_volume_down"),
        Some(MediaKey::Mute) => translate("media_mute"),
        None => value.to_string(),
    }
}

fn short_target(value: &str) -> String {
    let trimmed = value.trim_end_matches('/');
    let last = trimmed.rsplit(['/', '\\']).next().unwrap_or(trimmed);
    if last.is_empty() {
        truncate(trimmed, 16)
    } else {
        truncate(last, 16)
    }
}

fn truncate(value: &str, max_chars: usize) -> String {
    let count = value.chars().count();
    if count <= max_chars {
        return value.to_string();
    }
    let mut result: String = value.chars().take(max_chars.saturating_sub(1)).collect();
    result.push('…');
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presents_every_builtin_without_panicking() {
        for definition in crate::definitions() {
            let instance = ActionInstance::from_def(definition);
            let view = present(&instance, 0);
            assert!(!view.title.is_empty(), "{} has no title", definition.kind);
            assert!(view.icon.is_some(), "{} has no icon", definition.kind);
        }
    }

    #[test]
    fn empty_keys_are_marked_empty() {
        let view = empty(3);
        assert!(view.is_empty());
        assert_eq!(view.index, 3);
    }

    #[test]
    fn subtitles_summarize_settings() {
        let mut instance = ActionInstance::new("brightness");
        instance.set("percent", serde_json::json!("55"));
        assert_eq!(present(&instance, 0).subtitle.as_deref(), Some("55%"));
    }
}
