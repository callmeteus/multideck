//! Translations for the MultiDeck user interface.
//!
//! Strings live in `locales/<tag>/*.ftl` at the repository root and are
//! compiled into the binary, so a missing or malformed translation is a build
//! error instead of a runtime surprise.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use fluent_templates::{Loader, fluent_bundle::FluentValue, static_loader};
use unic_langid::{LanguageIdentifier, langid};

static_loader! {
    static LOCALES = {
        locales: "../../../locales",
        fallback_language: "en-US",
    };
}

/// A language the user interface can be displayed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Locale {
    /// English, United States.
    #[default]
    EnUs,
    /// Portuguese, Brazil.
    PtBr,
}

impl Locale {
    /// Every supported locale, in menu order.
    pub const ALL: [Locale; 2] = [Locale::EnUs, Locale::PtBr];

    /// BCP 47 tag, also used as the file name of the translation directory.
    pub fn tag(self) -> &'static str {
        match self {
            Locale::EnUs => "en-US",
            Locale::PtBr => "pt-BR",
        }
    }

    /// Language identifier used by Fluent.
    pub fn langid(self) -> LanguageIdentifier {
        match self {
            Locale::EnUs => langid!("en-US"),
            Locale::PtBr => langid!("pt-BR"),
        }
    }

    /// Locale name written in its own language.
    pub fn native_name(self) -> &'static str {
        match self {
            Locale::EnUs => "English",
            Locale::PtBr => "Português (Brasil)",
        }
    }

    /// Parses a tag such as `pt-BR`, falling back to English.
    pub fn parse(tag: &str) -> Self {
        let normalized = tag.to_ascii_lowercase().replace('_', "-");
        if normalized.starts_with("pt") {
            Locale::PtBr
        } else {
            Locale::EnUs
        }
    }

    /// Detects the locale from the operating system, falling back to English.
    pub fn from_system() -> Self {
        sys_locale::get_locale()
            .map(|tag| Locale::parse(&tag))
            .unwrap_or(Locale::EnUs)
    }
}

impl std::fmt::Display for Locale {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.tag())
    }
}

fn current_slot() -> &'static RwLock<Locale> {
    static CURRENT: OnceLock<RwLock<Locale>> = OnceLock::new();
    CURRENT.get_or_init(|| RwLock::new(Locale::from_system()))
}

/// Returns the locale currently used by the user interface.
pub fn current() -> Locale {
    *current_slot().read().expect("locale lock is not poisoned")
}

/// Switches the locale used by [`translate`] and [`translate_args`].
pub fn set_locale(locale: Locale) {
    *current_slot().write().expect("locale lock is not poisoned") = locale;
}

/// Looks a message up in the current locale.
pub fn translate(key: &str) -> String {
    let locale = current();
    LOCALES
        .try_lookup(&locale.langid(), key)
        .or_else(|| LOCALES.try_lookup(&langid!("en-US"), key))
        .unwrap_or_else(|| key.to_string())
}

/// Convenience wrapper used by the UI screens.
pub fn tr(key: &str) -> String {
    translate(key)
}

/// Convenience wrapper used by the UI screens.
pub fn tr_args(key: &str, args: &[(&'static str, String)]) -> String {
    translate_args(key, args)
}

/// Looks a message up and interpolates the given arguments.
pub fn translate_args(key: &str, args: &[(&'static str, String)]) -> String {
    let data: HashMap<std::borrow::Cow<'static, str>, FluentValue<'_>> = args
        .iter()
        .map(|(name, value)| {
            (
                std::borrow::Cow::Borrowed(*name),
                FluentValue::from(value.clone()),
            )
        })
        .collect();

    let locale = current();
    LOCALES
        .try_lookup_with_args(&locale.langid(), key, &data)
        .or_else(|| LOCALES.try_lookup_with_args(&langid!("en-US"), key, &data))
        .unwrap_or_else(|| key.to_string())
}

/// Translates a message in the current locale.
#[macro_export]
macro_rules! tr {
    ($key:expr) => {
        $crate::translate($key)
    };
}

/// Translates a message with `name = value` arguments.
#[macro_export]
macro_rules! tr_args {
    ($key:expr, $($name:literal => $value:expr),+ $(,)?) => {
        $crate::translate_args($key, &[$(($name, $value.to_string())),+])
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    const EN: &str = include_str!("../../../../locales/en-US/main.ftl");
    const PT: &str = include_str!("../../../../locales/pt-BR/main.ftl");

    fn keys(source: &str) -> Vec<&str> {
        source
            .lines()
            .filter_map(|line| line.split_once('='))
            .map(|(key, _)| key.trim())
            .filter(|key| !key.is_empty() && !key.starts_with('#'))
            .collect()
    }

    #[test]
    fn locales_have_the_same_keys() {
        let mut english = keys(EN);
        let mut portuguese = keys(PT);
        english.sort_unstable();
        portuguese.sort_unstable();
        assert_eq!(english, portuguese, "en-US and pt-BR keys diverged");
    }

    #[test]
    fn lookup_works_and_falls_back() {
        set_locale(Locale::EnUs);
        assert_eq!(translate("app_name"), "MultiDeck");
        assert_eq!(
            translate("this.key.does.not.exist"),
            "this.key.does.not.exist"
        );

        set_locale(Locale::PtBr);
        assert_eq!(translate("app_name"), "MultiDeck");
        assert_ne!(translate("nav_devices"), "Devices");
        set_locale(Locale::EnUs);
    }

    #[test]
    fn arguments_are_interpolated() {
        let text = translate_args(
            "status_page_of",
            &[("current", "1".into()), ("total", "3".into())],
        );
        assert!(text.contains('1') && text.contains('3'), "got {text}");
    }
}
