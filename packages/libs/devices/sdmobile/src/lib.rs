//! Stream Deck Mobile adapter.
//!
//! The phone discovers this process over mDNS (`_vstreamdeck2._tcp`), opens a
//! WebSocket and exchanges protobuf messages in the `vsd2` package. This crate
//! implements the server side so a phone can drive a MultiDeck profile.
//!
//! Pairing: the client sends `HelloFromClient` with its public key. The first
//! six hex digits of a BLAKE2b digest of that key are the verification code that
//! both sides would show. This build trusts clients automatically (configurable)
//! while the confirmation UI is not written yet; the code is logged so it can be
//! compared with the phone.

pub mod proto {
    #![allow(clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/vsd2.rs"));
}

mod session;

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use async_trait::async_trait;
use image::RgbaImage;
use multideck_core::{DeviceCaps, DeviceCommand, DeviceDescriptor, KeyImageFormat, Transport};
use multideck_device_api::{CommandReceiver, DeviceAdapter, DeviceError, InputSender};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, broadcast, watch};
use tokio::task::JoinSet;

/// Configuration for one mobile deck server.
#[derive(Debug, Clone)]
pub struct MobileConfig {
    /// Workstation identity shown to the phone and advertised over mDNS.
    pub workstation_id: String,
    /// Host name announced in `HelloFromServer`.
    pub hostname: String,
    /// Device name used when the phone does not ask for a specific one.
    pub device_name: String,
    /// Grid and image capabilities of the virtual device.
    pub caps: DeviceCaps,
    /// Trust clients without a confirmation dialog. Logged either way.
    pub auto_trust: bool,
}

impl MobileConfig {
    /// Defaults for a 5x3 phone layout.
    pub fn new(workstation_id: impl Into<String>, hostname: impl Into<String>) -> Self {
        Self {
            workstation_id: workstation_id.into(),
            hostname: hostname.into(),
            device_name: "MultiDeck Mobile".to_string(),
            caps: DeviceCaps {
                cols: 5,
                rows: 3,
                key_image: (96, 96),
                encoders: 0,
                touch_strip: false,
                format: KeyImageFormat::Png,
            },
            auto_trust: false,
        }
    }
}

/// Generates a 20 character workstation identity in the base62 alphabet.
pub fn generate_workstation_id() -> String {
    const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    let mut bytes = [0u8; 20];
    if getrandom::fill(&mut bytes).is_err() {
        return "MULTIDECK000000000000".to_string();
    }
    bytes
        .iter()
        .map(|byte| ALPHABET[(*byte as usize) % ALPHABET.len()] as char)
        .collect()
}
pub(crate) struct PairingState {
    pub code: String,
    pub decision: Option<bool>,
}

pub(crate) struct Shared {
    pub config: MobileConfig,
    pub public_key: [u8; 32],
    pub latest: StdMutex<Vec<Option<Arc<RgbaImage>>>>,
    pub updates: broadcast::Sender<(u8, Arc<RgbaImage>)>,
    pub inputs: Mutex<Option<InputSender>>,
    pub port: watch::Sender<Option<u16>>,
    pub brightness: AtomicU8,
    pub pairing: StdMutex<Option<PairingState>>,
}

/// Host facing handle of the mobile deck.
#[derive(Clone)]
pub struct MobileDeck {
    shared: Arc<Shared>,
    descriptor: DeviceDescriptor,
    ports: watch::Receiver<Option<u16>>,
}

/// The adapter task that owns the listener.
pub struct MobileAdapter {
    shared: Arc<Shared>,
    descriptor: DeviceDescriptor,
}

impl MobileDeck {
    /// Starts a deck handle and its adapter. The socket is bound when the
    /// adapter runs, so the port becomes available through [`Self::port`].
    pub fn new(config: MobileConfig) -> (Self, MobileAdapter) {
        let mut public_key = [0u8; 32];
        if getrandom::fill(&mut public_key).is_err() {
            tracing::warn!("no system randomness available for the mobile keypair");
        }

        let keys = config.caps.key_count() as usize;
        let (updates, _) = broadcast::channel(64);
        let (port, ports) = watch::channel(None);
        let shared = Arc::new(Shared {
            config,
            public_key,
            latest: StdMutex::new(vec![None; keys]),
            updates,
            inputs: Mutex::new(None),
            port,
            brightness: AtomicU8::new(80),
            pairing: StdMutex::new(None),
        });

        let descriptor = DeviceDescriptor {
            id: "sdmobile".to_string(),
            name: "Stream Deck Mobile".to_string(),
            model: "vsd2".to_string(),
            transport: Transport::Network,
            caps: shared.config.caps.clone(),
        };

        let deck = Self {
            shared: Arc::clone(&shared),
            descriptor: descriptor.clone(),
            ports,
        };
        let adapter = MobileAdapter { shared, descriptor };
        (deck, adapter)
    }

