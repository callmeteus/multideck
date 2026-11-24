//! A deck that lives inside the desktop process.
//!
//! It is a real adapter: the host sends [`DeviceCommand`]s and receives
//! [`DeviceInput`]s exactly like it would with hardware. That makes it the
//! default device for development, integration tests and screenshots.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use image::RgbaImage;
use multideck_core::{
    DeviceCaps, DeviceCommand, DeviceDescriptor, DeviceInput, KeyImageFormat, Transport,
};
use multideck_device_api::{CommandReceiver, DeviceAdapter, DeviceError, InputSender};
use tokio::sync::mpsc::{self, UnboundedReceiver};

/// State shared between the host facing handle and the adapter task.
#[derive(Debug, Default)]
pub struct VirtualState {
    images: Mutex<HashMap<u8, Arc<RgbaImage>>>,
    brightness: AtomicU8,
}

impl VirtualState {
    /// Last image received for a key.
    pub fn image(&self, key: u8) -> Option<Arc<RgbaImage>> {
        self.images
            .lock()
            .expect("virtual images lock is not poisoned")
            .get(&key)
            .cloned()
    }

    /// Brightness last requested by the host.
    pub fn brightness(&self) -> u8 {
        self.brightness.load(Ordering::Relaxed)
    }

    fn set_image(&self, key: u8, image: Arc<RgbaImage>) {
        self.images
            .lock()
            .expect("virtual images lock is not poisoned")
            .insert(key, image);
    }
}

/// Host facing handle of the virtual deck.
#[derive(Debug, Clone)]
pub struct VirtualDeck {
    state: Arc<VirtualState>,
    inputs: InputSender,
    descriptor: DeviceDescriptor,
}

/// The adapter task for the virtual deck.
pub struct VirtualAdapter {
    state: Arc<VirtualState>,
    inputs: UnboundedReceiver<DeviceInput>,
    descriptor: DeviceDescriptor,
}

impl VirtualDeck {
    /// Builds the handle and the adapter that must be spawned by the host.
    pub fn new() -> (Self, VirtualAdapter) {
        let caps = DeviceCaps {
            cols: 5,
            rows: 3,
            key_image: (96, 96),
            encoders: 0,
            touch_strip: false,
            format: KeyImageFormat::Raw,
        };
        let descriptor = DeviceDescriptor {
            id: "virtual".to_string(),
            name: "Virtual Deck".to_string(),
            model: "multideck-virtual".to_string(),
            transport: Transport::Virtual,
            caps,
        };

        let (input_tx, input_rx) = mpsc::unbounded_channel();
        let state = Arc::new(VirtualState::default());

        let deck = Self {
            state: Arc::clone(&state),
            inputs: input_tx,
            descriptor: descriptor.clone(),
        };
        let adapter = VirtualAdapter {
            state,
            inputs: input_rx,
            descriptor,
        };
        (deck, adapter)
    }

    /// Identity and capabilities of the deck.
    pub fn descriptor(&self) -> &DeviceDescriptor {
        &self.descriptor
    }

    /// Simulates a full press: key down followed by key up.
    pub fn press(&self, key: u8) {
        let _ = self.inputs.send(DeviceInput::KeyDown { key });
        let _ = self.inputs.send(DeviceInput::KeyUp { key });
    }

    /// Last image the deck received for a key.
    pub fn image(&self, key: u8) -> Option<Arc<RgbaImage>> {
        self.state.image(key)
    }

    /// Last brightness the deck received.
    pub fn brightness(&self) -> u8 {
        self.state.brightness()
    }

    /// Shared state, useful in tests and previews.
    pub fn state(&self) -> Arc<VirtualState> {
        Arc::clone(&self.state)
    }
}

#[async_trait]
impl DeviceAdapter for VirtualAdapter {
    fn descriptor(&self) -> DeviceDescriptor {
        self.descriptor.clone()
    }

    async fn run(
        mut self: Box<Self>,
        mut commands: CommandReceiver,
        inputs: InputSender,
    ) -> Result<(), DeviceError> {
        loop {
            tokio::select! {
                command = commands.recv() => {
                    match command {
                        Some(DeviceCommand::SetKeyImage { key, image }) => self.state.set_image(key, image),
                        Some(DeviceCommand::SetBrightness { percent }) => {
                            self.state.brightness.store(percent, Ordering::Relaxed);
                        }
                        Some(DeviceCommand::Reset) => self.state.images.lock().expect("lock").clear(),
                        None => break,
                    }
                }
                input = self.inputs.recv() => {
                    match input {
                        Some(input) => {
                            if inputs.send(input).is_err() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image() -> Arc<RgbaImage> {
        Arc::new(RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255])))
    }

    #[tokio::test]
    async fn adapter_stores_images_and_reports_input() {
        let (deck, adapter) = VirtualDeck::new();
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (input_tx, mut input_rx) = mpsc::unbounded_channel();
        let task = tokio::spawn(async move {
            let _ = Box::new(adapter).run(command_rx, input_tx).await;
        });

        command_tx
            .send(DeviceCommand::SetKeyImage {
                key: 2,
                image: image(),
            })
            .expect("send");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while deck.image(2).is_none() && std::time::Instant::now() < deadline {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert!(deck.image(2).is_some());

        deck.press(7);
        let first = input_rx.recv().await.expect("input");
        assert_eq!(first, DeviceInput::KeyDown { key: 7 });

        drop(command_tx);
        let _ = task.await;
    }
}
