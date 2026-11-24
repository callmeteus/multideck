//! One WebSocket session with a Stream Deck Mobile client.

use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use image::{ImageEncoder, RgbaImage};
use multideck_core::DeviceInput;
use prost::Message;
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message as WsMessage;

use crate::Shared;
use crate::proto;

const PROTOCOL_VERSION: u32 = 3;

struct SessionState {
    opened: bool,
    authenticated: bool,
    icon_size: (u32, u32),
    formats: Vec<i32>,
}

impl SessionState {
    fn new(width: u16, height: u16) -> Self {
        Self {
            opened: false,
            authenticated: false,
            icon_size: (width as u32, height as u32),
            formats: Vec::new(),
        }
    }

    fn apply_config(&mut self, config: &proto::KeypadConfig) {
        if let Some(size) = &config.icon_size
            && size.width > 0
            && size.height > 0
        {
            self.icon_size = (size.width, size.height);
        }
        self.formats = config.supported_icon_image_formats.clone();
    }
}

/// Runs one session until the client closes or the connection drops.
pub(crate) async fn run(socket: TcpStream, shared: Arc<Shared>) {
    let mut ws = match tokio_tungstenite::accept_async(socket).await {
        Ok(ws) => ws,
        Err(error) => {
            tracing::warn!(%error, "mobile websocket handshake failed");
            return;
        }
    };

    let mut updates = shared.updates.subscribe();
    let mut state = SessionState::new(
        shared.config.caps.key_image.0,
        shared.config.caps.key_image.1,
    );

    loop {
        tokio::select! {
            message = ws.next() => {
                match message {
                    Some(Ok(WsMessage::Binary(bytes))) => {
                        match proto::ClientMessage::decode(bytes.as_ref()) {
                            Ok(client) => {
                                if !handle(&mut ws, &shared, &mut state, client).await {
                                    break;
                                }
                            }
                            Err(error) => {
                                tracing::warn!(%error, "mobile client sent invalid protobuf");
                            }
                        }
                    }
                    Some(Ok(WsMessage::Ping(payload))) => {
                        let _ = ws.send(WsMessage::Pong(payload)).await;
                    }
                    Some(Ok(WsMessage::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(error)) => {
                        tracing::debug!(%error, "mobile websocket error");
                        break;
                    }
                }
            }
            update = updates.recv() => {
                if let Ok((key, image)) = update
                    && state.opened
                {
                    send_icon(&mut ws, &state, key, &image).await;
                }
            }
        }
    }

    tracing::info!("mobile session closed");
}

async fn send(ws: &mut WebSocketStream<TcpStream>, message: &proto::ServerMessage) {
    if let Err(error) = ws
        .send(WsMessage::Binary(message.encode_to_vec().into()))
        .await
    {
        tracing::debug!(%error, "could not send a mobile reply");
    }
}

async fn handle(
    ws: &mut WebSocketStream<TcpStream>,
    shared: &Arc<Shared>,
    state: &mut SessionState,
    client: proto::ClientMessage,
) -> bool {
    let Some(payload) = client.payload else {
        return true;
    };

    match payload {
        proto::client_message::Payload::Hello(hello) => {
            let code = pairing_code(&hello.public_key);
            let name = hello
                .client_information
                .as_ref()
                .map(|info| info.name.clone())
                .unwrap_or_default();
            tracing::info!(%code, %name, "mobile client introduced itself");
            state.authenticated = shared.config.auto_trust;

            let port = shared.port.borrow().unwrap_or(0);
            let reply = proto::ServerMessage {
                payload: Some(proto::server_message::Payload::Hello(
                    proto::HelloFromServer {
                        workstation_id: shared.config.workstation_id.clone(),
                        hostname: shared.config.hostname.clone(),
                        version: env!("CARGO_PKG_VERSION").to_string(),
                        protocol_version: PROTOCOL_VERSION,
                        public_key: shared.public_key.to_vec(),
                        client_needs_authentication: !state.authenticated,
                        os_name: Some(std::env::consts::OS.to_string()),
                        os_version: None,
                        endpoints: Some(proto::ServiceEndpoints {
                            addresses: Vec::new(),
                            port: port as u32,
                        }),
                    },
                )),
            };
            send(ws, &reply).await;
        }
        proto::client_message::Payload::Authenticate(request) => {
            if state.authenticated {
                let reply = proto::ServerMessage {
                    payload: Some(proto::server_message::Payload::Ok(proto::Ok {
                        request_id: request.request_id,
                    })),
                };
                send(ws, &reply).await;
            } else {
                let reply = proto::ServerMessage {
                    payload: Some(proto::server_message::Payload::Error(proto::Error {
                        request_id: request.request_id,
                        code: proto::error::Code::AuthenticationRejected as i32,
                        reason: Some("pairing confirmation is not enabled".into()),
                    })),
                };
                send(ws, &reply).await;
            }
        }
        proto::client_message::Payload::CancelAuthenticate(request) => {
            let reply = proto::ServerMessage {
                payload: Some(proto::server_message::Payload::Ok(proto::Ok {
                    request_id: request.request_id,
                })),
            };
            send(ws, &reply).await;
        }
        proto::client_message::Payload::CreateVirtualDevice(create) => {
            if let Some(config) = &create.keypad_config {
                state.apply_config(config);
            }
            state.opened = true;
            let name = if create.name.is_empty() {
                shared.config.device_name.clone()
            } else {
                create.name.clone()
            };
            reply_opened(ws, create.request_id, &name).await;
            send_snapshot(ws, shared, state).await;
        }
        proto::client_message::Payload::OpenVirtualDevice(open) => {
            if let Some(config) = &open.keypad_config {
                state.apply_config(config);
            }
            state.opened = true;
            let device_id = if open.device_id.is_empty() {
                "multideck".to_string()
            } else {
                open.device_id.clone()
            };
            reply_opened(ws, open.request_id, &device_id).await;
            send_snapshot(ws, shared, state).await;
        }
        proto::client_message::Payload::UpdateKeypadConfig(update) => {
            if let Some(size) = &update.icon_size
                && size.width > 0
                && size.height > 0
            {
                state.icon_size = (size.width, size.height);
            }
        }
        proto::client_message::Payload::KeyPress(press) => {
            let event = if press.pressed {
                DeviceInput::KeyDown {
                    key: press.index as u8,
                }
            } else {
                DeviceInput::KeyUp {
                    key: press.index as u8,
                }
            };
            if let Some(inputs) = shared.inputs.lock().await.as_ref() {
                let _ = inputs.send(event);
            }
        }
        proto::client_message::Payload::NextPage(_)
        | proto::client_message::Payload::PreviousPage(_)
        | proto::client_message::Payload::PageByIndex(_) => {
            tracing::debug!("mobile client asked for a page change; handled by the host page bar");
        }
        proto::client_message::Payload::DeleteVirtualDevice(_) => {
            tracing::info!("mobile client deleted the virtual device");
            return false;
        }
        proto::client_message::Payload::ClientCapabilities(capabilities) => {
            tracing::debug!(pro = capabilities.is_user_pro, "mobile client capabilities");
        }
        proto::client_message::Payload::AssignShortcut(shortcut) => {
            tracing::debug!(index = shortcut.index, "mobile client assigned a shortcut");
        }
        proto::client_message::Payload::RemoveShortcut(_)
        | proto::client_message::Payload::TriggerShortcut(_) => {
            tracing::debug!("shortcut message ignored by the current host");
        }
    }
    true
}

async fn reply_opened(ws: &mut WebSocketStream<TcpStream>, request_id: u32, device_id: &str) {
    let reply = proto::ServerMessage {
        payload: Some(proto::server_message::Payload::VirtualDeviceOpened(
            proto::VirtualDeviceOpened {
                request_id,
                device: Some(proto::VirtualDevice {
                    id: device_id.to_string(),
                    name: device_id.to_string(),
                }),
            },
        )),
    };
    send(ws, &reply).await;
}

async fn send_snapshot(ws: &mut WebSocketStream<TcpStream>, shared: &Shared, state: &SessionState) {
    let latest = match shared.latest.lock() {
        Ok(latest) => latest.clone(),
        Err(_) => return,
    };
    let stored = latest.iter().filter(|image| image.is_some()).count();
    tracing::debug!(keys = latest.len(), stored, "sending the mobile snapshot");
    for (key, image) in latest.iter().enumerate() {
        if let Some(image) = image {
            send_icon(ws, state, key as u8, image).await;
        }
    }
}

async fn send_icon(
    ws: &mut WebSocketStream<TcpStream>,
    state: &SessionState,
    key: u8,
    source: &RgbaImage,
) {
    let Some(image) = encode_icon(source, state) else {
        tracing::debug!(key, "mobile icon could not be encoded");
        return;
    };
    tracing::debug!(key, bytes = image.image.len(), "sending mobile icon");
    let reply = proto::ServerMessage {
        payload: Some(proto::server_message::Payload::Icon(proto::Icon {
            index: key as u32,
            image: Some(image),
        })),
    };
    send(ws, &reply).await;
}

fn encode_icon(source: &RgbaImage, state: &SessionState) -> Option<proto::Image> {
    let (width, height) = state.icon_size;
    if width == 0 || height == 0 {
        return None;
    }
    let scaled = if source.width() == width && source.height() == height {
        source.clone()
    } else {
        image::imageops::resize(source, width, height, image::imageops::FilterType::Lanczos3)
    };

    let supported = |format: proto::ImageFormat| {
        state.formats.is_empty() || state.formats.contains(&(format as i32))
    };
    let (format, bytes, row_bytes) = if supported(proto::ImageFormat::Png) {
        (proto::ImageFormat::Png, encode_png(&scaled)?, 0)
    } else if supported(proto::ImageFormat::Jpg) {
        (proto::ImageFormat::Jpg, encode_jpeg(&scaled)?, 0)
    } else if supported(proto::ImageFormat::Bmp) {
        (proto::ImageFormat::Bmp, encode_bmp(&scaled)?, 0)
    } else if supported(proto::ImageFormat::Lz4) {
        let raw = scaled.into_raw();
        let compressed = lz4_flex::compress(&raw);
        (proto::ImageFormat::Lz4, compressed, width * 4)
    } else {
        tracing::warn!("phone offered no supported image format; falling back to PNG");
        (proto::ImageFormat::Png, encode_png(&scaled)?, 0)
    };

    Some(proto::Image {
        format: format as i32,
        r#type: proto::ImageType::Opaque as i32,
        size: Some(proto::Size { width, height }),
        row_bytes,
        image: bytes,
    })
}

fn encode_png(image: &RgbaImage) -> Option<Vec<u8>> {
    let mut buffer = Vec::new();
    image::codecs::png::PngEncoder::new(&mut buffer)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgba8,
        )
        .ok()?;
    Some(buffer)
}

fn encode_jpeg(image: &RgbaImage) -> Option<Vec<u8>> {
    let mut buffer = Vec::new();
    let rgb = image::DynamicImage::ImageRgba8(image.clone()).to_rgb8();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buffer, 92)
        .encode(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .ok()?;
    Some(buffer)
}

fn encode_bmp(image: &RgbaImage) -> Option<Vec<u8>> {
    let mut buffer = Vec::new();
    image::codecs::bmp::BmpEncoder::new(&mut buffer)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgba8,
        )
        .ok()?;
    Some(buffer)
}

fn pairing_code(public_key: &[u8]) -> String {
    use blake2::{Blake2b512, Digest};

    let mut hasher = Blake2b512::new();
    hasher.update(public_key);
    let digest = hasher.finalize();
    digest
        .iter()
        .take(3)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