    /// Identity and capabilities exposed to the host.
    pub fn descriptor(&self) -> &DeviceDescriptor {
        &self.descriptor
    }

    /// Port the listener is bound to, once the adapter has started.
    pub fn port(&self) -> Option<u16> {
        *self.ports.borrow()
    }

    /// Receiver that reports the port as soon as it is known.
    pub fn port_receiver(&self) -> watch::Receiver<Option<u16>> {
        self.ports.clone()
    }

    /// Code of the client waiting for a trust decision, when there is one.
    pub fn pending_pairing(&self) -> Option<String> {
        self.shared
            .pairing
            .lock()
            .ok()
            .and_then(|pairing| pairing.as_ref().map(|state| state.code.clone()))
    }

    /// Accepts or rejects the pending pairing request.
    pub fn resolve_pairing(&self, accept: bool) {
        if let Ok(mut pairing) = self.shared.pairing.lock()
            && let Some(state) = pairing.as_mut()
        {
            state.decision = Some(accept);
        }
    }
    /// Publishes an image, as the host would through a device command.
    pub fn set_image(&self, key: u8, image: Arc<RgbaImage>) {
        publish_image(&self.shared, key, image);
    }
}

fn publish_image(shared: &Shared, key: u8, image: Arc<RgbaImage>) {
    if let Ok(mut latest) = shared.latest.lock() {
        if let Some(slot) = latest.get_mut(key as usize) {
            *slot = Some(Arc::clone(&image));
        } else {
            tracing::warn!(key, "image for a key outside the mobile grid");
            return;
        }
    }
    tracing::debug!(key, "mobile image stored");
    let _ = shared.updates.send((key, image));
}

fn spawn_mdns(shared: &Arc<Shared>, port: u16) {
    let instance = format!(
        "MultiDeck-{}",
        shared
            .config
            .workstation_id
            .chars()
            .take(6)
            .collect::<String>()
    );
    let host = format!("{}.local.", shared.config.hostname.trim_end_matches('.'));
    let properties = [
        ("workstationID", shared.config.workstation_id.as_str()),
        ("version", "3"),
        ("protocol", "vsd2"),
    ];
    let info = mdns_sd::ServiceInfo::new(
        "_vstreamdeck2._tcp.local.",
        &instance,
        &host,
        "",
        port,
        &properties[..],
    )
    .map(|info| info.enable_addr_auto());

    match info {
        Ok(info) => match mdns_sd::ServiceDaemon::new() {
            Ok(daemon) => {
                if let Err(error) = daemon.register(info) {
                    tracing::warn!(%error, "could not advertise the mobile deck over mDNS");
                } else {
                    tracing::info!(port, "mobile deck advertised as _vstreamdeck2._tcp");
                }
                std::mem::forget(daemon);
            }
            Err(error) => tracing::warn!(%error, "could not start the mDNS daemon"),
        },
        Err(error) => tracing::warn!(%error, "invalid mDNS service info"),
    }
}

#[async_trait]
impl DeviceAdapter for MobileAdapter {
    fn descriptor(&self) -> DeviceDescriptor {
        self.descriptor.clone()
    }

