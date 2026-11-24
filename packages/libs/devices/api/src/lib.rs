//! The contract shared by every device adapter.
//!
//! An adapter knows how to draw a key and how to report input. It does not
//! know what an action is, which profile is open, or how the UI looks, which
//! is why a new kind of deck only touches this crate's implementors.

use async_trait::async_trait;
use multideck_core::{DeviceCommand, DeviceDescriptor, DeviceInput};
use thiserror::Error;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

/// Channel used by the host to send work to an adapter.
pub type CommandSender = UnboundedSender<DeviceCommand>;

/// Receiving end of the command channel.
pub type CommandReceiver = UnboundedReceiver<DeviceCommand>;

/// Channel used by an adapter to report input.
pub type InputSender = UnboundedSender<DeviceInput>;

/// Failures reported by a device adapter.
#[derive(Debug, Error)]
pub enum DeviceError {
    /// The adapter stopped because of a transport level problem.
    #[error("device '{device}' failed: {reason}")]
    Failed {
        /// Device id.
        device: String,
        /// Human readable reason.
        reason: String,
    },

    /// The adapter could not be started.
    #[error("device '{device}' could not start: {reason}")]
    Unavailable {
        /// Device id.
        device: String,
        /// Human readable reason.
        reason: String,
    },
}

/// A device the host can drive.
#[async_trait]
pub trait DeviceAdapter: Send + 'static {
    /// Identity and capabilities, available before the adapter starts.
    fn descriptor(&self) -> DeviceDescriptor;

    /// Runs the adapter until the command channel closes or the device fails.
    async fn run(
        self: Box<Self>,
        commands: CommandReceiver,
        inputs: InputSender,
    ) -> Result<(), DeviceError>;
}
