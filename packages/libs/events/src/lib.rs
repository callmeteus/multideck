//! Event bus and device supervisor.
//!
//! The UI never talks to an adapter directly. It sends
//! [`multideck_core::DeviceCommand`]s through the handle returned by
//! [`connect`] and receives [`AppEvent`]s from the bus, which keeps the UI
//! and the device layer independent.

use multideck_core::{DeviceDescriptor, DeviceInput};
use multideck_device_api::{CommandSender, DeviceAdapter};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

/// Events published by the host to whoever is interested.
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// A device finished connecting.
    DeviceConnected(DeviceDescriptor),
    /// A device stopped.
    DeviceDisconnected(String),
    /// A device reported input.
    Input {
        /// Device id.
        device: String,
        /// What happened.
        input: DeviceInput,
    },
    /// Free form status line for the UI.
    Status(String),
}

/// Broadcast bus shared by the UI and the supervisor.
#[derive(Debug, Clone)]
pub struct EventBus {
    sender: broadcast::Sender<AppEvent>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    /// Creates a bus with room for the given number of pending events.
    pub fn with_capacity(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity.max(16));
        Self { sender }
    }

    /// Creates a bus with the default capacity.
    pub fn new() -> Self {
        Self::with_capacity(256)
    }

    /// Subscribes to future events.
    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent> {
        self.sender.subscribe()
    }

    /// Publishes an event, ignoring the case where nobody is listening.
    pub fn publish(&self, event: AppEvent) {
        let _ = self.sender.send(event);
    }
}

/// A device that is now supervised by the host.
pub struct ConnectedDevice {
    /// Identity the adapter reported.
    pub descriptor: DeviceDescriptor,
    /// Channel used to send commands to the device.
    pub commands: CommandSender,
    /// Task running the adapter.
    pub task: JoinHandle<()>,
}

/// Starts an adapter and wires it to the bus.
pub fn connect(
    adapter: Box<dyn DeviceAdapter>,
    bus: &EventBus,
    runtime: &tokio::runtime::Handle,
) -> ConnectedDevice {
    let descriptor = adapter.descriptor();
    let (commands, command_rx) = tokio::sync::mpsc::unbounded_channel();
    let (input_tx, mut input_rx) = tokio::sync::mpsc::unbounded_channel();

    bus.publish(AppEvent::DeviceConnected(descriptor.clone()));

    let bus_for_task = bus.clone();
    let device_id = descriptor.id.clone();
    let task = runtime.spawn(async move {
        let supervisor = {
            let bus = bus_for_task.clone();
            let device = device_id.clone();
            async move {
                while let Some(input) = input_rx.recv().await {
                    bus.publish(AppEvent::Input {
                        device: device.clone(),
                        input,
                    });
                }
            }
        };

        let runner = adapter.run(command_rx, input_tx);
        tokio::pin!(supervisor);
        tokio::pin!(runner);

        tokio::select! {
            result = &mut runner => {
                if let Err(error) = result {
                    tracing::error!(%error, device = %device_id, "device stopped with an error");
                    bus_for_task.publish(AppEvent::Status(error.to_string()));
                }
            }
            _ = &mut supervisor => {}
        }

        bus_for_task.publish(AppEvent::DeviceDisconnected(device_id));
    });

    ConnectedDevice {
        descriptor,
        commands,
        task,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use multideck_device_virtual::VirtualDeck;
    use std::time::Duration;

    #[tokio::test]
    async fn forwards_input_events_through_the_bus() {
        let bus = EventBus::new();
        let mut events = bus.subscribe();
        let (deck, adapter) = VirtualDeck::new();
        let handle = tokio::runtime::Handle::current();
        let _device = connect(Box::new(adapter), &bus, &handle);

        let connected = events.recv().await.expect("connected event");
        assert!(matches!(connected, AppEvent::DeviceConnected(_)));

        deck.press(4);

        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        let mut seen = Vec::new();
        while tokio::time::Instant::now() < deadline && seen.len() < 2 {
            if let Ok(AppEvent::Input { input, .. }) =
                tokio::time::timeout_at(deadline, events.recv())
                    .await
                    .unwrap_or(Err(broadcast::error::RecvError::Closed))
            {
                seen.push(input);
            }
        }

        assert_eq!(seen.first(), Some(&DeviceInput::KeyDown { key: 4 }));
        assert_eq!(seen.get(1), Some(&DeviceInput::KeyUp { key: 4 }));
    }
}
