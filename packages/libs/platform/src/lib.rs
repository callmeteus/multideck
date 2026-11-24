//! Operating system integration used by actions_
//!
//! Everything in this crate has side effects outside the application. The
//! rest of the workspace treats it as the only place allowed to touch the
//! user's desktop.

use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use thiserror::Error;

/// Failure while talking to the operating system.
#[derive(Debug, Error)]
pub enum PlatformError {
    /// The input backend could not be initialised.
    #[error("could not initialise input simulation: {0}")]
    Input(String),

    /// A hotkey string could not be parsed.
    #[error("invalid hotkey '{combo}': {reason}")]
    InvalidHotkey {
        /// The combination as written by the user.
        combo: String,
        /// Why it was rejected.
        reason: String,
    },

    /// The target could not be opened.
    #[error("could not open '{target}': {source}")]
    Open {
        /// What the user asked to open.
        target: String,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },

    /// A shell command could not be spawned.
    #[error("could not run command: {0}")]
    Command(String),
}

/// Media and volume keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKey {
    /// Play or pause the current media_
    PlayPause,
    /// Skip to the next track.
    Next,
    /// Go back to the previous track.
    Previous,
    /// Raise the system volume.
    VolumeUp,
    /// Lower the system volume.
    VolumeDown,
    /// Toggle mute.
    Mute,
}

impl MediaKey {
    /// Parses the value stored in an action setting.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "play_pause" | "playpause" | "play" => Some(Self::PlayPause),
            "next" => Some(Self::Next),
            "previous" | "prev" => Some(Self::Previous),
            "volume_up" => Some(Self::VolumeUp),
            "volume_down" => Some(Self::VolumeDown),
            "mute" => Some(Self::Mute),
            _ => None,
        }
    }
}

fn engine() -> Result<Enigo, PlatformError> {
    Enigo::new(&Settings::default()).map_err(|error| PlatformError::Input(error.to_string()))
}

/// Sends one media key press.
pub fn media_key(media: MediaKey) -> Result<(), PlatformError> {
    let key = match media {
        MediaKey::PlayPause => Key::MediaPlayPause,
        MediaKey::Next => Key::MediaNextTrack,
        MediaKey::Previous => Key::MediaPrevTrack,
        MediaKey::VolumeUp => Key::VolumeUp,
        MediaKey::VolumeDown => Key::VolumeDown,
        MediaKey::Mute => Key::VolumeMute,
    };
    let mut enigo = engine()?;
    enigo
        .key(key, Direction::Click)
        .map_err(|error| PlatformError::Input(error.to_string()))
}

/// Types text into the focused window.
pub fn type_text(text: &str) -> Result<(), PlatformError> {
    let mut enigo = engine()?;
    enigo
        .text(text)
        .map_err(|error| PlatformError::Input(error.to_string()))
}

/// Presses a key combination such as `ctrl+shift+p`.
pub fn send_hotkey(combo: &str) -> Result<(), PlatformError> {
    let (modifiers, key) = parse_hotkey(combo)?;
    let mut enigo = engine()?;

    for modifier in &modifiers {
        enigo
            .key(*modifier, Direction::Press)
            .map_err(|error| PlatformError::Input(error.to_string()))?;
    }
    let result = enigo
        .key(key, Direction::Click)
        .map_err(|error| PlatformError::Input(error.to_string()));
    for modifier in modifiers.iter().rev() {
        enigo
            .key(*modifier, Direction::Release)
            .map_err(|error| PlatformError::Input(error.to_string()))?;
    }
    result
}

/// Opens a file, folder or URL with the user's default application.
pub fn open_target(target: &str) -> Result<(), PlatformError> {
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return Err(PlatformError::Open {
            target: target.to_string(),
            source: std::io::Error::new(std::io::ErrorKind::InvalidInput, "empty target"),
        });
    }
    open::that(trimmed).map_err(|source| PlatformError::Open {
        target: trimmed.to_string(),
        source,
    })
}

/// Spawns a shell command without waiting for it.
pub fn run_command(command: &str) -> Result<(), PlatformError> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err(PlatformError::Command("empty command".into()));
    }

    #[cfg(windows)]
    let child = {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("cmd")
            .raw_arg("/C")
            .raw_arg(trimmed)
            .spawn()
    };

    #[cfg(not(windows))]
    let child = std::process::Command::new("sh")
        .args(["-c", trimmed])
        .spawn();

    match child {
        Ok(mut process) => {
            std::thread::spawn(move || {
                if let Err(error) = process.wait() {
                    tracing::warn!(%error, "shell command could not be reaped");
                }
            });
            Ok(())
        }
        Err(error) => Err(PlatformError::Command(error.to_string())),
    }
}

