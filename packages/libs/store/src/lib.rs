//! Persistence for settings and profiles_
//!
//! Everything lives under one data directory so the user can back up or move
//! their setup by copying a single folder:
//!
//! ```text
//! <root>/settings.toml
//! <root>/profiles/<id>.json
//! <root>/assets/
//! ```

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use multideck_core::{DeviceCaps, Profile};
use notify::Watcher;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Failures while reading or writing the data directory.
#[derive(Debug, Error)]
pub enum StoreError {
    /// A filesystem operation failed.
    #[error("{action} failed for {path}: {source}")]
    Io {
        /// What was being attempted.
        action: &'static str,
        /// Path involved.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },

    /// Settings could not be decoded.
    #[error("invalid settings: {0}")]
    Settings(String),

    /// A profile could not be decoded or written.
    #[error("invalid profile: {0}")]
    Profile(String),

    /// The profile directory could not be watched.
    #[error("could not watch profiles: {0}")]
    Watch(String),
}

/// Locations used by the store.
#[derive(Debug, Clone)]
pub struct Paths {
    root: PathBuf,
}

impl Paths {
    /// Builds paths rooted at the given directory.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Resolves the default data directory, honouring `MULTIDECK_DATA_DIR`.
    pub fn discover() -> Self {
        if let Ok(root) = std::env::var("MULTIDECK_DATA_DIR")
            && !root.trim().is_empty()
        {
            return Self::new(root);
        }
        let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        Self::new(base.join("multideck"))
    }

    /// Root of the data directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Directory holding profile files.
    pub fn profiles_dir(&self) -> PathBuf {
        self.root.join("profiles")
    }

    /// Directory holding user assets.
    pub fn assets_dir(&self) -> PathBuf {
        self.root.join("assets")
    }

    /// File holding application settings_
    pub fn settings_file(&self) -> PathBuf {
        self.root.join("settings.toml")
    }

    /// File holding one profile.
    pub fn profile_file(&self, id: &str) -> PathBuf {
        self.profiles_dir().join(format!("{}.json", sanitize(id)))
    }
}

/// Application settings persisted between runs.
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Locale tag; empty means "detect from the system".
    #[serde(default)]
    pub locale: String,
    /// Default brightness for devices_
    #[serde(default = "default_brightness")]
    pub brightness: u8,
    /// Id of the profile that was open when the app closed.
    #[serde(default)]
    pub active_profile: Option<String>,
    /// Workstation identity announced to Stream Deck Mobile.
    #[serde(default)]
    pub mobile_workstation_id: String,
    /// Network address selected for the pairing QR.
    #[serde(default)]
    pub mobile_address: String,
    /// Port kept between launches so the phone can reconnect to the same host.
    #[serde(default)]
    pub mobile_port: u16,
    /// Whether the free plan notice should still be offered.
    #[serde(default = "default_true")]
    pub mobile_free_notice: bool,
    /// Whether the editor shows only the six keys a free phone can use.
    #[serde(default)]
    pub mobile_free_layout: bool,
}

fn default_brightness() -> u8 {
    80
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            locale: String::new(),
            brightness: default_brightness(),
            active_profile: None,
            mobile_workstation_id: String::new(),
            mobile_address: String::new(),
            mobile_port: 0,
            mobile_free_notice: true,
            mobile_free_layout: false,
        }
    }
}

/// Watches the profiles directory so the app can reload edits made outside it.
pub struct ProfileWatcher {
    receiver: std::sync::mpsc::Receiver<()>,
    _watcher: notify::RecommendedWatcher,
}

impl ProfileWatcher {
    /// Drains pending events and reports whether any profile file changed.
    pub fn changed(&self) -> bool {
        let mut changed = false;
        while self.receiver.try_recv().is_ok() {
            changed = true;
        }
        changed
    }
}

/// Reads and writes the data directory.
#[derive(Debug, Clone)]
pub struct Store {
    paths: Paths,
    settings: Settings,
}

impl Store {
    /// Opens the default data directory, creating it if needed.
    pub fn open() -> Result<Self, StoreError> {
        Self::open_at(Paths::discover())
    }

