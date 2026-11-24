//! Application state and message handling.

use std::path::PathBuf;
use std::time::Duration;

use iced::Task;
use iced::widget::image::Handle as ImageHandle;
use multideck_actions::{self as actions, Effect};
use multideck_core::{ActionInstance, DeviceCaps, DeviceCommand, Profile};
use multideck_device_virtual::VirtualDeck;
use multideck_events::{AppEvent, ConnectedDevice, EventBus};
use multideck_i18n::{self as i18n, Locale};
use multideck_render::KeyRenderer;
use multideck_store::Store;
use serde_json::json;
use tokio::sync::broadcast;

use crate::seed;

/// Which screen is visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    /// Key editor_
    Editor,
    /// Application settings_
    Settings,
}

/// UI messages.
#[derive(Debug, Clone)]
pub enum Message {
    /// Selects a key, or assigns the pending action to it.
    SelectKey(u8),
    /// Removes the action from the selected key.
    ClearKey,
    /// Marks an action kind as the one to place on the next clicked key.
    PickAction(&'static str),
    /// Updates one inspector field_
    FieldChanged(usize, String),
    /// Writes inspector values into the action_
    ApplyInspector,
    /// Removes the selected action_
    DeleteAction,
    /// Changes the interface language.
    SelectLocale(Locale),
    /// Changes brightness.
    SetBrightness(u8),
    /// Adds a page to the active profile.
    NewPage,
    /// Shows another page.
    SelectPage(usize),
    /// Loads another profile.
    SelectProfile(String),
    /// Creates a profile.
    NewProfile,
    /// Simulates a press on a key.
    PressKey(u8),
    /// Opens the data directory.
    OpenDataFolder,
    /// Shows the editor_
    ShowEditor,
    /// Shows settings_
    ShowSettings,
    /// Drains device events from the bus.
    Poll,
}

/// The whole application state.
pub struct App {
    pub(crate) screen: Screen,
    pub(crate) profiles: Vec<Profile>,
    pub(crate) profile: Profile,
    pub(crate) caps: DeviceCaps,
    pub(crate) locale: Locale,
    pub(crate) selected_key: Option<u8>,
    pub(crate) pending_kind: Option<&'static str>,
    pub(crate) inspector: Vec<(String, String)>,
    pub(crate) key_images: Vec<Option<ImageHandle>>,
    pub(crate) status: String,
    pub(crate) data_dir: PathBuf,
    #[allow(dead_code)]
    runtime: tokio::runtime::Runtime,
    store: Store,
    renderer: KeyRenderer,
    bus: EventBus,
    events: broadcast::Receiver<AppEvent>,
    deck: VirtualDeck,
    #[allow(dead_code)]
    device: ConnectedDevice,
}

impl App {
    /// Builds the application, its data directory and the virtual deck.
    pub fn boot() -> (Self, Task<Message>) {
        match Self::new() {
            Ok(mut app) => {
                app.refresh();
                app.status = i18n::tr!("status_ready");
                (app, Task::none())
            }
            Err(error) => {
                tracing::error!(%error, "could not start MultiDeck");
                panic!("could not start MultiDeck: {error}");
            }
        }
    }

    fn new() -> Result<Self, String> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|error| format!("tokio runtime: {error}"))?;

        let mut store = Store::open().map_err(|error| error.to_string())?;
        let caps = DeviceCaps::default();
        let fresh = store.settings().active_profile.is_none();
        let mut profile = store
            .ensure_profile(&caps)
            .map_err(|error| error.to_string())?;
        if fresh {
            seed::apply(&mut profile);
            store
                .save_profile(&profile)
                .map_err(|error| error.to_string())?;
        }

        let locale = if store.settings().locale.is_empty() {
            Locale::from_system()
        } else {
            Locale::parse(&store.settings().locale)
        };
        i18n::set_locale(locale);

