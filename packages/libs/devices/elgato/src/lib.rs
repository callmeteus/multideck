//! Elgato Stream Deck support over USB HID.
//!
//! The adapter is a thin translation layer on top of `elgato-streamdeck`: it
//! reports the grid and image size of the connected model, writes the rendered
//! key images and forwards button, encoder and touch input to the host.
//!
//! Keys are addressed by the logical index the library reports, which matches
//! the profile grid for the models that use the same layout.

use std::time::Duration;

use async_trait::async_trait;
use elgato_streamdeck::info::Kind;
use elgato_streamdeck::{StreamDeck, StreamDeckInput, new_hidapi};
use image::DynamicImage;
use multideck_core::{
    DeviceCaps, DeviceCommand, DeviceDescriptor, DeviceInput, KeyImageFormat, Transport,
};
use multideck_device_api::{CommandReceiver, DeviceAdapter, DeviceError, InputSender};
use tokio::sync::mpsc::error::TryRecvError;

/// A Stream Deck found on the USB bus.
#[derive(Debug, Clone)]
pub struct ElgatoDevice {
    /// Model reported by the vendor and product id.
    pub kind: Kind,
    /// Serial number used to reconnect to the same unit.
    pub serial: String,
    /// Identity and capabilities exposed to the host.
    pub descriptor: DeviceDescriptor,
}

impl ElgatoDevice {
    /// Turns the discovery result into a runnable adapter.
    pub fn into_adapter(self) -> ElgatoAdapter {
        ElgatoAdapter { device: self }
    }
}

/// USB adapter for a single Elgato deck.
pub struct ElgatoAdapter {
    device: ElgatoDevice,
}

/// Lists every Elgato deck currently attached.
///
/// HID enumeration can fail on a machine without a usable backend, so the
/// result is deliberately forgiving: no device means an empty list, never a
/// panic that would keep the desktop app from starting.
pub fn discover() -> Vec<ElgatoDevice> {
    let detected = std::panic::catch_unwind(|| {
        let hidapi = new_hidapi().ok()?;
        Some(elgato_streamdeck::list_devices(&hidapi))
    })
    .ok()
    .flatten()
    .unwrap_or_default();

    detected
        .into_iter()
        .map(|(kind, serial)| ElgatoDevice {
            kind,
            descriptor: descriptor_for(kind, &serial),
            serial,
        })
        .collect()
}

/// Capabilities of a model, derived from the library tables.
pub fn caps_for(kind: Kind) -> DeviceCaps {
    let (rows, cols) = kind.key_layout();
    let (width, height) = kind.key_image_format().size;
    DeviceCaps {
        cols,
        rows,
        key_image: (width as u16, height as u16),
        encoders: kind.encoder_count(),
        touch_strip: kind.touchpoint_count() > 0 || kind.lcd_strip_size().is_some(),
        format: KeyImageFormat::Raw,
    }
}

/// Human readable model name used by the device list.
pub fn model_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Original | Kind::OriginalV2 => "Stream Deck Original",
        Kind::Xl | Kind::XlV2 => "Stream Deck XL",
        Kind::Mini | Kind::MiniMk2 | Kind::MiniDiscord => "Stream Deck Mini",
        Kind::Mk2 | Kind::Mk2Scissor => "Stream Deck MK.2",
        Kind::Neo => "Stream Deck Neo",
        Kind::Pedal => "Stream Deck Pedal",
        Kind::Plus => "Stream Deck +",
        Kind::PlusXl => "Stream Deck + XL",
        Kind::MiniMk2Module => "Stream Deck Mini Module",
        Kind::Mk2Module | Kind::Mk2ModuleScissor => "Stream Deck MK.2 Module",
        Kind::XlV2Module => "Stream Deck XL Module",
    }
}

fn descriptor_for(kind: Kind, serial: &str) -> DeviceDescriptor {
    DeviceDescriptor {
        id: format!("elgato-{}", sanitize(serial)),
        name: model_name(kind).to_string(),
        model: format!("{kind:?}"),
        transport: Transport::Usb,
        caps: caps_for(kind),
    }
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect()
}

#[async_trait]
impl DeviceAdapter for ElgatoAdapter {
    fn descriptor(&self) -> DeviceDescriptor {
        self.device.descriptor.clone()
    }

    async fn run(
        self: Box<Self>,
        commands: CommandReceiver,
        inputs: InputSender,
    ) -> Result<(), DeviceError> {
        let id = self.device.descriptor.id.clone();
        let kind = self.device.kind;
        let serial = self.device.serial.clone();

        tokio::task::spawn_blocking(move || run_blocking(id, kind, serial, commands, inputs))
            .await
            .map_err(|error| DeviceError::Failed {
                device: "elgato".to_string(),
                reason: format!("adapter task stopped: {error}"),
            })?
    }
}