/// Title of the window that currently has focus, when the platform exposes it.
pub fn active_window() -> Option<String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW};

        let handle = unsafe { GetForegroundWindow() };
        if handle.is_null() {
            return None;
        }
        let mut buffer = [0u16; 512];
        let length = unsafe { GetWindowTextW(handle, buffer.as_mut_ptr(), buffer.len() as i32) };
        if length <= 0 {
            return None;
        }
        let title = String::from_utf16_lossy(&buffer[..length as usize]);
        Some(title.trim().to_string()).filter(|title| !title.is_empty())
    }

    #[cfg(not(windows))]
    {
        None
    }
}

fn parse_hotkey(combo: &str) -> Result<(Vec<Key>, Key), PlatformError> {
    let mut modifiers = Vec::new();
    let mut key = None;

    for raw in combo.split('+') {
        let token = raw.trim();
        if token.is_empty() {
            continue;
        }
        match token.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => modifiers.push(Key::Control),
            "shift" => modifiers.push(Key::Shift),
            "alt" | "option" => modifiers.push(Key::Alt),
            "meta" | "win" | "super" | "cmd" => modifiers.push(Key::Meta),
            other => {
                if key.is_some() {
                    return Err(invalid(combo, "more than one non modifier key"));
                }
                key = Some(
                    named_key(other)
                        .or_else(|| single_key(other))
                        .ok_or_else(|| invalid(combo, &format!("unknown key '{other}'")))?,
                );
            }
        }
    }

    let key = key.ok_or_else(|| invalid(combo, "missing the key to press"))?;
    if modifiers.is_empty() && combo.split('+').count() == 1 {
        // A single key is valid: it simply types that key.
    }
    Ok((modifiers, key))
}

fn single_key(token: &str) -> Option<Key> {
    let mut characters = token.chars();
    match (characters.next(), characters.next()) {
        (Some(character), None) => Some(Key::Unicode(character)),
        _ => None,
    }
}

fn named_key(token: &str) -> Option<Key> {
    let key = match token {
        "space" => Key::Space,
        "enter" | "return" => Key::Return,
        "tab" => Key::Tab,
        "escape" | "esc" => Key::Escape,
        "backspace" => Key::Backspace,
        "delete" | "del" => Key::Delete,
        "insert" | "ins" => Key::Insert,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" | "page_up" => Key::PageUp,
        "pagedown" | "page_down" => Key::PageDown,
        "up" | "arrowup" => Key::UpArrow,
        "down" | "arrowdown" => Key::DownArrow,
        "left" | "arrowleft" => Key::LeftArrow,
        "right" | "arrowright" => Key::RightArrow,
        "printscreen" | "print_scr" => Key::PrintScr,
        "f1" => Key::F1,
        "f2" => Key::F2,
        "f3" => Key::F3,
        "f4" => Key::F4,
        "f5" => Key::F5,
        "f6" => Key::F6,
        "f7" => Key::F7,
        "f8" => Key::F8,
        "f9" => Key::F9,
        "f10" => Key::F10,
        "f11" => Key::F11,
        "f12" => Key::F12,
        _ => return None,
    };
    Some(key)
}

fn invalid(combo: &str, reason: &str) -> PlatformError {
    PlatformError::InvalidHotkey {
        combo: combo.to_string(),
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_modifiers_and_letters() {
        let (modifiers, key) = parse_hotkey("ctrl+shift+p").expect("parse");
        assert_eq!(modifiers, vec![Key::Control, Key::Shift]);
        assert_eq!(key, Key::Unicode('p'));
    }

    #[test]
    fn parses_named_keys() {
        let (modifiers, key) = parse_hotkey("meta+space").expect("parse");
        assert_eq!(modifiers, vec![Key::Meta]);
        assert_eq!(key, Key::Space);
    }

    #[test]
    fn rejects_unknown_and_double_keys() {
        assert!(parse_hotkey("ctrl+nope").is_err());
        assert!(parse_hotkey("ctrl+a+b").is_err());
        assert!(parse_hotkey("ctrl").is_err());
    }

    #[test]
    fn parses_media_keys() {
        assert_eq!(MediaKey::parse("play_pause"), Some(MediaKey::PlayPause));
        assert_eq!(MediaKey::parse("next"), Some(MediaKey::Next));
        assert_eq!(MediaKey::parse("nah"), None);
    }
}
