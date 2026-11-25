//! One WebSocket session with a Stream Deck Mobile client.

use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::Ordering;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use image::RgbaImage;
use multideck_core::DeviceInput;
use prost::Message;
use tokio::net::TcpStream;
use tokio::sync::Notify;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message as WsMessage;

use crate::PairingState;
use crate::Shared;
use crate::proto;

mod icon;

use icon::encode_icon;

const PROTOCOL_VERSION: u32 = 3;
const PAIRING_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionPhase {
    AwaitingHello,
    AwaitingAuthentication,
    Authenticated,
}

struct SessionState {
    phase: SessionPhase,
    trusted: bool,
    opened: bool,
    counted: bool,
    icon_size: (u32, u32),
    formats: Vec<i32>,
    cipher: Option<crypto_box::SalsaBox>,
    client_key: Vec<u8>,
    pairing_id: Option<u64>,
    pairing_decision: Option<Arc<StdMutex<Option<bool>>>>,
    pairing_notify: Option<Arc<Notify>>,
    pending_authenticate: Option<u32>,
}

impl SessionState {
    fn new(width: u16, height: u16) -> Self {
        Self {
            phase: SessionPhase::AwaitingHello,
            trusted: false,
            opened: false,
            counted: false,
            icon_size: (width as u32, height as u32),
            formats: Vec::new(),
            cipher: None,
            client_key: Vec::new(),
            pairing_id: None,
            pairing_decision: None,
            pairing_notify: None,
            pending_authenticate: None,
        }
    }

