//! Application state and message handling.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::Duration;

use iced::Task;
use iced::widget::image::Handle as ImageHandle;
use multideck_actions::{self as actions, Effect};
use multideck_core::{
    ActionInstance, DeviceCaps, DeviceCommand, DeviceDescriptor, DeviceInput, Profile,
};
use multideck_device_api::DeviceAdapter;
use multideck_device_sdmobile::{MobileConfig, MobileDeck, NetworkInterface};
use multideck_device_virtual::VirtualDeck;
use multideck_events::{AppEvent, ConnectedDevice, EventBus};
use multideck_i18n::{self as i18n, Locale};
use multideck_render::KeyRenderer;
use multideck_store::{ProfileWatcher, Store};
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
    /// Opens the context actions for a profile.
    OpenProfileMenu(String),
    /// Closes the profile context menu.
    CloseProfileMenu,
    /// Tracks the mouse position for the context menu.
    CursorMoved(f32, f32),
    /// Filters the action palette.
    PaletteQuery(String),
    /// Marks an element as hovered.
    Hover(String),
    /// Clears the hovered element.
    Unhover,
    /// Closes the profile context dialog.
    CloseProfileDialog,
    /// Switches the context dialog to rename mode.
    StartProfileRename,
    /// Edits the rename draft.
    ProfileRenameDraft(String),
    /// Commits the rename.
    ConfirmProfileRename,
    /// Switches the context dialog to delete confirmation.
    StartProfileDelete,
    /// Deletes the profile.
    ConfirmProfileDelete,
    /// Simulates a press on a key.
    PressKey(u8),
    /// Starts dragging an action from the palette.
    StartDrag(&'static str),
    /// Drops the dragged action on a key.
    DropOnKey(u8),
    /// Cancels the current drag.
    CancelDrag,
    /// Asks for confirmation before removing the action of the selected key.
    RequestDelete,
    /// Confirms the removal.
    ConfirmDelete,
    /// Closes the confirmation without removing anything.
    CancelDelete,
    /// Keyboard or timer event without an action.
    Noop,
    /// Trusts the phone waiting for pairing.
    PairAccept,
    /// Chooses which local address goes into the pairing QR.
    SelectMobileAddress(String),
    /// Rejects the phone waiting for pairing.
    PairReject,
    /// Opens the pairing modal.
    OpenPairing,
    /// Dismisses the free plan notice.
    CloseLimitNotice,
    /// Rebuilds the profile for the free 3x2 mobile grid.
    ApplyFreeLayout,
    /// Closes the pairing modal.
    ClosePairing,
    /// Opens the data directory.
    OpenDataFolder,
    /// Shows the editor screen.
    ShowEditor,
    /// Shows the settings screen.
    ShowSettings,
    /// Drains device events from the bus and checks for external profile edits.
    Poll,
    /// Continues the effect queue after a delay.
    EffectsReady,
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
    pub(crate) dragging: Option<&'static str>,
    pub(crate) pending_delete: Option<u8>,
    pub(crate) pairing_code: Option<String>,
    pub(crate) qr: Option<(String, ImageHandle)>,
    pub(crate) show_pairing: bool,
    pub(crate) mobile_interfaces: Vec<NetworkInterface>,
    pub(crate) mobile_address: Option<String>,
    pub(crate) mobile_connected: bool,
    pub(crate) mobile_pro: Option<bool>,
    pub(crate) limit_notice: bool,
    pub(crate) free_layout: bool,
    pub(crate) profile_menu: Option<String>,
    pub(crate) menu_pos: Option<(f32, f32)>,
    pub(crate) cursor: (f32, f32),
    pub(crate) profile_rename: Option<(String, String)>,
    pub(crate) profile_delete: Option<String>,
    pub(crate) queued_press: VecDeque<u8>,
    pub(crate) palette_query: String,
    pub(crate) hover: Option<String>,
    pub(crate) inspector: Vec<(String, String)>,
    pub(crate) key_images: Vec<Option<ImageHandle>>,
    pub(crate) status: String,
    pub(crate) data_dir: PathBuf,
    pub(crate) device_descriptor: DeviceDescriptor,
    pending_effects: VecDeque<Effect>,
    effects_waiting: bool,
    watcher: Option<ProfileWatcher>,
    #[cfg(windows)]
    tray: Option<std::sync::mpsc::Receiver<crate::tray::TrayCommand>>,
    #[allow(dead_code)]
    runtime: tokio::runtime::Runtime,
    store: Store,
    renderer: KeyRenderer,
    bus: EventBus,
    events: broadcast::Receiver<AppEvent>,
    deck: Option<VirtualDeck>,
    mobile: Option<MobileDeck>,
    mobile_port: Option<u16>,
    #[allow(dead_code)]
    device: ConnectedDevice,
}

