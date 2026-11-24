//! Domain model for MultiDeck.
//!
//! This crate has no I/O and no UI. It describes what a device looks like
//! (capabilities), what the user configured (profiles, pages, key slots),
//! what a device sends back (inputs) and what the renderer needs to draw a
//! key (the view model).

pub mod action;
pub mod device;
pub mod error;
pub mod profile;
pub mod view;

pub use action::{ActionDef, ActionInstance, FieldKind, SettingField};
pub use device::{
    DeviceCaps, DeviceCommand, DeviceDescriptor, DeviceInput, KeyImageFormat, Transport,
};
pub use error::CoreError;
pub use profile::{KeySlot, Page, Profile};
pub use view::{KeyView, Rgb};
