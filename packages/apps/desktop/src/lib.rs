//! MultiDeck desktop application.
//!
//! The binary is intentionally thin: it initialises logging, builds [`App`]
//! and hands control to iced. Everything else lives in the workspace crates.

mod app;
mod seed;
mod view;

pub use app::App;

/// Entry point used by `main.rs` and by integration tests.
pub fn run() -> iced::Result {
    iced::application(App::boot, App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .window_size((1180.0, 760.0))
        .run()
}