    fn apply_config(&mut self, config: &proto::KeypadConfig) {
        if let Some(size) = &config.icon_size
            && size.width > 0
            && size.height > 0
        {
            self.icon_size = (size.width, size.height);
            tracing::debug!(width = size.width, height = size.height, "phone icon size");
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
    let mut keepalive = tokio::time::interval_at(
        tokio::time::Instant::now() + std::time::Duration::from_secs(5),
        std::time::Duration::from_secs(5),
    );
    keepalive.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut state = SessionState::new(
        shared.config.caps.key_image.0,
        shared.config.caps.key_image.1,
    );

    loop {
        tokio::select! {
            message = ws.next() => {
                match message {
                    Some(Ok(WsMessage::Binary(bytes))) => {
                        let payload = match &state.cipher {
                            Some(cipher) => match decrypt_frame(cipher, &bytes) {
                                Some(payload) => payload,
                                None => {
                                    tracing::warn!(
                                        bytes = bytes.len(),
                                        "mobile frame could not be decrypted"
                                    );
                                    continue;
                                }
                            },
                            None => bytes.to_vec(),
                        };
                        if payload.len() < 200 {
                            tracing::debug!(
                                bytes = payload.len(),
                                hex = %hex_string(&payload),
                                "mobile frame received"
                            );
                        } else {
                            tracing::debug!(bytes = payload.len(), "mobile frame received");
                        }
                        match proto::ClientMessage::decode(payload.as_slice()) {
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
                        tracing::debug!(bytes = payload.len(), "mobile ping");
                        let _ = ws.send(WsMessage::Pong(payload)).await;
                    }
                    Some(Ok(WsMessage::Close(frame))) => {
                        tracing::info!(?frame, "mobile client closed the socket");
                        break;
                    }
                    None => break,
                    Some(Ok(other)) => {
                        tracing::debug!(?other, "mobile frame ignored");
                    }
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
            decision = wait_for_pairing(
                state.pending_authenticate.is_some(),
                state.pairing_notify.clone(),
                state.pairing_decision.clone(),
            ) => {
                if let Some(request_id) = state.pending_authenticate.take() {
                    finish_authentication(&mut ws, &shared, &mut state, request_id, decision).await;
                }
            }
            _ = keepalive.tick() => {
                // The phone drops the socket when it receives no data frame for
                // ten seconds ("receive timeout"). An empty Ok is ignored by its
                // RPC layer but keeps the connection alive. Before the handshake
                // the only allowed server frame is Hello, so wait for it.
                if state.cipher.is_some() {
                    let keepalive = proto::ServerMessage {
                        payload: Some(proto::server_message::Payload::Ok(proto::Ok {
                            request_id: 0,
                        })),
                    };
                    send(&mut ws, &state, &keepalive).await;
                }
            }
        }
    }

    if let Some(id) = state.pairing_id {
        crate::clear_pending_pairing(&shared, id);
    }
    if state.counted {
        shared.connected.fetch_sub(1, Ordering::Relaxed);
    }
    tracing::info!("mobile session closed");
}

async fn send(
    ws: &mut WebSocketStream<TcpStream>,
    state: &SessionState,
    message: &proto::ServerMessage,
) {
    let bytes = message.encode_to_vec();
    let bytes = match &state.cipher {
        Some(cipher) => encrypt_frame(cipher, &bytes),
        None => bytes,
    };
    tracing::debug!(bytes = bytes.len(), "mobile frame sent");
    if let Err(error) = ws.send(WsMessage::Binary(bytes.into())).await {
        tracing::debug!(%error, "could not send a mobile reply");
    }
}

async fn send_plain(ws: &mut WebSocketStream<TcpStream>, message: &proto::ServerMessage) {
    let bytes = message.encode_to_vec();
    tracing::debug!(bytes = bytes.len(), "mobile frame sent in the clear");
    if let Err(error) = ws.send(WsMessage::Binary(bytes.into())).await {
        tracing::debug!(%error, "could not send a mobile reply");
    }
}

async fn wait_for_pairing(
    enabled: bool,
    notify: Option<Arc<Notify>>,
    decision: Option<Arc<StdMutex<Option<bool>>>>,
) -> Option<bool> {
    if !enabled {
        std::future::pending::<Option<bool>>().await;
    }
    let (Some(notify), Some(decision)) = (notify, decision) else {
        return None;
    };
    if let Some(value) = decision.lock().ok().and_then(|decision| *decision) {
        return Some(value);
    }
    if tokio::time::timeout(PAIRING_TIMEOUT, notify.notified())
        .await
        .is_err()
    {
        return None;
    }
    decision.lock().ok().and_then(|decision| *decision)
}

async fn finish_authentication(
    ws: &mut WebSocketStream<TcpStream>,
    shared: &Arc<Shared>,
    state: &mut SessionState,
    request_id: u32,
    decision: Option<bool>,
) {
    if let Some(id) = state.pairing_id.take() {
        crate::clear_pending_pairing(shared, id);
    }
    state.pairing_decision = None;
    state.pairing_notify = None;

    let reply = if decision.unwrap_or(false) {
        state.phase = SessionPhase::Authenticated;
        if !state.counted {
            state.counted = true;
            shared.connected.fetch_add(1, Ordering::Relaxed);
        }
        if !state.trusted {
            crate::remember_trust(shared, &state.client_key);
        }
        tracing::info!("mobile pairing accepted");
        proto::ServerMessage {
            payload: Some(proto::server_message::Payload::Ok(proto::Ok { request_id })),
        }
    } else {
        tracing::info!("mobile pairing rejected or timed out");
        proto::ServerMessage {
            payload: Some(proto::server_message::Payload::Error(proto::Error {
                request_id,
                code: proto::error::Code::AuthenticationRejected as i32,
                reason: Some("the pairing request was not confirmed".into()),
            })),
        }
    };
    send(ws, state, &reply).await;
    if state.phase == SessionPhase::Authenticated {
        send_device_list(ws, state, shared).await;
    }
}

async fn reject_unauthorized(ws: &mut WebSocketStream<TcpStream>, state: &SessionState) {
    let reply = proto::ServerMessage {
        payload: Some(proto::server_message::Payload::Error(proto::Error {
            request_id: 0,
            code: proto::error::Code::AuthenticationRejected as i32,
            reason: Some("authenticate before using the mobile device".into()),
        })),
    };
    send(ws, state, &reply).await;
}

fn encrypt_frame(cipher: &crypto_box::SalsaBox, plain: &[u8]) -> Vec<u8> {
    use crypto_box::aead::Aead;

    let mut nonce_bytes = [0u8; 24];
    if getrandom::fill(&mut nonce_bytes).is_err() {
        return Vec::new();
    }
    let nonce = crypto_box::Nonce::from_slice(&nonce_bytes);
    let Ok(ciphertext) = cipher.encrypt(nonce, plain) else {
        return Vec::new();
    };
    let mut frame = nonce_bytes.to_vec();
    frame.extend_from_slice(&ciphertext);
    frame
}

fn decrypt_frame(cipher: &crypto_box::SalsaBox, frame: &[u8]) -> Option<Vec<u8>> {
    use crypto_box::aead::Aead;

    if frame.len() < 24 + 16 {
        return None;
    }
    let nonce = crypto_box::Nonce::from_slice(&frame[..24]);
    cipher.decrypt(nonce, &frame[24..]).ok()
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
            if let Some(id) = state.pairing_id.take() {
                crate::clear_pending_pairing(shared, id);
            }
            state.pending_authenticate = None;
            state.pairing_decision = None;
            state.pairing_notify = None;
            state.phase = SessionPhase::AwaitingHello;
            state.trusted = false;
            state.client_key = hello.public_key.clone();

            let code = pairing_code(&hello.public_key);
            let name = hello
                .client_information
                .as_ref()
                .map(|info| info.name.clone())
                .unwrap_or_default();
            tracing::info!(
                %code,
                %name,
                client_key = %hex_string(&hello.public_key),
                server_key = %hex_string(&shared.public_key),
                "mobile client introduced itself"
            );

            if hello.public_key.len() == 32 {
                state.trusted =
                    shared.config.auto_trust || crate::is_trusted(shared, &hello.public_key);
                state.phase = if state.trusted {
                    SessionPhase::Authenticated
                } else {
                    SessionPhase::AwaitingAuthentication
                };
                if state.trusted {
                    mark_connected(shared, state);
                }
                if !state.trusted {
                    let pairing = PairingState::new(code, hello.public_key.clone());
                    state.pairing_id = Some(pairing.id);
                    state.pairing_decision = Some(Arc::clone(&pairing.decision));
                    state.pairing_notify = Some(Arc::clone(&pairing.notify));
                    if let Ok(mut pending) = shared.pairing.lock() {
                        *pending = Some(pairing);
                    }
                }

                let mut client_public = [0u8; 32];
                client_public.copy_from_slice(&hello.public_key);
                let secret = crypto_box::SecretKey::from(shared.secret_key);
                state.cipher = Some(crypto_box::SalsaBox::new(
                    &crypto_box::PublicKey::from(client_public),
                    &secret,
                ));
            } else {
                tracing::warn!(
                    bytes = hello.public_key.len(),
                    "mobile client sent an invalid key"
                );
            }

            let port = shared.port.borrow().unwrap_or(0);
            let reply = proto::ServerMessage {
                payload: Some(proto::server_message::Payload::Hello(
                    proto::HelloFromServer {
                        workstation_id: shared.config.workstation_id.clone(),
                        hostname: shared.config.hostname.clone(),
                        version: env!("CARGO_PKG_VERSION").to_string(),
                        protocol_version: PROTOCOL_VERSION,
                        public_key: shared.public_key.to_vec(),
                        client_needs_authentication: !state.trusted,
                        os_name: Some(std::env::consts::OS.to_string()),
                        os_version: None,
                        endpoints: Some(proto::ServiceEndpoints {
                            addresses: Vec::new(),
                            port: port as u32,
                        }),
                    },
                )),
            };
            send_plain(ws, &reply).await;
            send_device_list(ws, state, shared).await;
        }
        proto::client_message::Payload::Authenticate(request) => {
            tracing::info!(
                request_id = request.request_id,
                phase = ?state.phase,
                trusted = state.trusted,
                "mobile client asked to authenticate"
            );
            if state.phase == SessionPhase::Authenticated {
                let reply = proto::ServerMessage {
                    payload: Some(proto::server_message::Payload::Ok(proto::Ok {
                        request_id: request.request_id,
                    })),
                };
                send(ws, state, &reply).await;
                send_device_list(ws, state, shared).await;
            } else if state.phase == SessionPhase::AwaitingAuthentication && state.trusted {
                finish_authentication(ws, shared, state, request.request_id, Some(true)).await;
            } else if state.phase == SessionPhase::AwaitingAuthentication
                && state.pairing_decision.is_some()
            {
                state.pending_authenticate = Some(request.request_id);
            } else {
                let reply = proto::ServerMessage {
                    payload: Some(proto::server_message::Payload::Error(proto::Error {
                        request_id: request.request_id,
                        code: proto::error::Code::AuthenticationRejected as i32,
                        reason: Some("say hello before authenticating".into()),
                    })),
                };
                send(ws, state, &reply).await;
            }
        }
        proto::client_message::Payload::CancelAuthenticate(request) => {
            state.pending_authenticate = None;
            if let Some(id) = state.pairing_id.take() {
                crate::clear_pending_pairing(shared, id);
            }
            state.pairing_decision = None;
            state.pairing_notify = None;
            let reply = proto::ServerMessage {
                payload: Some(proto::server_message::Payload::Ok(proto::Ok {
                    request_id: request.request_id,
                })),
            };
            send(ws, state, &reply).await;
        }
        proto::client_message::Payload::CreateVirtualDevice(_)
        | proto::client_message::Payload::OpenVirtualDevice(_)
        | proto::client_message::Payload::UpdateKeypadConfig(_)
        | proto::client_message::Payload::KeyPress(_)
        | proto::client_message::Payload::DeleteVirtualDevice(_)
            if state.phase != SessionPhase::Authenticated =>
        {
            tracing::warn!(
                "phone opened the device without authenticating; remove and re-add the computer on the tablet, or add its key to mobile_trusted.txt"
            );
            reject_unauthorized(ws, state).await;
        }
        proto::client_message::Payload::CreateVirtualDevice(create) => {
            if let Some(config) = &create.keypad_config {
                state.apply_config(config);
            }
            state.opened = true;
            mark_connected(shared, state);
            let name = if create.name.is_empty() {
                shared.config.device_name.clone()
            } else {
                create.name.clone()
            };
            reply_opened(ws, state, create.request_id, &name).await;
            send_snapshot(ws, shared, state).await;
        }
        proto::client_message::Payload::OpenVirtualDevice(open) => {
            if let Some(config) = &open.keypad_config {
                state.apply_config(config);
            }
            state.opened = true;
            mark_connected(shared, state);
            let device_id = if open.device_id.is_empty() {
                "multideck".to_string()
            } else {
                open.device_id.clone()
            };
            reply_opened(ws, state, open.request_id, &device_id).await;
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
            if let Ok(mut pro) = shared.pro.lock() {
                *pro = Some(capabilities.is_user_pro);
            }
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

fn mark_connected(shared: &Shared, state: &mut SessionState) {
    if !state.counted {
        state.counted = true;
        shared.connected.fetch_add(1, Ordering::Relaxed);
    }
}
async fn send_device_list(
    ws: &mut WebSocketStream<TcpStream>,
    state: &SessionState,
    shared: &Shared,
) {
    let reply = proto::ServerMessage {
        payload: Some(proto::server_message::Payload::VirtualDeviceList(
            proto::VirtualDeviceList {
                devices: vec![proto::VirtualDevice {
                    id: "multideck".to_string(),
                    name: shared.config.device_name.clone(),
                }],
            },
        )),
    };
    send(ws, state, &reply).await;
}
async fn reply_opened(
    ws: &mut WebSocketStream<TcpStream>,
    state: &SessionState,
    request_id: u32,
    device_id: &str,
) {
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
    send(ws, state, &reply).await;
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
    send(ws, state, &reply).await;
}

fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn pairing_code(public_key: &[u8]) -> String {
    use blake2::Blake2b;
    use blake2::digest::{Digest, consts::U32};

    // The phone shows the first three bytes of a BLAKE2b-256 fingerprint of its
    // own public key, in uppercase hex. BLAKE2b-512 produces a different value.
    let mut hasher = Blake2b::<U32>::new();
    hasher.update(public_key);
    let digest = hasher.finalize();
    digest
        .iter()
        .take(3)
        .map(|byte| format!("{byte:02X}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::pairing_code;

    #[test]
    fn pairing_code_matches_the_phone_fingerprint() {
        // Public key of a real Stream Deck Mobile client, paired on 2026-11-24.
        let key: Vec<u8> = (0..64)
            .step_by(2)
            .map(|index| u8::from_str_radix(&KEY[index..index + 2], 16).unwrap())
            .collect();
        assert_eq!(pairing_code(&key), "ADCF47");
    }

    #[test]
    fn decrypts_a_libsodium_frame() {
        // Vector produced with PyNaCl (libsodium) using the same scheme as the app:
        // frame = nonce (24) || MAC (16) || ciphertext.
        let server_secret =
            hex_bytes("d026795a4e7f92822be3a620dd18f4fc007caea0d501b16e53c2c48e44583535");
        let client_public =
            hex_bytes("33d73c811f560f0d738216d826e7b21edfa9b13caa7a461251e11b9a6d345237");
        let frame = hex_bytes(
            "000102030405060708090a0b0c0d0e0f10111213141516171402739fc430d098164ba5cd21f053b18d49cf04",
        );

        let secret = crypto_box::SecretKey::from(<[u8; 32]>::try_from(server_secret).unwrap());
        let mut public = [0u8; 32];
        public.copy_from_slice(&client_public);
        let cipher = crypto_box::SalsaBox::new(&crypto_box::PublicKey::from(public), &secret);

        let plain = super::decrypt_frame(&cipher, &frame).expect("libsodium frame decrypts");
        assert_eq!(plain, vec![0x12, 0x02, 0x08, 0x00]);
    }

    #[test]
    fn encrypts_frames_that_decrypt_with_the_same_key() {
        let server_secret = crypto_box::SecretKey::from([3u8; 32]);
        let client_secret = crypto_box::SecretKey::from([9u8; 32]);
        let client = crypto_box::SalsaBox::new(&server_secret.public_key(), &client_secret);
        let server = crypto_box::SalsaBox::new(&client_secret.public_key(), &server_secret);

        let frame = super::encrypt_frame(&server, b"hello");
        assert_eq!(super::decrypt_frame(&client, &frame).unwrap(), b"hello");
    }

    fn hex_bytes(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&text[index..index + 2], 16).unwrap())
            .collect()
    }

    const KEY: &str = "bc8759bb95b8e35ef1096bc2d50b8638e36fbff617cd5d54e987a81d3f502e1a";
}