        let renderer = KeyRenderer::new();
        let (deck, adapter) = VirtualDeck::new();
        let bus = EventBus::new();
        let events = bus.subscribe();
        let device = multideck_events::connect(Box::new(adapter), &bus, runtime.handle());

        let profiles = store.list_profiles().unwrap_or_default();
        let data_dir = store.paths().root().to_path_buf();
        let status = String::new();
        let screen = match std::env::var("MULTIDECK_VIEW").as_deref() {
            Ok("settings") => Screen::Settings,
            _ => Screen::Editor,
        };

        Ok(Self {
            screen,
            profiles,
            profile,
            caps,
            locale,
            selected_key: None,
            pending_kind: None,
            inspector: Vec::new(),
            key_images: Vec::new(),
            status,
            data_dir,
            runtime,
            store,
            renderer,
            bus,
            events,
            deck,
            device,
        })
    }

    /// Window title.
    pub fn title(&self) -> String {
        i18n::tr!("app_window_title")
    }

    /// Root view, delegated to the `view` module.
    pub fn view(&self) -> iced::Element<'_, Message> {
        crate::view::view(self)
    }

    /// iced theme.
    pub fn theme(&self) -> iced::Theme {
        multideck_ui::theme()
    }

    /// Timer that drains device events.
    pub fn subscription(&self) -> iced::Subscription<Message> {
        iced::time::every(Duration::from_millis(250)).map(|_| Message::Poll)
    }

    /// Handles one message.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Poll => self.drain_events(),
            Message::SelectKey(key) => {
                if let Some(kind) = self.pending_kind.take() {
                    self.assign(key, kind);
                }
                self.select(key);
            }
            Message::ClearKey => self.delete_action(),
            Message::PickAction(kind) => {
                self.pending_kind = Some(kind);
                self.status = i18n::tr!("editor_assign_hint");
            }
            Message::FieldChanged(index, value) => {
                if let Some(field) = self.inspector.get_mut(index) {
                    field.1 = value;
                }
            }
            Message::ApplyInspector => self.apply_inspector(),
            Message::DeleteAction => self.delete_action(),
            Message::SelectLocale(locale) => self.set_locale(locale),
            Message::SetBrightness(percent) => self.set_brightness(percent),
            Message::NewPage => self.new_page(),
            Message::SelectPage(index) => {
                self.profile.active_page = index;
                self.persist();
                self.refresh();
            }
            Message::SelectProfile(id) => self.load_profile(&id),
            Message::NewProfile => self.new_profile(),
            Message::PressKey(key) => self.press(key),
            Message::OpenDataFolder => {
                if let Err(error) = open::that(&self.data_dir) {
                    tracing::warn!(%error, "could not open the data folder");
                }
            }
            Message::ShowEditor => self.screen = Screen::Editor,
            Message::ShowSettings => self.screen = Screen::Settings,
        }
        Task::none()
    }

    fn drain_events(&mut self) {
        loop {
            match self.events.try_recv() {
                Ok(AppEvent::DeviceDisconnected(device)) => {
                    self.bus.publish(AppEvent::Status(String::new()));
                    self.status = format!("{device} disconnected");
                }
                Ok(AppEvent::Status(status)) if !status.is_empty() => self.status = status,
                Ok(_) => {}
                Err(broadcast::error::TryRecvError::Empty) => break,
                Err(broadcast::error::TryRecvError::Closed) => break,
                Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
            }
        }
    }

    fn select(&mut self, key: u8) {
        self.selected_key = Some(key);
        self.load_inspector();
    }

    fn assign(&mut self, key: u8, kind: &str) {
        let Some(definition) = actions::definition(kind) else {
            self.status = format!("unknown action {kind}");
            return;
        };
        let instance = ActionInstance::from_def(definition);
        if let Some(slot) = self.profile.slot_mut(key) {
            slot.action = Some(instance);
        }
        self.selected_key = Some(key);
        self.persist();
        self.refresh();
        self.load_inspector();
        self.status = i18n::tr!("inspector_applied");
    }

    fn delete_action(&mut self) {
        let Some(key) = self.selected_key else {
            return;
        };
        if let Some(slot) = self.profile.slot_mut(key) {
            slot.action = None;
            slot.title_override = None;
        }
        self.inspector.clear();
        self.persist();
        self.refresh();
        self.status = i18n::tr!("status_saved");
    }

    fn load_inspector(&mut self) {
        self.inspector.clear();
        let Some(key) = self.selected_key else {
            return;
        };
        let Some(action) = self.profile.slot(key).and_then(|slot| slot.action.clone()) else {
            return;
        };
        let Some(definition) = actions::definition(&action.kind) else {
            return;
        };
        for field in definition.fields {
            let value = action
                .string(field.key)
                .map(str::to_string)
                .unwrap_or_else(|| field.default.to_string());
            self.inspector.push((field.key.to_string(), value));
        }
    }

    fn apply_inspector(&mut self) {
        let Some(key) = self.selected_key else {
            return;
        };
        let values = self.inspector.clone();
        let Some(action) = self
            .profile
            .slot_mut(key)
            .and_then(|slot| slot.action.as_mut())
        else {
            return;
        };
        for (field, value) in values {
            action.set(&field, json!(value));
        }
        self.persist();
        self.refresh();
        self.status = i18n::tr!("inspector_applied");
    }

    fn set_locale(&mut self, locale: Locale) {
        self.locale = locale;
        i18n::set_locale(locale);
        if let Err(error) = self.store.update_settings(|settings| {
            settings.locale = locale.tag().to_string();
        }) {
            tracing::warn!(%error, "could not persist the locale");
        }
        self.refresh();
        self.load_inspector();
        self.status = i18n::tr!("status_settings_saved");
    }

    fn set_brightness(&mut self, percent: u8) {
        self.profile.brightness = percent;
        let _ = self
            .device
            .commands
            .send(DeviceCommand::SetBrightness { percent });
        if let Err(error) = self
            .store
            .update_settings(|settings| settings.brightness = percent)
        {
            tracing::warn!(%error, "could not persist brightness");
        }
        self.persist();
    }

    fn new_page(&mut self) {
        let number = self.profile.pages.len() + 1;
        let name = i18n::tr_args("profiles_page_name", &[("number", number.to_string())]);
        let caps = self.caps.clone();
        self.profile.add_page(name, &caps);
        self.persist();
        self.refresh();
        self.status = i18n::tr!("status_saved");
    }

    fn new_profile(&mut self) {
        let mut profile = Profile::new(format!("Profile {}", self.profiles.len() + 1), &self.caps);
        profile.brightness = self.profile.brightness;
        if let Err(error) = self.store.save_profile(&profile) {
            tracing::warn!(%error, "could not create the profile");
            self.status = i18n::tr_args("errors_save_failed", &[("reason", error.to_string())]);
            return;
        }
        self.profiles = self.store.list_profiles().unwrap_or_default();
        self.adopt(profile);
        self.status = i18n::tr!("status_saved");
    }

    fn load_profile(&mut self, id: &str) {
        match self.store.load_profile(id) {
            Ok(profile) => {
                self.adopt(profile);
                self.status = i18n::tr!("status_saved");
            }
            Err(error) => {
                tracing::warn!(%error, %id, "could not load the profile");
                self.status = i18n::tr_args("errors_save_failed", &[("reason", error.to_string())]);
            }
        }
    }

    fn adopt(&mut self, mut profile: Profile) {
        profile.fit_to(&self.caps);
        self.profile = profile;
        self.selected_key = None;
        self.pending_kind = None;
        self.inspector.clear();
        self.persist();
        self.refresh();
        if let Err(error) = self.store.set_active_profile(&self.profile.id) {
            tracing::warn!(%error, "could not remember the active profile");
        }
    }

    fn press(&mut self, key: u8) {
        self.deck.press(key);
        let Some(action) = self.profile.slot(key).and_then(|slot| slot.action.clone()) else {
            return;
        };
        let title = actions::present(&action, key).title;
        match actions::execute(&action) {
            Ok(effects) => {
                self.apply_effects(effects);
                self.status = i18n::tr_args("status_action_ran", &[("action", title)]);
            }
            Err(error) => {
                tracing::warn!(%error, kind = %action.kind, "action failed");
                self.status =
                    i18n::tr_args("status_action_failed", &[("reason", error.to_string())]);
            }
        }
    }

    fn apply_effects(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::SwitchProfile(id) => self.load_profile(&id),
                Effect::SwitchPage(index) => {
                    if index < self.profile.pages.len() {
                        self.profile.active_page = index;
                        self.refresh();
                    }
                }
                Effect::SetBrightness(percent) => self.set_brightness(percent),
                Effect::Delay(millis) => {
                    tracing::debug!(millis, "delay effect ignored by the editor");
                }
            }
        }
    }

    fn persist(&mut self) {
        if let Err(error) = self.store.save_profile(&self.profile) {
            tracing::warn!(%error, "could not save the profile");
            self.status = i18n::tr_args("errors_save_failed", &[("reason", error.to_string())]);
        }
    }

    /// Renders every key and sends it to the device.
    pub(crate) fn refresh(&mut self) {
        let keys = self.caps.key_count();
        self.key_images.resize(keys as usize, None);
        for key in 0..keys {
            let view = self.key_view(key);
            let size = self.caps.key_image.0 as u32;
            let image = self.renderer.render(&view, size);
            let handle =
                ImageHandle::from_rgba(image.width(), image.height(), image.as_raw().clone());
            self.key_images[key as usize] = Some(handle);
            let _ = self
                .device
                .commands
                .send(DeviceCommand::SetKeyImage { key, image });
        }
        let _ = self.device.commands.send(DeviceCommand::SetBrightness {
            percent: self.profile.brightness,
        });
    }

    /// View model of one key, honouring overrides.
    pub(crate) fn key_view(&self, key: u8) -> multideck_core::KeyView {
        let slot = self.profile.slot(key);
        match slot.and_then(|slot| slot.action.as_ref()) {
            Some(action) => {
                let mut view = actions::present(action, key);
                if let Some(title) = slot.and_then(|slot| slot.title_override.clone()) {
                    view.title = title;
                }
                view
            }
            None => actions::empty(key),
        }
    }

    /// Action of the selected key, when there is one.
    pub(crate) fn selected_action(&self) -> Option<&ActionInstance> {
        let key = self.selected_key?;
        self.profile.slot(key).and_then(|slot| slot.action.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_profile_has_actions() {
        let caps = DeviceCaps::default();
        let mut profile = Profile::new("test", &caps);
        seed::apply(&mut profile);
        let actions = profile
            .active_page()
            .slots
            .iter()
            .filter(|slot| slot.action.is_some())
            .count();
        assert_eq!(actions, 6);
    }

    #[test]
    fn pressing_a_key_runs_its_action_and_renders_all_keys() {
        let directory = tempfile::tempdir().expect("temp dir");
        let marker = directory.path().join("marker.txt");
        // SAFETY: this test binary has a single test that reads the environment,
        // and it runs before the assertion below. No other test depends on it.
        unsafe {
            std::env::set_var("MULTIDECK_DATA_DIR", directory.path().join("data"));
        }

        let mut app = App::new().expect("app");
        app.refresh();
        assert_eq!(app.key_images.len(), app.caps.key_count() as usize);
        assert!(app.key_images.iter().all(Option::is_some));

        let mut action = ActionInstance::new("command");
        action.set(
            "command",
            json!(format!("echo ok>\"{}\"", marker.display())),
        );
        app.profile.slot_mut(0).expect("slot").action = Some(action);

        app.press(0);

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !marker.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(
            marker.exists(),
            "the command action should have written a file, status: {}",
            app.status
        );
    }
}
