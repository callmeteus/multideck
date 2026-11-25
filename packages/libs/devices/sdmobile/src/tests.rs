use super::*;
use crate::proto;
use futures_util::{SinkExt, StreamExt};
use prost::Message;
use tokio_tungstenite::tungstenite::Message as WsMessage;

fn client_keys(seed: u8) -> (Vec<u8>, crypto_box::SecretKey) {
    let secret = crypto_box::SecretKey::from([seed; 32]);
    let public = secret.public_key().as_bytes().to_vec();
    (public, secret)
}

fn shared_box(server_public: &[u8], secret: &crypto_box::SecretKey) -> crypto_box::SalsaBox {
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(server_public);
    crypto_box::SalsaBox::new(&crypto_box::PublicKey::from(bytes), secret)
}

fn seal(shared: &crypto_box::SalsaBox, nonce_byte: u8, message: &proto::ClientMessage) -> Vec<u8> {
    use crypto_box::aead::Aead;

    let nonce_bytes = [nonce_byte; 24];
    let nonce = crypto_box::Nonce::from_slice(&nonce_bytes);
    let mut frame = nonce_bytes.to_vec();
    let sealed = shared
        .encrypt(nonce, message.encode_to_vec().as_slice())
        .expect("seal client frame");
    frame.extend_from_slice(&sealed);
    frame
}

type ClientSocket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn next_server(
    ws: &mut ClientSocket,
    shared: Option<&crypto_box::SalsaBox>,
) -> proto::ServerMessage {
    let message = tokio::time::timeout(std::time::Duration::from_secs(5), ws.next())
        .await
        .expect("server reply timed out")
        .expect("stream ended")
        .expect("websocket error");
    let WsMessage::Binary(bytes) = message else {
        panic!("expected a binary frame");
    };
    let payload = match shared {
        Some(shared) => {
            use crypto_box::aead::Aead;

            let nonce = crypto_box::Nonce::from_slice(&bytes[..24]);
            shared
                .decrypt(nonce, &bytes[24..])
                .expect("decrypt server frame")
        }
        None => bytes.to_vec(),
    };
    proto::ServerMessage::decode(payload.as_slice()).expect("valid server message")
}