    /// Opens a specific data directory, creating it if needed.
    pub fn open_at(paths: Paths) -> Result<Self, StoreError> {
        fs::create_dir_all(paths.profiles_dir()).map_err(|source| StoreError::Io {
            action: "create directory",
            path: paths.profiles_dir(),
            source,
        })?;
        fs::create_dir_all(paths.assets_dir()).map_err(|source| StoreError::Io {
            action: "create directory",
            path: paths.assets_dir(),
            source,
        })?;

        let settings = match fs::read_to_string(paths.settings_file()) {
            Ok(text) => match toml::from_str::<Settings>(&text) {
                Ok(settings) => settings,
                Err(error) => {
                    tracing::warn!(%error, "settings file is invalid, using defaults");
                    Settings::default()
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Settings::default(),
            Err(source) => {
                return Err(StoreError::Io {
                    action: "read settings",
                    path: paths.settings_file(),
                    source,
                });
            }
        };

        Ok(Self { paths, settings })
    }

    /// Data directory in use.
    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    /// Current settings_
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Mutates settings and writes them back.
    pub fn update_settings(
        &mut self,
        change: impl FnOnce(&mut Settings),
    ) -> Result<&Settings, StoreError> {
        let mut candidate = self.settings.clone();
        change(&mut candidate);
        self.write_settings(&candidate)?;
        self.settings = candidate;
        Ok(&self.settings)
    }

    /// Writes the settings file.
    pub fn save_settings(&self) -> Result<(), StoreError> {
        self.write_settings(&self.settings)
    }

    fn write_settings(&self, settings: &Settings) -> Result<(), StoreError> {
        let text = toml::to_string_pretty(settings)
            .map_err(|error| StoreError::Settings(error.to_string()))?;
        write_atomic(&self.paths.settings_file(), &text, "write settings")
    }

    /// Lists every readable profile, newest file first.
    pub fn list_profiles(&self) -> Result<Vec<Profile>, StoreError> {
        let mut profiles = Vec::new();
        let entries = fs::read_dir(self.paths.profiles_dir()).map_err(|source| StoreError::Io {
            action: "list profiles",
            path: self.paths.profiles_dir(),
            source,
        })?;

        for entry in entries {
            let entry = entry.map_err(|source| StoreError::Io {
                action: "list profiles",
                path: self.paths.profiles_dir(),
                source,
            })?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            match fs::read_to_string(&path) {
                Ok(text) => match Profile::from_json(&text) {
                    Ok(profile) => profiles.push(profile),
                    Err(error) => {
                        tracing::warn!(path = %path.display(), %error, "skipping invalid profile")
                    }
                },
                Err(error) => {
                    tracing::warn!(path = %path.display(), %error, "could not read profile")
                }
            }
        }

        profiles.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(profiles)
    }

    /// Loads one profile by id.
    pub fn load_profile(&self, id: &str) -> Result<Profile, StoreError> {
        let path = self.paths.profile_file(id);
        let text = fs::read_to_string(&path).map_err(|source| StoreError::Io {
            action: "read profile",
            path: path.clone(),
            source,
        })?;
        Profile::from_json(&text).map_err(|error| StoreError::Profile(error.to_string()))
    }

    /// Writes one profile.
    pub fn save_profile(&self, profile: &Profile) -> Result<(), StoreError> {
        let path = self.paths.profile_file(&profile.id);
        let text = profile
            .to_json()
            .map_err(|error| StoreError::Profile(error.to_string()))?;
        write_atomic(&path, &text, "write profile")
    }

    /// Deletes a profile file. Missing files are treated as already deleted.
    pub fn delete_profile(&self, id: &str) -> Result<(), StoreError> {
        let path = self.paths.profile_file(id);
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(StoreError::Io {
                action: "delete profile",
                path,
                source,
            }),
        }
    }

    /// Loads the active profile, creating a default one when there is none.
    pub fn ensure_profile(&mut self, caps: &DeviceCaps) -> Result<Profile, StoreError> {
        if let Some(id) = self.settings.active_profile.clone() {
            match self.load_profile(&id) {
                Ok(mut profile) => {
                    profile.fit_to(caps);
                    return Ok(profile);
                }
                Err(error) => {
                    tracing::warn!(%error, %id, "active profile could not be loaded");
                }
            }
        }

        let mut profile = Profile::new("Default", caps);
        profile.fit_to(caps);
        self.save_profile(&profile)?;
        self.update_settings(|settings| settings.active_profile = Some(profile.id.clone()))?;
        Ok(profile)
    }

    /// Remembers which profile is active.
    pub fn set_active_profile(&mut self, id: &str) -> Result<(), StoreError> {
        self.update_settings(|settings| settings.active_profile = Some(id.to_string()))?;
        Ok(())
    }

    /// Watches the profiles directory for external edits.
    pub fn watch_profiles(&self) -> Result<ProfileWatcher, StoreError> {
        let (sender, receiver) = std::sync::mpsc::channel();
        let mut watcher =
            notify::recommended_watcher(move |result: Result<notify::Event, notify::Error>| {
                if let Ok(event) = result
                    && (event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove())
                {
                    let _ = sender.send(());
                }
            })
            .map_err(|error| StoreError::Watch(error.to_string()))?;

        watcher
            .watch(
                &self.paths.profiles_dir(),
                notify::RecursiveMode::NonRecursive,
            )
            .map_err(|error| StoreError::Watch(error.to_string()))?;

        Ok(ProfileWatcher {
            receiver,
            _watcher: watcher,
        })
    }
}

fn write_atomic(path: &Path, contents: &str, action: &'static str) -> Result<(), StoreError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|source| StoreError::Io {
        action,
        path: path.to_path_buf(),
        source,
    })?;
    file.write_all(contents.as_bytes())
        .map_err(|source| StoreError::Io {
            action,
            path: path.to_path_buf(),
            source,
        })?;
    file.as_file().sync_all().map_err(|source| StoreError::Io {
        action,
        path: path.to_path_buf(),
        source,
    })?;
    file.persist(path).map_err(|error| StoreError::Io {
        action,
        path: path.to_path_buf(),
        source: error.error,
    })?;
    Ok(())
}

