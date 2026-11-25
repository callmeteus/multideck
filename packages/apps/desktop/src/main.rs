#![windows_subsystem = "windows"]

//! Binary entry point.

use multideck_desktop::run;

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "multideck=info,iced=warn".into()),
        )
        .with_target(false)
        .init();

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting MultiDeck");
    run()
}
