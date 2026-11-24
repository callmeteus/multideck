//! Seeds a fresh profile so the first launch is not an empty grid.

use multideck_core::{ActionInstance, Page, Profile};
use serde_json::json;

/// Adds a small demo layout to a brand new profile.
pub fn apply(profile: &mut Profile) {
    let page = profile.active_page_mut();
    place(page, 3, hotkey("ctrl+alt+p"), None);
    place(
        page,
        4,
        open("https://github.com/callmeteus/multideck"),
        None,
    );
    place(page, 5, media("play_pause"), None);
    place(page, 10, text("Hello from MultiDeck"), None);
    place(page, 11, command("echo MultiDeck"), None);
    place(page, 14, brightness(85), None);
}

fn place(page: &mut Page, key: usize, action: ActionInstance, title: Option<&str>) {
    if let Some(slot) = page.slots.get_mut(key) {
        slot.action = Some(action);
        slot.title_override = title.map(str::to_string);
    }
}

fn hotkey(combo: &str) -> ActionInstance {
    let mut instance = ActionInstance::new("hotkey");
    instance.set("hotkey", json!(combo));
    instance
}

fn open(target: &str) -> ActionInstance {
    let mut instance = ActionInstance::new("open");
    instance.set("target", json!(target));
    instance
}

fn media(key: &str) -> ActionInstance {
    let mut instance = ActionInstance::new("media");
    instance.set("media", json!(key));
    instance
}

fn text(value: &str) -> ActionInstance {
    let mut instance = ActionInstance::new("text");
    instance.set("text", json!(value));
    instance
}

fn command(value: &str) -> ActionInstance {
    let mut instance = ActionInstance::new("command");
    instance.set("command", json!(value));
    instance
}

fn brightness(percent: u8) -> ActionInstance {
    let mut instance = ActionInstance::new("brightness");
    instance.set("percent", json!(percent.to_string()));
    instance
}