async fn hello_client(port: u16, seed: u8, name: &str) -> (ClientSocket, crypto_box::SalsaBox) {
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
        .await
        .expect("connect");
    let (client_public, client_secret) = client_keys(seed);
    let hello = proto::ClientMessage {
        payload: Some(proto::client_message::Payload::Hello(
            proto::HelloFromClient {
                public_key: client_public,
                client_information: Some(proto::ClientInformation {
                    name: name.into(),
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
    let reply = next_server(&mut ws, None).await;
    let Some(proto::server_message::Payload::Hello(server)) = reply.payload else {
        panic!("expected HelloFromServer");
    };
    let shared = shared_box(&server.public_key, &client_secret);
    let list = next_server(&mut ws, Some(&shared)).await;
    assert!(matches!(
        list.payload,
        Some(proto::server_message::Payload::VirtualDeviceList(_))
    ));
    (ws, shared)
}

async fn wait_for_port(ports: &mut tokio::sync::watch::Receiver<Option<u16>>) -> u16 {
    loop {
        if let Some(port) = *ports.borrow() {
            return port;
        }
        ports.changed().await.expect("port sender alive");
    }
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

    let (client_public, client_secret) = client_keys(7);
    let hello = proto::ClientMessage {
        payload: Some(proto::client_message::Payload::Hello(
            proto::HelloFromClient {
                public_key: client_public,
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

    let reply = next_server(&mut ws, None).await;
    let Some(proto::server_message::Payload::Hello(server)) = reply.payload else {
        panic!("expected HelloFromServer");
    };
    let shared = shared_box(&server.public_key, &client_secret);
    assert_eq!(server.workstation_id, "TESTWORKSTATION00000");
    assert_eq!(server.protocol_version, 3);
    assert!(
        !server.client_needs_authentication,
        "a trusted client opens the device without authenticating"
    );
    let list = next_server(&mut ws, Some(&shared)).await;
    assert!(matches!(
        list.payload,
        Some(proto::server_message::Payload::VirtualDeviceList(_))
    ));

    let authenticate = proto::ClientMessage {
        payload: Some(proto::client_message::Payload::Authenticate(
            proto::Authenticate { request_id: 42 },
        )),
    };
    ws.send(WsMessage::Binary(seal(&shared, 1, &authenticate).into()))
        .await
        .expect("send authenticate");
    let reply = next_server(&mut ws, Some(&shared)).await;
    assert!(matches!(
        reply.payload,
        Some(proto::server_message::Payload::Ok(proto::Ok {
            request_id: 42
        }))
    ));
    let list = next_server(&mut ws, Some(&shared)).await;
    assert!(matches!(
        list.payload,
        Some(proto::server_message::Payload::VirtualDeviceList(_))
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
    ws.send(WsMessage::Binary(seal(&shared, 2, &open).into()))
        .await
        .expect("send open");
    let reply = next_server(&mut ws, Some(&shared)).await;
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
    ws.send(WsMessage::Binary(seal(&shared, 3, &press).into()))
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
    let reply = next_server(&mut ws, Some(&shared)).await;
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
async fn unauthenticated_client_cannot_use_device_commands() {
    let mut config = MobileConfig::new("UNAUTHENTICATED00000", "guard-host");
    config.auto_trust = false;
    let (deck, adapter) = MobileDeck::new(config);
    let mut ports = deck.port_receiver();
    let (command_tx, command_rx) = tokio::sync::mpsc::unbounded_channel();
    let (input_tx, mut input_rx) = tokio::sync::mpsc::unbounded_channel();
    let task = tokio::spawn(async move {
        let _ = Box::new(adapter).run(command_rx, input_tx).await;
    });
    let port = wait_for_port(&mut ports).await;
    let (mut ws, shared) = hello_client(port, 11, "Unknown phone").await;

    let open = proto::ClientMessage {
        payload: Some(proto::client_message::Payload::OpenVirtualDevice(
            proto::OpenVirtualDevice {
                request_id: 9,
                device_id: "phone".into(),
                keypad_config: None,
            },
        )),
    };
    ws.send(WsMessage::Binary(seal(&shared, 2, &open).into()))
        .await
        .expect("send open");
    let reply = next_server(&mut ws, Some(&shared)).await;
    assert!(matches!(
        reply.payload,
        Some(proto::server_message::Payload::Error(error))
            if error.code == proto::error::Code::AuthenticationRejected as i32
    ));

    let press = proto::ClientMessage {
        payload: Some(proto::client_message::Payload::KeyPress(proto::KeyPress {
            index: 2,
            pressed: true,
        })),
    };
    ws.send(WsMessage::Binary(seal(&shared, 3, &press).into()))
        .await
        .expect("send press");
    let reply = next_server(&mut ws, Some(&shared)).await;
    assert!(matches!(
        reply.payload,
        Some(proto::server_message::Payload::Error(error))
            if error.code == proto::error::Code::AuthenticationRejected as i32
    ));
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(200), input_rx.recv())
            .await
            .is_err(),
        "an unauthenticated press must not reach the host"
    );

    drop(command_tx);
    task.abort();
}

#[tokio::test]
async fn pairing_decision_is_bound_to_one_client() {
    let directory = tempfile::tempdir().expect("temp dir");
    let mut config = MobileConfig::new("PAIRINGBOUND00000000", "bound-host");
    config.auto_trust = false;
    config.trust_file = Some(directory.path().join("trusted.txt"));
    let (deck, adapter) = MobileDeck::new(config);
    let mut ports = deck.port_receiver();
    let (command_tx, command_rx) = tokio::sync::mpsc::unbounded_channel();
    let (input_tx, _input_rx) = tokio::sync::mpsc::unbounded_channel();
    let task = tokio::spawn(async move {
        let _ = Box::new(adapter).run(command_rx, input_tx).await;
    });
    let port = wait_for_port(&mut ports).await;

    let (mut first, first_box) = hello_client(port, 21, "First phone").await;
    let (mut second, second_box) = hello_client(port, 22, "Second phone").await;
    deck.pending_pairing().expect("pending pairing");
    deck.resolve_pairing(true);

    let authenticate = proto::ClientMessage {
        payload: Some(proto::client_message::Payload::Authenticate(
            proto::Authenticate { request_id: 31 },
        )),
    };
    second
        .send(WsMessage::Binary(
            seal(&second_box, 4, &authenticate).into(),
        ))
        .await
        .expect("second authenticate");
    let reply = next_server(&mut second, Some(&second_box)).await;
    assert!(matches!(
        reply.payload,
        Some(proto::server_message::Payload::Ok(proto::Ok {
            request_id: 31
        }))
    ));
    let list = next_server(&mut second, Some(&second_box)).await;
    assert!(matches!(
        list.payload,
        Some(proto::server_message::Payload::VirtualDeviceList(_))
    ));

    first
        .send(WsMessage::Binary(seal(&first_box, 5, &authenticate).into()))
        .await
        .expect("first authenticate");
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(400), first.next())
            .await
            .is_err(),
        "the first client must not consume the second client's approval"
    );

    let cancel = proto::ClientMessage {
        payload: Some(proto::client_message::Payload::CancelAuthenticate(
            proto::CancelAuthenticate { request_id: 32 },
        )),
    };
    first
        .send(WsMessage::Binary(seal(&first_box, 6, &cancel).into()))
        .await
        .expect("cancel authenticate");
    let reply = next_server(&mut first, Some(&first_box)).await;
    assert!(matches!(
        reply.payload,
        Some(proto::server_message::Payload::Ok(proto::Ok {
            request_id: 32
        }))
    ));

    drop(command_tx);
    task.abort();
}

#[tokio::test]
async fn pairing_waits_for_the_host_decision() {
    let directory = tempfile::tempdir().expect("temp dir");
    let mut config = MobileConfig::new("PAIRINGTEST000000000", "pair-host");
    config.auto_trust = false;
    config.trust_file = Some(directory.path().join("trusted.txt"));
    let trust_file = config.trust_file.clone();
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

    let (client_public, client_secret) = client_keys(9);
    let hello = proto::ClientMessage {
        payload: Some(proto::client_message::Payload::Hello(
            proto::HelloFromClient {
                public_key: client_public,
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
    let reply = next_server(&mut ws, None).await;
    let Some(proto::server_message::Payload::Hello(server)) = reply.payload else {
        panic!("expected HelloFromServer");
    };
    let shared = shared_box(&server.public_key, &client_secret);
    assert!(server.client_needs_authentication);
    let list = next_server(&mut ws, Some(&shared)).await;
    assert!(matches!(
        list.payload,
        Some(proto::server_message::Payload::VirtualDeviceList(_))
    ));
    let code = deck.pending_pairing().expect("pending pairing code");
    assert_eq!(code.len(), 6);

    let authenticate = proto::ClientMessage {
        payload: Some(proto::client_message::Payload::Authenticate(
            proto::Authenticate { request_id: 5 },
        )),
    };
    ws.send(WsMessage::Binary(seal(&shared, 1, &authenticate).into()))
        .await
        .expect("send authenticate");

    let early = tokio::time::timeout(std::time::Duration::from_millis(400), ws.next()).await;
    assert!(early.is_err(), "server must wait for the decision");

    deck.resolve_pairing(true);
    let reply = next_server(&mut ws, Some(&shared)).await;
    assert!(matches!(
        reply.payload,
        Some(proto::server_message::Payload::Ok(proto::Ok {
            request_id: 5
        }))
    ));
    assert!(
        trust_file.as_ref().is_some_and(|path| path.exists()),
        "the trusted key must be persisted"
    );

    drop(command_tx);
    task.abort();
}

#[tokio::test]
async fn sends_the_stored_snapshot_when_the_device_opens() {
    let mut config = MobileConfig::new("SNAPSHOTTEST00000000", "snapshot-host");
    config.auto_trust = true;
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

    command_tx
        .send(multideck_core::DeviceCommand::SetKeyImage {
            key: 3,
            image: std::sync::Arc::new(image::RgbaImage::from_pixel(
                32,
                32,
                image::Rgba([40, 80, 120, 255]),
            )),
        })
        .expect("publish image");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
        .await
        .expect("connect");
    let (client_public, client_secret) = client_keys(11);
    let hello = proto::ClientMessage {
        payload: Some(proto::client_message::Payload::Hello(
            proto::HelloFromClient {
                public_key: client_public,
                client_information: Some(proto::ClientInformation {
                    name: "Snapshot phone".into(),
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
    let reply = next_server(&mut ws, None).await;
    let Some(proto::server_message::Payload::Hello(server)) = reply.payload else {
        panic!("expected HelloFromServer");
    };
    let shared = shared_box(&server.public_key, &client_secret);
    let list = next_server(&mut ws, Some(&shared)).await;
    assert!(matches!(
        list.payload,
        Some(proto::server_message::Payload::VirtualDeviceList(_))
    ));

    let authenticate = proto::ClientMessage {
        payload: Some(proto::client_message::Payload::Authenticate(
            proto::Authenticate { request_id: 12 },
        )),
    };
    ws.send(WsMessage::Binary(seal(&shared, 2, &authenticate).into()))
        .await
        .expect("send authenticate");
    let reply = next_server(&mut ws, Some(&shared)).await;
    assert!(matches!(
        reply.payload,
        Some(proto::server_message::Payload::Ok(proto::Ok {
            request_id: 12
        }))
    ));
    let list = next_server(&mut ws, Some(&shared)).await;
    assert!(matches!(
        list.payload,
        Some(proto::server_message::Payload::VirtualDeviceList(_))
    ));

    let open = proto::ClientMessage {
        payload: Some(proto::client_message::Payload::OpenVirtualDevice(
            proto::OpenVirtualDevice {
                request_id: 9,
                device_id: "multideck".into(),
                keypad_config: Some(proto::KeypadConfig {
                    supported_icon_image_formats: vec![proto::ImageFormat::Png as i32],
                    icon_size: Some(proto::Size {
                        width: 64,
                        height: 64,
                    }),
                    high_dpi: false,
                    overscale: false,
                    layout_crop: None,
                    enable_context_api: false,
                    pressed_icon_size: None,
                }),
            },
        )),
    };
    ws.send(WsMessage::Binary(seal(&shared, 1, &open).into()))
        .await
        .expect("send open");

    let reply = next_server(&mut ws, Some(&shared)).await;
    assert!(
        matches!(
            reply.payload,
            Some(proto::server_message::Payload::VirtualDeviceOpened(_))
        ),
        "unexpected reply: {:?}",
        reply.payload
    );
    let reply = next_server(&mut ws, Some(&shared)).await;
    let Some(proto::server_message::Payload::Icon(icon)) = reply.payload else {
        panic!("expected the stored icon after opening the device");
    };
    assert_eq!(icon.index, 3);

    drop(command_tx);
    task.abort();
}

#[tokio::test]
async fn a_trusted_client_opens_without_authenticating() {
    let directory = tempfile::tempdir().expect("temp dir");
    let (client_public, client_secret) = client_keys(13);
    let trust_file = directory.path().join("mobile_trusted.txt");
    std::fs::write(
        &trust_file,
        client_public
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
    )
    .expect("trust file");

    let mut config = MobileConfig::new("TRUSTEDTEST000000000", "trusted-host");
    config.auto_trust = false;
    config.trust_file = Some(trust_file);
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
                public_key: client_public,
                client_information: Some(proto::ClientInformation {
                    name: "Trusted phone".into(),
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
    let reply = next_server(&mut ws, None).await;
    let Some(proto::server_message::Payload::Hello(server)) = reply.payload else {
        panic!("expected HelloFromServer");
    };
    assert!(!server.client_needs_authentication);
    let shared = shared_box(&server.public_key, &client_secret);
    let list = next_server(&mut ws, Some(&shared)).await;
    assert!(matches!(
        list.payload,
        Some(proto::server_message::Payload::VirtualDeviceList(_))
    ));

    let open = proto::ClientMessage {
        payload: Some(proto::client_message::Payload::OpenVirtualDevice(
            proto::OpenVirtualDevice {
                request_id: 4,
                device_id: "multideck".into(),
                keypad_config: None,
            },
        )),
    };
    ws.send(WsMessage::Binary(seal(&shared, 1, &open).into()))
        .await
        .expect("send open");
    let reply = next_server(&mut ws, Some(&shared)).await;
    assert!(
        matches!(
            reply.payload,
            Some(proto::server_message::Payload::VirtualDeviceOpened(_))
        ),
        "unexpected reply: {:?}",
        reply.payload
    );

    drop(command_tx);
    task.abort();
}