impl App {
    /// Builds the application, its data directory and the virtual deck.
    pub fn boot() -> (Self, Task<Message>) {
        match Self::new() {
            Ok(mut app) => {
                app.refresh();
                if let Ok(value) = std::env::var("MULTIDECK_SELECT_KEY")
                    && let Ok(key) = value.parse::<u8>()
                {
                    app.select(key);
                }
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

        let requested = std::env::var("MULTIDECK_DEVICE")
            .unwrap_or_default()
            .to_ascii_lowercase();

        let detected = if requested == "virtual" || requested == "mobile" {
            Vec::new()
        } else {
            multideck_device_elgato::discover()
        };

        let (descriptor, deck, adapter, mobile): (
            DeviceDescriptor,
            Option<VirtualDeck>,
            Box<dyn DeviceAdapter>,
            Option<MobileDeck>,
        ) = if requested == "mobile" {
            let workstation_id = if store.settings().mobile_workstation_id.is_empty() {
                let generated = multideck_device_sdmobile::generate_workstation_id();
                store
                    .update_settings(|settings| {
                        settings.mobile_workstation_id = generated.clone();
                    })
                    .map_err(|error| error.to_string())?;
                generated
            } else {
                store.settings().mobile_workstation_id.clone()
            };
            let hostname = std::env::var("COMPUTERNAME")
                .or_else(|_| std::env::var("HOSTNAME"))
                .unwrap_or_else(|_| "multideck".to_string());
            let mut config = MobileConfig::new(workstation_id, hostname);
            config.trust_file = Some(store.paths().root().join("mobile_trusted.txt"));
            config.key_file = Some(store.paths().root().join("mobile_key.txt"));
            config.port = store.settings().mobile_port;
            config.auto_trust = std::env::var("MULTIDECK_MOBILE_TRUST")
                .map(|value| value == "1")
                .unwrap_or(false);
            let (mobile, adapter) = MobileDeck::new(config);
            let descriptor = mobile.descriptor().clone();
            (descriptor, None, Box::new(adapter), Some(mobile))
        } else {
            match detected.into_iter().next() {
                Some(found) => {
                    let descriptor = found.descriptor.clone();
                    (descriptor, None, Box::new(found.into_adapter()), None)
                }
                None => {
                    let (deck, adapter) = VirtualDeck::new();
                    let descriptor = deck.descriptor().clone();
                    (descriptor, Some(deck), Box::new(adapter), None)
                }
            }
        };

        let mobile_interfaces = if mobile.is_some() {
            multideck_device_sdmobile::interfaces()
        } else {
            Vec::new()
        };
        let mobile_address = if mobile.is_some() {
            let saved = store.settings().mobile_address.clone();
            let still_present = mobile_interfaces
                .iter()
                .any(|interface| interface.address == saved);
            if saved.is_empty() || !still_present {
                None
            } else {
                Some(saved)
            }
        } else {
            None
        };

        let caps = descriptor.caps.clone();
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
        let bus = EventBus::new();
        let events = bus.subscribe();
        let device = multideck_events::connect(adapter, &bus, runtime.handle());

        let profiles = store.list_profiles().unwrap_or_default();
        let data_dir = store.paths().root().to_path_buf();
        let status = String::new();
        let screen = match std::env::var("MULTIDECK_VIEW").as_deref() {
            Ok("settings") => Screen::Settings,
            _ => Screen::Editor,
        };
        #[cfg(windows)]
        let tray = crate::tray::spawn();

        let open_pairing =
            std::env::var("MULTIDECK_OPEN_PAIRING").is_ok() && !mobile_interfaces.is_empty();

        Ok(Self {
            screen,
            profiles,
            profile,
            caps,
            locale,
            selected_key: None,
            pending_kind: None,
            dragging: None,
            pending_delete: None,
            pairing_code: None,
            qr: None,
            show_pairing: open_pairing,
            mobile_interfaces,
            mobile_address,
            mobile_connected: false,
            mobile_pro: None,
            limit_notice: false,
            free_layout: store.settings().mobile_free_layout,
            profile_menu: None,
            menu_pos: None,
            cursor: (0.0, 0.0),
            profile_rename: None,
            profile_delete: None,
            queued_press: VecDeque::new(),
            palette_query: String::new(),
            hover: None,
            inspector: Vec::new(),
            key_images: Vec::new(),
            status,
            data_dir,
            device_descriptor: descriptor,
            pending_effects: VecDeque::new(),
            effects_waiting: false,
            watcher: store.watch_profiles().ok(),
            #[cfg(windows)]
            tray,
            runtime,
            store,
            renderer,
            bus,
            events,
            deck,
            mobile,
            mobile_port: None,
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
        iced::Subscription::batch([
            iced::time::every(Duration::from_millis(250)).map(|_| Message::Poll),
            iced::event::listen_with(cursor_listener),
            iced::keyboard::listen().map(|event| match event {
                iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Delete),
                    ..
                } => Message::RequestDelete,
                iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                    ..
                } => Message::CancelDelete,
                _ => Message::Noop,
            }),
        ])
    }

    /// Handles one message.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Poll => {
                self.drain_events();
                let task = self.run_queued_presses();
                if self.effects_waiting {
                    return task;
                }
                if self.watcher.as_ref().is_some_and(ProfileWatcher::changed) {
                    self.reload_from_disk();
                }
                #[cfg(windows)]
                if let Some(receiver) = &self.tray
                    && receiver.try_recv().is_ok()
                {
                    return iced::window::latest().and_then(iced::window::gain_focus);
                }
                if let Some(mobile) = &self.mobile {
                    self.mobile_connected = mobile.is_connected();
                    self.mobile_pro = mobile.is_pro();
                    if self.mobile_pro == Some(false)
                        && !self.free_layout
                        && self.store.settings().mobile_free_notice
                    {
                        self.limit_notice = true;
                    } else if self.mobile_pro == Some(true) {
                        self.limit_notice = false;
                    }
                    let port = mobile.port();
                    if port != self.mobile_port {
                        self.mobile_port = port;
                        self.status = match port {
                            Some(port) => i18n::tr_args(
                                "status_mobile_waiting",
                                &[("port", port.to_string())],
                            ),
                            None => i18n::tr!("status_mobile_starting"),
                        };
                        if let Some(port) = port
                            && self.store.settings().mobile_port != port
                        {
                            let _ = self
                                .store
                                .update_settings(|settings| settings.mobile_port = port);
                        }
                    }
                    let chosen_addresses: Vec<String> = match &self.mobile_address {
                        Some(address) => vec![address.clone()],
                        None => self
                            .mobile_interfaces
                            .iter()
                            .map(|interface| interface.address.clone())
                            .collect(),
                    };
                    if self.pairing_code.is_none()
                        && let Some(url) = mobile.pairing_url_for(&chosen_addresses)
                    {
                        let stale = self
                            .qr
                            .as_ref()
                            .map(|(known, _)| known != &url)
                            .unwrap_or(true);
                        if stale && let Some(handle) = qr_handle(&url) {
                            self.qr = Some((url, handle));
                        }
                    }
                    let code = mobile.pending_pairing();
                    if code != self.pairing_code {
                        self.pairing_code = code;
                        if let Some(code) = &self.pairing_code {
                            self.status =
                                i18n::tr_args("status_mobile_pairing", &[("code", code.clone())]);
                            self.show_pairing = true;
                        }
                    }
                }
            }
            Message::SelectKey(key) => {
                if let Some(kind) = self.pending_kind.take() {
                    self.assign(key, kind);
                }
                self.select(key);
            }
            Message::ClearKey => self.request_delete(),
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
            Message::DeleteAction => self.request_delete(),
            Message::SelectLocale(locale) => self.set_locale(locale),
            Message::SetBrightness(percent) => self.set_brightness(percent),
            Message::NewPage => self.new_page(),
            Message::SelectPage(index) => {
                self.profile.active_page = index;
                self.persist();
                self.refresh();
            }
            Message::SelectProfile(id) => self.load_profile(&id),
            Message::OpenProfileMenu(id) => {
                self.profile_menu = Some(id);
                self.menu_pos = Some((self.cursor.0, self.cursor.1));
                self.profile_rename = None;
                self.profile_delete = None;
            }
            Message::CursorMoved(x, y) => self.cursor = (x, y),
            Message::PaletteQuery(query) => self.palette_query = query,
            Message::Hover(id) => self.hover = Some(id),
            Message::Unhover => self.hover = None,
            Message::CloseProfileMenu => self.profile_menu = None,
            Message::CloseProfileDialog => {
                self.profile_menu = None;
                self.profile_rename = None;
                self.profile_delete = None;
            }
            Message::StartProfileRename => {
                if let Some(id) = self.profile_menu.take() {
                    let name = self
                        .profiles
                        .iter()
                        .find(|profile| profile.id == id)
                        .map(|profile| profile.name.clone())
                        .or_else(|| (self.profile.id == id).then(|| self.profile.name.clone()))
                        .unwrap_or_default();
                    self.profile_rename = Some((id, name));
                }
            }
            Message::ProfileRenameDraft(text) => {
                if let Some((_, draft)) = &mut self.profile_rename {
                    *draft = text;
                }
            }
            Message::ConfirmProfileRename => self.confirm_profile_rename(),
            Message::StartProfileDelete => {
                if let Some(id) = self.profile_menu.take() {
                    self.profile_delete = Some(id);
                }
            }
            Message::ConfirmProfileDelete => self.confirm_profile_delete(),
            Message::NewProfile => self.new_profile(),
            Message::PressKey(key) => return self.press(key),
            Message::EffectsReady => return self.resume_pending(),
            Message::StartDrag(kind) => {
                self.dragging = Some(kind);
                self.status = i18n::tr!("editor_drag_hint");
            }
            Message::DropOnKey(key) => {
                if let Some(kind) = self.dragging.take() {
                    self.assign(key, kind);
                }
                self.select(key);
            }
            Message::CancelDrag => self.dragging = None,
            Message::RequestDelete => self.request_delete(),
            Message::ConfirmDelete => self.confirm_delete(),
            Message::CancelDelete => {
                self.pending_delete = None;
                self.dragging = None;
            }
            Message::Noop => {}
            Message::PairAccept => self.resolve_pairing(true),
            Message::PairReject => self.resolve_pairing(false),
            Message::OpenPairing => self.show_pairing = true,
            Message::ClosePairing => self.close_pairing(),
            Message::CloseLimitNotice => {
                self.limit_notice = false;
                let _ = self
                    .store
                    .update_settings(|settings| settings.mobile_free_notice = false);
            }
            Message::ApplyFreeLayout => self.apply_free_layout(),
            Message::SelectMobileAddress(address) => {
                self.mobile_address = Some(address.clone());
                if let Err(error) = self
                    .store
                    .update_settings(|settings| settings.mobile_address = address)
                {
                    tracing::warn!(%error, "could not persist the mobile address");
                }
                self.qr = None;
            }
            Message::OpenDataFolder => {
                if let Err(error) =
                    multideck_platform::open_target(&self.data_dir.to_string_lossy())
                {
                    tracing::warn!(%error, "could not open the data folder");
                }
            }
            Message::ShowEditor => self.screen = Screen::Editor,
            Message::ShowSettings => self.screen = Screen::Settings,
        }
        Task::none()
    }

    fn reload_from_disk(&mut self) {
        let Ok(mut profile) = self.store.load_profile(&self.profile.id) else {
            return;
        };
        profile.fit_to(&self.caps);
        if profile == self.profile {
            return;
        }
        self.profile = profile;
        self.refresh();
        self.status = i18n::tr!("status_saved");
    }

    fn drain_events(&mut self) {
        loop {
            match self.events.try_recv() {
                Ok(AppEvent::DeviceDisconnected(device)) => {
                    self.bus.publish(AppEvent::Status(String::new()));
                    self.status = format!("{device} disconnected");
                }
                Ok(AppEvent::Status(status)) if !status.is_empty() => self.status = status,
                Ok(AppEvent::Input {
                    input: DeviceInput::KeyDown { key },
                    ..
                }) => self.queued_press.push_back(key),
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
        self.dragging = None;
        self.pending_kind = None;
        self.persist();
        self.refresh();
        self.load_inspector();
        self.status = i18n::tr!("inspector_applied");
    }

    fn resolve_pairing(&mut self, accept: bool) {
        if let Some(mobile) = &self.mobile {
            mobile.resolve_pairing(accept);
        }
        self.pairing_code = None;
        self.show_pairing = false;
        self.status = if accept {
            i18n::tr!("status_mobile_paired")
        } else {
            i18n::tr!("status_mobile_rejected")
        };
    }
    fn close_pairing(&mut self) {
        if self.pairing_code.is_some() {
            self.resolve_pairing(false);
        }
        self.show_pairing = false;
    }

    fn request_delete(&mut self) {
        let Some(key) = self.selected_key else {
            return;
        };
        let has_action = self
            .profile
            .slot(key)
            .and_then(|slot| slot.action.as_ref())
            .is_some();
        if has_action {
            self.pending_delete = Some(key);
            self.status = i18n::tr!("confirm_delete_title");
        }
    }

    fn confirm_delete(&mut self) {
        let Some(key) = self.pending_delete.take() else {
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

    fn apply_free_layout(&mut self) {
        // The free app keeps its own 5x3 keypad and only unlocks the first three
        // keys of the first two rows, so the actions have to land on the real
        // phone indexes: 0..2 and 5..7. Shrinking our grid to 3x2 put keys on
        // indexes 3 and 4, which the free app does not accept.
        const FREE_SLOTS: [usize; 6] = [0, 1, 2, 5, 6, 7];
        let mut profile = Profile::new(i18n::tr!("profile_free_mobile"), &self.caps);
        profile.brightness = self.profile.brightness;
        let source: Vec<_> = self
            .profile
            .active_page()
            .slots
            .iter()
            .filter(|slot| !slot.is_empty())
            .cloned()
            .collect();
        for (target, source) in FREE_SLOTS.iter().zip(source) {
            if let Some(slot) = profile.pages[0].slots.get_mut(*target) {
                *slot = source;
            }
        }
        if let Err(error) = self.store.save_profile(&profile) {
            tracing::warn!(%error, "could not create the free mobile profile");
            self.status = i18n::tr_args("errors_save_failed", &[("reason", error.to_string())]);
            return;
        }
        if let Err(error) = self.store.set_active_profile(&profile.id) {
            tracing::warn!(%error, "could not activate the free mobile profile");
        }
        self.profiles = self.store.list_profiles().unwrap_or_default();
        self.adopt(profile);
        self.limit_notice = false;
        self.free_layout = true;
        let _ = self.store.update_settings(|settings| {
            settings.mobile_free_notice = false;
            settings.mobile_free_layout = true;
        });
        self.status = i18n::tr!("status_saved");
    }

    fn confirm_profile_rename(&mut self) {
        let Some((id, draft)) = self.profile_rename.take() else {
            return;
        };
        let name = draft.trim().to_string();
        if name.is_empty() {
            return;
        }
        match self.store.load_profile(&id) {
            Ok(mut profile) => {
                profile.name = name.clone();
                if let Err(error) = self.store.save_profile(&profile) {
                    tracing::warn!(%error, "could not rename the profile");
                    self.status =
                        i18n::tr_args("errors_save_failed", &[("reason", error.to_string())]);
                    return;
                }
            }
            Err(error) => {
                tracing::warn!(%error, "could not load the profile to rename");
                self.status = i18n::tr_args("errors_save_failed", &[("reason", error.to_string())]);
                return;
            }
        }
        if self.profile.id == id {
            self.profile.name = name;
        }
        self.profiles = self.store.list_profiles().unwrap_or_default();
        self.status = i18n::tr!("status_saved");
    }

    fn confirm_profile_delete(&mut self) {
        let Some(id) = self.profile_delete.take() else {
            return;
        };
        if self.profiles.len() <= 1 {
            self.status = i18n::tr!("status_last_profile");
            return;
        }
        if let Err(error) = self.store.delete_profile(&id) {
            tracing::warn!(%error, "could not delete the profile");
            self.status = i18n::tr_args("errors_save_failed", &[("reason", error.to_string())]);
            return;
        }
        self.profiles = self.store.list_profiles().unwrap_or_default();
        if self.profile.id == id
            && let Some(next) = self.profiles.first().cloned()
        {
            let _ = self.store.set_active_profile(&next.id);
            self.adopt(next);
        }
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

    fn press(&mut self, key: u8) -> Task<Message> {
        if let Some(deck) = &self.deck {
            deck.press(key);
        }
        let Some(action) = self.profile.slot(key).and_then(|slot| slot.action.clone()) else {
            return Task::none();
        };
        let title = actions::present(&action, key).title;
        match actions::execute(&action) {
            Ok(effects) => {
                self.status = i18n::tr_args("status_action_ran", &[("action", title)]);
                self.pending_effects.extend(effects);
                self.run_pending()
            }
            Err(error) => {
                tracing::warn!(%error, kind = %action.kind, "action failed");
                self.status =
                    i18n::tr_args("status_action_failed", &[("reason", error.to_string())]);
                Task::none()
            }
        }
    }

    fn run_queued_presses(&mut self) -> Task<Message> {
        while let Some(key) = self.queued_press.pop_front() {
            let task = self.press(key);
            if self.effects_waiting {
                return task;
            }
        }
        Task::none()
    }

    fn resume_pending(&mut self) -> Task<Message> {
        self.effects_waiting = false;
        let task = self.run_pending();
        if self.effects_waiting {
            return task;
        }
        self.run_queued_presses()
    }

    fn run_pending(&mut self) -> Task<Message> {
        while let Some(effect) = self.pending_effects.pop_front() {
            match effect {
                Effect::Delay(millis) if millis > 0 => {
                    self.effects_waiting = true;
                    return Task::perform(
                        async move { tokio::time::sleep(Duration::from_millis(millis)).await },
                        |_| Message::EffectsReady,
                    );
                }
                Effect::Delay(_) => continue,
                other => self.apply_effect(other),
            }
        }
        self.effects_waiting = false;
        Task::none()
    }

    fn apply_effect(&mut self, effect: Effect) {
        match effect {
            Effect::SwitchProfile(id) => self.load_profile(&id),
            Effect::SwitchPage(index) => {
                if index < self.profile.pages.len() {
                    self.profile.active_page = index;
                    self.refresh();
                }
            }
            Effect::SetBrightness(percent) => self.set_brightness(percent),
            Effect::Delay(_) => {}
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

    /// Whether this run is hosting the mobile deck.
    pub(crate) fn mobile_enabled(&self) -> bool {
        self.mobile.is_some()
    }

    /// Action of the selected key, when there is one.
    pub(crate) fn selected_action(&self) -> Option<&ActionInstance> {
        let key = self.selected_key?;
        self.profile.slot(key).and_then(|slot| slot.action.as_ref())
    }
}

fn qr_handle(text: &str) -> Option<ImageHandle> {
    let code = qrcode::QrCode::new(text.as_bytes()).ok()?;
    let image = code
        .render::<image::Luma<u8>>()
        .min_dimensions(240, 240)
        .build();
    let rgba = image::DynamicImage::ImageLuma8(image).to_rgba8();
    Some(ImageHandle::from_rgba(
        rgba.width(),
        rgba.height(),
        rgba.as_raw().clone(),
    ))
}

fn cursor_listener(
    event: iced::Event,
    _status: iced::event::Status,
    _window: iced::window::Id,
) -> Option<Message> {
    match event {
        iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
            Some(Message::CursorMoved(position.x, position.y))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