fn run_blocking(
    id: String,
    kind: Kind,
    serial: String,
    mut commands: CommandReceiver,
    inputs: InputSender,
) -> Result<(), DeviceError> {
    let hidapi = new_hidapi().map_err(|error| DeviceError::Failed {
        device: id.clone(),
        reason: error.to_string(),
    })?;
    let deck =
        StreamDeck::connect(&hidapi, kind, &serial).map_err(|error| DeviceError::Failed {
            device: id.clone(),
            reason: error.to_string(),
        })?;
    tracing::info!(model = model_name(kind), %serial, "connected to an Elgato deck");

    let mut keys = vec![false; kind.key_count() as usize];
    let mut encoders = vec![false; kind.encoder_count() as usize];

    loop {
        loop {
            match commands.try_recv() {
                Ok(DeviceCommand::SetKeyImage { key, image }) => {
                    let dynamic = DynamicImage::ImageRgba8((*image).clone());
                    deck.set_button_image(key, dynamic)
                        .and_then(|()| deck.flush())
                        .map_err(|error| DeviceError::Failed {
                            device: id.clone(),
                            reason: error.to_string(),
                        })?;
                }
                Ok(DeviceCommand::SetBrightness { percent }) => {
                    deck.set_brightness(percent)
                        .map_err(|error| DeviceError::Failed {
                            device: id.clone(),
                            reason: error.to_string(),
                        })?;
                }
                Ok(DeviceCommand::Reset) => {
                    deck.reset().map_err(|error| DeviceError::Failed {
                        device: id.clone(),
                        reason: error.to_string(),
                    })?;
                    keys.fill(false);
                    encoders.fill(false);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
        }

        match deck.read_input(Some(Duration::from_millis(40))) {
            Ok(input) => forward(input, &mut keys, &mut encoders, &inputs),
            Err(error) => {
                tracing::warn!(%error, "elgato read failed, assuming the deck was unplugged");
                return Err(DeviceError::Failed {
                    device: id,
                    reason: error.to_string(),
                });
            }
        }
    }
}

fn forward(input: StreamDeckInput, keys: &mut [bool], encoders: &mut [bool], inputs: &InputSender) {
    match input {
        StreamDeckInput::ButtonStateChange(states) => {
            for (index, pressed) in states.iter().enumerate().take(keys.len()) {
                if *pressed == keys[index] {
                    continue;
                }
                keys[index] = *pressed;
                let event = if *pressed {
                    DeviceInput::KeyDown { key: index as u8 }
                } else {
                    DeviceInput::KeyUp { key: index as u8 }
                };
                let _ = inputs.send(event);
            }
        }
        StreamDeckInput::EncoderStateChange(states) => {
            for (index, pressed) in states.iter().enumerate().take(encoders.len()) {
                if *pressed && !encoders[index] {
                    let _ = inputs.send(DeviceInput::EncoderPress { index: index as u8 });
                }
                encoders[index] = *pressed;
            }
        }
        StreamDeckInput::EncoderTwist(ticks) => {
            for (index, ticks) in ticks.iter().enumerate() {
                if *ticks != 0 {
                    let _ = inputs.send(DeviceInput::EncoderRotate {
                        index: index as u8,
                        ticks: *ticks,
                    });
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[test]
    fn maps_the_common_models() {
        let mk2 = caps_for(Kind::Mk2);
        assert_eq!((mk2.cols, mk2.rows), (5, 3));
        assert_eq!(mk2.key_image, (72, 72));

        let xl = caps_for(Kind::Xl);
        assert_eq!((xl.cols, xl.rows), (8, 4));
        assert_eq!(xl.key_image, (96, 96));

        let plus = caps_for(Kind::Plus);
        assert_eq!(plus.encoders, 4);
        assert!(plus.touch_strip);
    }

    #[test]
    fn descriptor_uses_the_serial_as_id() {
        let descriptor = descriptor_for(Kind::Mk2, "ABC 123");
        assert_eq!(descriptor.id, "elgato-ABC-123");
        assert_eq!(descriptor.transport, Transport::Usb);
        assert_eq!(descriptor.name, "Stream Deck MK.2");
    }

    #[test]
    fn forwards_press_and_release_only_on_edges() {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let mut keys = vec![false; 3];
        let mut encoders = vec![false; 1];

        forward(
            StreamDeckInput::ButtonStateChange(vec![false, true, false]),
            &mut keys,
            &mut encoders,
            &sender,
        );
        forward(
            StreamDeckInput::ButtonStateChange(vec![false, true, false]),
            &mut keys,
            &mut encoders,
            &sender,
        );
        forward(
            StreamDeckInput::ButtonStateChange(vec![false, false, false]),
            &mut keys,
            &mut encoders,
            &sender,
        );

        assert_eq!(
            receiver.try_recv().expect("down"),
            DeviceInput::KeyDown { key: 1 }
        );
        assert_eq!(
            receiver.try_recv().expect("up"),
            DeviceInput::KeyUp { key: 1 }
        );
        assert!(receiver.try_recv().is_err(), "no duplicate events");
    }

    #[test]
    fn forwards_encoder_twists() {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let mut keys = vec![false; 1];
        let mut encoders = vec![false; 2];

        forward(
            StreamDeckInput::EncoderTwist(vec![2, 0]),
            &mut keys,
            &mut encoders,
            &sender,
        );

        assert_eq!(
            receiver.try_recv().expect("twist"),
            DeviceInput::EncoderRotate { index: 0, ticks: 2 }
        );
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn discovery_never_panics_without_hardware() {
        let found = discover();
        assert!(found.len() < 16, "unexpected number of detected decks");
    }
}