fn sanitize(id: &str) -> String {
    id.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use multideck_core::ActionInstance;

    #[test]
    fn creates_defaults_and_round_trips_a_profile() {
        let directory = tempfile::tempdir().expect("temp dir");
        let mut store = Store::open_at(Paths::new(directory.path())).expect("open store");
        let caps = DeviceCaps::default();

        let mut profile = store.ensure_profile(&caps).expect("default profile");
        assert_eq!(profile.pages.len(), 1);
        assert_eq!(profile.active_page().slots.len(), caps.key_count() as usize);

        profile.slot_mut(0).expect("slot").action = Some(ActionInstance::new("hotkey"));
        store.save_profile(&profile).expect("save");

        let loaded = store.load_profile(&profile.id).expect("load");
        assert_eq!(loaded, profile);
        assert_eq!(
            store.settings().active_profile.as_deref(),
            Some(profile.id.as_str())
        );
    }

    #[test]
    fn settings_are_persisted() {
        let directory = tempfile::tempdir().expect("temp dir");
        let paths = Paths::new(directory.path());

        {
            let mut store = Store::open_at(paths.clone()).expect("open store");
            store
                .update_settings(|settings| {
                    settings.locale = "pt-BR".into();
                    settings.brightness = 42;
                })
                .expect("save");
        }

        let store = Store::open_at(paths).expect("reopen store");
        assert_eq!(store.settings().locale, "pt-BR");
        assert_eq!(store.settings().brightness, 42);
    }

    #[test]
    fn failed_settings_write_keeps_memory_and_disk_unchanged() {
        let directory = tempfile::tempdir().expect("temp dir");
        let paths = Paths::new(directory.path());
        let mut store = Store::open_at(paths.clone()).expect("open store");
        store.save_settings().expect("save settings");

        let settings_file = paths.settings_file();
        let backup = directory.path().join("settings.backup");
        fs::rename(&settings_file, &backup).expect("move settings aside");
        fs::create_dir(&settings_file).expect("block settings path");

        let result = store.update_settings(|settings| settings.locale = "pt-BR".into());

        assert!(result.is_err());
        assert_eq!(store.settings().locale, "");

        fs::remove_dir(&settings_file).expect("remove blocked path");
        fs::rename(&backup, &settings_file).expect("restore settings");
        let reopened = Store::open_at(paths).expect("reopen store");
        assert_eq!(reopened.settings().locale, "");
    }

    #[test]
    fn atomic_writes_leave_no_temporary_files() {
        let directory = tempfile::tempdir().expect("temp dir");
        let paths = Paths::new(directory.path());
        let store = Store::open_at(paths.clone()).expect("open store");
        store.save_settings().expect("save settings");
        store
            .save_profile(&Profile::new("Atomic", &DeviceCaps::default()))
            .expect("save profile");

        for folder in [paths.root(), paths.profiles_dir().as_path()] {
            for entry in fs::read_dir(folder).expect("read folder") {
                let name = entry.expect("entry").file_name();
                assert!(
                    !name.to_string_lossy().ends_with(".tmp"),
                    "temporary file left behind: {}",
                    name.to_string_lossy()
                );
            }
        }
    }

    #[test]
    fn lists_profiles() {
        let directory = tempfile::tempdir().expect("temp dir");
        let store = Store::open_at(Paths::new(directory.path())).expect("open store");
        let caps = DeviceCaps::default();
        store
            .save_profile(&Profile::new("Alpha", &caps))
            .expect("save");
        store
            .save_profile(&Profile::new("Beta", &caps))
            .expect("save");

        let names: Vec<String> = store
            .list_profiles()
            .expect("list")
            .into_iter()
            .map(|profile| profile.name)
            .collect();
        assert_eq!(names, vec!["Alpha".to_string(), "Beta".to_string()]);
    }

    #[test]
    fn watcher_reports_external_profile_changes() {
        let directory = tempfile::tempdir().expect("temp dir");
        let store = Store::open_at(Paths::new(directory.path())).expect("open store");
        let watcher = store.watch_profiles().expect("watch");
        let caps = DeviceCaps::default();

        store
            .save_profile(&Profile::new("External", &caps))
            .expect("save");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            if watcher.changed() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        panic!("the watcher did not report the new profile");
    }
}
