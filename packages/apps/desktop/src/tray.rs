//! Windows system tray with a menu that can bring the window back or quit.
//!
//! The tray lives on its own thread with a Win32 message pump, because
//! `tray-icon` needs a message loop on the thread that created it and iced
//! owns the main loop. The UI polls the channel on its own timer, so no
//! window handle crosses threads.

use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, TrayIconBuilder};

/// Commands the tray can send to the application.
pub enum TrayCommand {
    /// Focus the main window.
    Show,
}

/// Starts the tray thread. Returns `None` when the icon cannot be created,
/// in which case the app simply runs without a tray.
pub fn spawn() -> Option<Receiver<TrayCommand>> {
    let (sender, receiver) = channel();
    let spawned = std::thread::Builder::new()
        .name("multideck-tray".into())
        .spawn(move || {
            let _ = run(sender);
        });

    match spawned {
        Ok(_) => Some(receiver),
        Err(error) => {
            tracing::warn!(%error, "could not start the tray thread");
            None
        }
    }
}

fn run(sender: std::sync::mpsc::Sender<TrayCommand>) -> Option<()> {
    let icon = load_icon()?;
    let open = MenuItem::new("Open MultiDeck", true, None);
    let quit = MenuItem::new("Quit MultiDeck", true, None);
    let menu = Menu::new();
    menu.append(&open).ok()?;
    menu.append(&quit).ok()?;

    let _tray = TrayIconBuilder::new()
        .with_tooltip("MultiDeck")
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .build()
        .map_err(|error| {
            tracing::warn!(%error, "could not create the tray icon");
        })
        .ok()?;

    loop {
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id() == open.id() {
                let _ = sender.send(TrayCommand::Show);
            } else if event.id() == quit.id() {
                std::process::exit(0);
            }
        }
        pump_messages();
        std::thread::sleep(Duration::from_millis(16));
    }
}

fn pump_messages() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, TranslateMessage,
    };

    let mut message: MSG = unsafe { std::mem::zeroed() };
    unsafe {
        while PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

fn load_icon() -> Option<Icon> {
    let bytes = include_bytes!("../../../../assets/icons/tray-32.png");
    let image = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (width, height) = image.dimensions();
    Icon::from_rgba(image.into_raw(), width, height)
        .map_err(|error| {
            tracing::warn!(%error, "could not build the tray icon");
        })
        .ok()
}
