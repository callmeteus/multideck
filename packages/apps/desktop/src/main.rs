#![windows_subsystem = "windows"]

//! Binary entry point.

use std::fs::OpenOptions;

use multideck_desktop::run;

fn main() -> iced::Result {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "multideck=info,multideck_device_sdmobile=debug,iced=warn".into());

    let log_path = multideck_store::Paths::discover()
        .root()
        .join("logs")
        .join("multideck.log");
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .ok();

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_ansi(false);
    match file {
        Some(file) => subscriber.with_writer(file).init(),
        None => subscriber.init(),
    }

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        log = %log_path.display(),
        "starting MultiDeck"
    );
    run()
}
