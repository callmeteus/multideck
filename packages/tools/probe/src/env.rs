//! Environment report used by `probe env`.
//!
//! Everything here is read-only and resolved through a lookup closure so the
//! resolution rules can be tested without touching the real environment.

use std::path::Path;

/// Resolves the directory MultiDeck would store data in.
///
/// `MULTIDECK_DATA_DIR` wins when it is set and not blank. Otherwise the value
/// follows the usual per-platform configuration location:
///
/// * Windows: `%APPDATA%\MultiDeck`
/// * macOS: `$HOME/Library/Application Support/MultiDeck`
/// * other: `$XDG_CONFIG_HOME/multideck`, else `$HOME/.config/multideck`
///
/// `os` must be one of the strings from [`std::env::consts::OS`].
pub fn data_dir<F>(os: &str, lookup: F) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    if let Some(dir) = lookup("MULTIDECK_DATA_DIR").filter(|dir| !dir.trim().is_empty()) {
        return Some(dir);
    }

    match os {
        "windows" => lookup("APPDATA")
            .map(|base| join(&base, "MultiDeck"))
            .or_else(|| lookup("USERPROFILE").map(|base| join(&base, ".config"))),
        "macos" => lookup("HOME").map(|home| join(&home, "Library/Application Support/MultiDeck")),
        _ => lookup("XDG_CONFIG_HOME")
            .map(|base| join(&base, "multideck"))
            .or_else(|| lookup("HOME").map(|home| join(&home, ".config/multideck"))),
    }
}

/// Joins a base directory and a relative suffix with the host separator.
fn join(base: &str, suffix: &str) -> String {
    Path::new(base).join(suffix).display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn lookup_from(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect();
        move |key: &str| map.get(key).cloned()
    }

    #[test]
    fn explicit_override_wins() {
        let lookup = lookup_from(&[
            ("MULTIDECK_DATA_DIR", "D:/decks"),
            ("APPDATA", "C:/Users/x/AppData/Roaming"),
        ]);
        assert_eq!(data_dir("windows", lookup).as_deref(), Some("D:/decks"));
    }

    #[test]
    fn blank_override_is_ignored() {
        let lookup = lookup_from(&[
            ("MULTIDECK_DATA_DIR", "   "),
            ("APPDATA", "C:/Users/x/AppData/Roaming"),
        ]);
        let resolved = data_dir("windows", lookup).expect("appdata resolves");
        assert!(resolved.ends_with("MultiDeck"));
    }

    #[test]
    fn windows_falls_back_to_home() {
        let lookup = lookup_from(&[("USERPROFILE", "C:/Users/x")]);
        let resolved = data_dir("windows", lookup).expect("userprofile resolves");
        assert!(resolved.ends_with(".config"));
    }

    #[test]
    fn linux_prefers_xdg_config_home() {
        let lookup = lookup_from(&[("XDG_CONFIG_HOME", "/home/x/.config"), ("HOME", "/home/x")]);
        let resolved = data_dir("linux", lookup).expect("xdg resolves");
        assert!(resolved.ends_with("multideck"));
        assert!(resolved.contains(".config"));
    }

    #[test]
    fn linux_falls_back_to_home() {
        let lookup = lookup_from(&[("HOME", "/home/x")]);
        let resolved = data_dir("linux", lookup).expect("home resolves");
        assert_eq!(resolved, join("/home/x", ".config/multideck"));
    }

    #[test]
    fn nothing_set_yields_none() {
        let lookup = lookup_from(&[]);
        assert_eq!(data_dir("linux", lookup), None);
    }
}