    async fn run(
        self: Box<Self>,
        mut commands: CommandReceiver,
        inputs: InputSender,
    ) -> Result<(), DeviceError> {
        *self.shared.inputs.lock().await = Some(inputs);

        let listener =
            TcpListener::bind(("0.0.0.0", 0))
                .await
                .map_err(|error| DeviceError::Failed {
                    device: "sdmobile".to_string(),
                    reason: format!("could not bind the mobile listener: {error}"),
                })?;
        let port = listener
            .local_addr()
            .map_err(|error| DeviceError::Failed {
                device: "sdmobile".to_string(),
                reason: error.to_string(),
            })?
            .port();
        let _ = self.shared.port.send(Some(port));
        tracing::info!(port, "mobile deck listening for Stream Deck Mobile");
        spawn_mdns(&self.shared, port);

        let mut sessions = JoinSet::new();
        loop {
            tokio::select! {
                command = commands.recv() => {
                    match command {
                        Some(DeviceCommand::SetKeyImage { key, image }) => publish_image(&self.shared, key, image),
                        Some(DeviceCommand::SetBrightness { percent }) => {
                            self.shared.brightness.store(percent, Ordering::Relaxed);
                        }
                        Some(DeviceCommand::Reset) => {
                            if let Ok(mut latest) = self.shared.latest.lock() {
                                latest.iter_mut().for_each(|slot| *slot = None);
                            }
                        }
                        None => break,
                    }
                }
                accepted = listener.accept() => {
                    match accepted {
                        Ok((socket, address)) => {
                            tracing::info!(%address, "phone connected");
                            sessions.spawn(session::run(socket, Arc::clone(&self.shared)));
                        }
                        Err(error) => {
                            tracing::warn!(%error, "mobile listener accept failed");
                        }
                    }
                }
            }
        }

        sessions.abort_all();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto;
    use futures_util::{SinkExt, StreamExt};
    use prost::Message;
    use tokio_tungstenite::tungstenite::Message as WsMessage;

    async fn next_server(
        ws: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    ) -> proto::ServerMessage {
        let message = tokio::time::timeout(std::time::Duration::from_secs(5), ws.next())
            .await
            .expect("server reply timed out")
            .expect("stream ended")
            .expect("websocket error");
        let WsMessage::Binary(bytes) = message else {
            panic!("expected a binary frame");
        };
        proto::ServerMessage::decode(bytes.as_ref()).expect("valid server message")
    }

    #[tokio::test]
    async fn speaks_the_vsd2_handshake_and_reports_keys() {
        let mut config = MobileConfig::new("TESTWORKSTATION00000", "test-host");
        config.auto_trust = true;
        let (deck, adapter) = MobileDeck::new(config);
        let mut ports = deck.port_receiver();
        let (command_tx, command_rx) = tokio::sync::mpsc::unbounded_channel();
        let (input_tx, mut input_rx) = tokio::sync::mpsc::unbounded_channel();

        let task = tokio::spawn(async move {
            let _ = Box::new(adapter).run(command_rx, input_tx).await;
        });

        let port = loop {
            if let Some(port) = *ports.borrow() {
                break port;
            }
            ports.changed().await.expect("port sender alive");
        };

        let url = format!("ws://127.0.0.1:{port}");
        let (mut ws, _) = tokio_tungstenite::connect_async(url)
            .await
            .expect("connect");

        let hello = proto::ClientMessage {
            payload: Some(proto::client_message::Payload::Hello(
                proto::HelloFromClient {
                    public_key: vec![7u8; 32],
                    client_information: Some(proto::ClientInformation {
                        name: "Test phone".into(),
                        user_agent: "Stream Deck Mobile/2.1.2".into(),
                        protocol_version: 3,
                        ..Default::default()
                    }),
                },
            )),
        };
        ws.send(WsMessage::Binary(hello.encode_to_vec().into()))
            .await
            .expect("send hello");

        let reply = next_server(&mut ws).await;
        let Some(proto::server_message::Payload::Hello(server)) = reply.payload else {
            panic!("expected HelloFromServer");
        };
        assert_eq!(server.workstation_id, "TESTWORKSTATION00000");
        assert_eq!(server.protocol_version, 3);
        assert!(!server.client_needs_authentication, "auto trust is enabled");

        let authenticate = proto::ClientMessage {
            payload: Some(proto::client_message::Payload::Authenticate(
                proto::Authenticate { request_id: 42 },
            )),
        };
        ws.send(WsMessage::Binary(authenticate.encode_to_vec().into()))
            .await
            .expect("send authenticate");
        let reply = next_server(&mut ws).await;
        assert!(matches!(
            reply.payload,
            Some(proto::server_message::Payload::Ok(proto::Ok {
                request_id: 42
            }))
        ));

        let open = proto::ClientMessage {
            payload: Some(proto::client_message::Payload::OpenVirtualDevice(
                proto::OpenVirtualDevice {
                    request_id: 7,
                    device_id: "phone".into(),
                    keypad_config: Some(proto::KeypadConfig {
                        supported_icon_image_formats: vec![proto::ImageFormat::Png as i32],
                        icon_size: Some(proto::Size {
                            width: 64,
                            height: 64,
                        }),
                        high_dpi: false,
                        overscale: true,
                        layout_crop: None,
                        enable_context_api: false,
                        pressed_icon_size: None,
                    }),
                },
            )),
        };
        ws.send(WsMessage::Binary(open.encode_to_vec().into()))
            .await
            .expect("send open");
        let reply = next_server(&mut ws).await;
        assert!(matches!(
            reply.payload,
            Some(proto::server_message::Payload::VirtualDeviceOpened(
                proto::VirtualDeviceOpened { request_id: 7, .. }
            ))
        ));

        let press = proto::ClientMessage {
            payload: Some(proto::client_message::Payload::KeyPress(proto::KeyPress {
                index: 2,
                pressed: true,
            })),
        };
        ws.send(WsMessage::Binary(press.encode_to_vec().into()))
            .await
            .expect("send press");
        let event = tokio::time::timeout(std::time::Duration::from_secs(2), input_rx.recv())
            .await
            .expect("input timed out")
            .expect("input channel");
        assert_eq!(event, multideck_core::DeviceInput::KeyDown { key: 2 });

        command_tx
            .send(DeviceCommand::SetKeyImage {
                key: 2,
                image: Arc::new(RgbaImage::from_pixel(
                    32,
                    32,
                    image::Rgba([10, 20, 30, 255]),
                )),
            })
            .expect("send image");
        let reply = next_server(&mut ws).await;
        let Some(proto::server_message::Payload::Icon(icon)) = reply.payload else {
            panic!("expected an icon");
        };
        assert_eq!(icon.index, 2);
        let image = icon.image.expect("image payload");
        assert_eq!(image.format, proto::ImageFormat::Png as i32);
        assert_eq!(image.size.unwrap().width, 64);

        drop(command_tx);
        task.abort();
    }

    #[tokio::test]
    async fn pairing_waits_for_the_host_decision() {
        let mut config = MobileConfig::new("PAIRINGTEST000000000", "pair-host");
        config.auto_trust = false;
        let (deck, adapter) = MobileDeck::new(config);
        let mut ports = deck.port_receiver();
        let (command_tx, command_rx) = tokio::sync::mpsc::unbounded_channel();
        let (input_tx, _input_rx) = tokio::sync::mpsc::unbounded_channel();

        let task = tokio::spawn(async move {
            let _ = Box::new(adapter).run(command_rx, input_tx).await;
        });

        let port = loop {
            if let Some(port) = *ports.borrow() {
                break port;
            }
            ports.changed().await.expect("port sender alive");
        };
        let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
            .await
            .expect("connect");

        let hello = proto::ClientMessage {
            payload: Some(proto::client_message::Payload::Hello(
                proto::HelloFromClient {
                    public_key: vec![9u8; 32],
                    client_information: Some(proto::ClientInformation {
                        name: "Pairing phone".into(),
                        user_agent: "Stream Deck Mobile/2.1.2".into(),
                        protocol_version: 3,
                        ..Default::default()
                    }),
                },
            )),
        };
        ws.send(WsMessage::Binary(hello.encode_to_vec().into()))
            .await
            .expect("send hello");
        let reply = next_server(&mut ws).await;
        let Some(proto::server_message::Payload::Hello(server)) = reply.payload else {
            panic!("expected HelloFromServer");
        };
        assert!(server.client_needs_authentication);
        let code = deck.pending_pairing().expect("pending pairing code");
        assert_eq!(code.len(), 6);

        let authenticate = proto::ClientMessage {
            payload: Some(proto::client_message::Payload::Authenticate(
                proto::Authenticate { request_id: 5 },
            )),
        };
        ws.send(WsMessage::Binary(authenticate.encode_to_vec().into()))
            .await
            .expect("send authenticate");

        let early = tokio::time::timeout(std::time::Duration::from_millis(400), ws.next()).await;
        assert!(early.is_err(), "server must wait for the decision");

        deck.resolve_pairing(true);
        let reply = next_server(&mut ws).await;
        assert!(matches!(
            reply.payload,
            Some(proto::server_message::Payload::Ok(proto::Ok {
                request_id: 5
            }))
        ));

        drop(command_tx);
        task.abort();
    }
}
