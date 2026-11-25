use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::json;

use super::*;

static DATA_DIR_LOCK: Mutex<()> = Mutex::new(());

fn test_app() -> (tempfile::TempDir, App) {
    let directory = tempfile::tempdir().expect("temp dir");
    // SAFETY: tests that call App::new hold DATA_DIR_LOCK, so no other test
    // changes these variables while the application reads them.
    unsafe {
        std::env::set_var("MULTIDECK_DATA_DIR", directory.path().join("data"));
        std::env::set_var("MULTIDECK_DEVICE", "virtual");
    }
    let app = App::new().expect("app");
    (directory, app)
}

fn brightness_action(percent: u8) -> ActionInstance {
    let mut action = ActionInstance::new("brightness");
    action.set("percent", json!(percent.to_string()));
    action
}

fn publish_key_down(app: &App, key: u8) {
    app.bus.publish(AppEvent::Input {
        device: "test".to_string(),
        input: DeviceInput::KeyDown { key },
    });
}

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
fn poll_runs_every_key_down_in_order() {
    let _guard = DATA_DIR_LOCK.lock().expect("data dir lock");
    let (_directory, mut app) = test_app();
    app.profile.slot_mut(0).expect("slot").action = Some(brightness_action(10));
    app.profile.slot_mut(1).expect("slot").action = Some(brightness_action(20));

    publish_key_down(&app, 0);
    publish_key_down(&app, 1);

    let _ = app.update(Message::Poll);

    assert!(app.queued_press.is_empty());
    assert_eq!(app.profile.brightness, 20);
}

#[test]
fn queued_presses_wait_for_the_current_delay() {
    let _guard = DATA_DIR_LOCK.lock().expect("data dir lock");
    let (_directory, mut app) = test_app();
    let mut delay = ActionInstance::new("delay");
    delay.set("seconds", json!("1"));
    app.profile.slot_mut(0).expect("slot").action = Some(delay);
    app.profile.slot_mut(1).expect("slot").action = Some(brightness_action(20));

    publish_key_down(&app, 0);
    publish_key_down(&app, 1);

    let _ = app.update(Message::Poll);

    assert!(app.effects_waiting);
    assert_eq!(app.queued_press, VecDeque::from([1]));
    assert_eq!(app.profile.brightness, 80);

    let _ = app.update(Message::EffectsReady);

    assert!(!app.effects_waiting);
    assert!(app.queued_press.is_empty());
    assert_eq!(app.profile.brightness, 20);
}

#[test]
fn pressing_a_key_runs_its_action_and_renders_all_keys() {
    let _guard = DATA_DIR_LOCK.lock().expect("data dir lock");
    let (directory, mut app) = test_app();
    let marker = directory.path().join("marker.txt");
    app.refresh();
    assert_eq!(app.key_images.len(), app.caps.key_count() as usize);
    assert!(app.key_images.iter().all(Option::is_some));

    let mut action = ActionInstance::new("command");
    action.set(
        "command",
        json!(format!("echo ok>\"{}\"", marker.display())),
    );
    app.profile.slot_mut(0).expect("slot").action = Some(action);

    let _ = app.press(0);

    let deadline = Instant::now() + Duration::from_secs(5);
    while !marker.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        marker.exists(),
        "the command action should have written a file, status: {}",
        app.status
    );
}
