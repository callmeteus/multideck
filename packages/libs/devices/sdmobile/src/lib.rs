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

use std::sync::atomic::{AtomicU8, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use async_trait::async_trait;
use image::RgbaImage;
use multideck_core::{DeviceCaps, DeviceCommand, DeviceDescriptor, KeyImageFormat, Transport};
use multideck_device_api::{CommandReceiver, DeviceAdapter, DeviceError, InputSender};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, Notify, broadcast, watch};
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
    /// File where trusted client keys are remembered. `None` keeps trust in memory.
    pub trust_file: Option<std::path::PathBuf>,
    /// File holding the long lived host public key. The phone pins this key, so
    /// it has to survive restarts or every launch looks like a new computer.
    pub key_file: Option<std::path::PathBuf>,
    /// Port to listen on. Zero asks the operating system for a free port.
    pub port: u16,
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
                key_image: (288, 288),
                encoders: 0,
                touch_strip: false,
                format: KeyImageFormat::Png,
            },
            auto_trust: false,
            trust_file: None,
            key_file: None,
            port: 0,
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
static NEXT_PAIRING_ID: AtomicU64 = AtomicU64::new(1);

pub(crate) struct PairingState {
    pub id: u64,
    pub code: String,
    pub client_key: Vec<u8>,
    pub decision: Arc<StdMutex<Option<bool>>>,
    pub notify: Arc<Notify>,
}

impl PairingState {
    pub(crate) fn new(code: String, client_key: Vec<u8>) -> Self {
        Self {
            id: NEXT_PAIRING_ID.fetch_add(1, Ordering::Relaxed),
            code,
            client_key,
            decision: Arc::new(StdMutex::new(None)),
            notify: Arc::new(Notify::new()),
        }
    }

    pub(crate) fn decision(&self) -> Option<bool> {
        self.decision.lock().ok().and_then(|decision| *decision)
    }

    pub(crate) fn resolve(&self, accept: bool) {
        if let Ok(mut decision) = self.decision.lock() {
            *decision = Some(accept);
        }
        self.notify.notify_one();
    }
}

pub(crate) fn clear_pending_pairing(shared: &Shared, id: u64) {
    if let Ok(mut pairing) = shared.pairing.lock()
        && pairing.as_ref().is_some_and(|pending| pending.id == id)
    {
        *pairing = None;
    }
}

pub(crate) struct Shared {
    pub config: MobileConfig,
    pub secret_key: [u8; 32],
    pub public_key: [u8; 32],
    pub latest: StdMutex<Vec<Option<Arc<RgbaImage>>>>,
    pub updates: broadcast::Sender<(u8, Arc<RgbaImage>)>,
    pub inputs: Mutex<Option<InputSender>>,
    pub port: watch::Sender<Option<u16>>,
    pub brightness: AtomicU8,
    pub connected: AtomicUsize,
    pub pro: StdMutex<Option<bool>>,
    pub pairing: StdMutex<Option<PairingState>>,
    pub token: String,
    pub trusted: StdMutex<Vec<Vec<u8>>>,
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
        let secret_key = load_or_create_key(config.key_file.as_deref());
        let public_key = *crypto_box::SecretKey::from(secret_key)
            .public_key()
            .as_bytes();

        let keys = config.caps.key_count() as usize;
        let trusted = load_trusted(config.trust_file.as_deref());
        let (updates, _) = broadcast::channel(64);
        let (port, ports) = watch::channel(None);
        let shared = Arc::new(Shared {
            config,
            secret_key,
            public_key,
            latest: StdMutex::new(vec![None; keys]),
            updates,
            inputs: Mutex::new(None),
            port,
            brightness: AtomicU8::new(80),
            connected: AtomicUsize::new(0),
            pro: StdMutex::new(None),
            token: generate_token(),
            pairing: StdMutex::new(None),
            trusted: StdMutex::new(trusted),
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

    /// Subscription plan the phone reported, when it has said anything.
    pub fn is_pro(&self) -> Option<bool> {
        self.shared.pro.lock().ok().and_then(|pro| *pro)
    }

    /// Whether an authenticated phone session is active right now.
    pub fn is_connected(&self) -> bool {
        self.shared.connected.load(Ordering::Relaxed) > 0
    }

    /// Port the listener is bound to, once the adapter has started.
    pub fn port(&self) -> Option<u16> {
        *self.ports.borrow()
    }

    /// Receiver that reports the port as soon as it is known.
    pub fn port_receiver(&self) -> watch::Receiver<Option<u16>> {
        self.ports.clone()
    }

    /// URL encoded in the pairing QR, mirroring the official desktop format.
    pub fn pairing_url(&self) -> Option<String> {
        self.pairing_url_for(&local_addresses())
    }

    /// Pairing URL limited to the addresses the user selected.
    pub fn pairing_url_for(&self, addresses: &[String]) -> Option<String> {
        let port = self.port()?;
        let mut url = format!(
            "https://streamdeck.elgato.com/c/?t={}&w={}",
            self.shared.token, self.shared.config.workstation_id
        );
        for address in addresses {
            url.push_str(&format!("&av2={address}:{port}"));
        }
        Some(url)
    }
    /// Code of the client waiting for a trust decision, when there is one.
    pub fn pending_pairing(&self) -> Option<String> {
        self.shared.pairing.lock().ok().and_then(|pairing| {
            pairing
                .as_ref()
                .filter(|state| state.decision().is_none())
                .map(|state| state.code.clone())
        })
    }

    /// Accepts or rejects the pending pairing request.
    pub fn resolve_pairing(&self, accept: bool) {
        let approved = if let Ok(pairing) = self.shared.pairing.lock() {
            pairing.as_ref().and_then(|state| {
                state.resolve(accept);
                accept.then(|| state.client_key.clone())
            })
        } else {
            None
        };
        if let Some(key) = approved {
            // The phone drops the socket every few seconds while it waits for the
            // answer, so the decision has to outlive this session. Trusting the key
            // makes the next connection authenticate immediately.
            remember_trust(&self.shared, &key);
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

        let requested_port = self.shared.config.port;
        let listener = match bind_listener(requested_port).await {
            Some(listener) => listener,
            None if requested_port != 0 => {
                tracing::warn!(
                    requested_port,
                    "mobile port is busy, falling back to a free port"
                );
                bind_listener(0).await.ok_or_else(|| DeviceError::Failed {
                    device: "sdmobile".to_string(),
                    reason: "could not bind the mobile listener".to_string(),
                })?
            }
            None => {
                return Err(DeviceError::Failed {
                    device: "sdmobile".to_string(),
                    reason: "could not bind the mobile listener".to_string(),
                });
            }
        };
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

/// One IPv4 address the pairing QR can advertise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkInterface {
    /// Operating system interface name, for example `Ethernet` or `Tailscale`.
    pub name: String,
    /// IPv4 address on that interface.
    pub address: String,
}

/// Lists the IPv4 interfaces that can host the pairing connection.
pub fn interfaces() -> Vec<NetworkInterface> {
    let mut list: Vec<NetworkInterface> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|interface| match interface.ip() {
            std::net::IpAddr::V4(ip) if !ip.is_loopback() && !ip.is_link_local() => {
                Some(NetworkInterface {
                    name: interface.name.clone(),
                    address: ip.to_string(),
                })
            }
            _ => None,
        })
        .collect();

    list.sort_by(|left, right| {
        let left_score = address_score(&left.name, &left.address);
        let right_score = address_score(&right.name, &right.address);
        right_score
            .cmp(&left_score)
            .then_with(|| left.name.cmp(&right.name))
    });
    list.dedup_by(|left, right| left.address == right.address);
    list
}

fn address_score(name: &str, address: &str) -> i32 {
    let Ok(ip) = address.parse::<std::net::Ipv4Addr>() else {
        return 0;
    };
    let mut score = 0;
    if ip.is_private() {
        score += 10;
    }
    // Carrier grade NAT is what Tailscale and similar overlays use; keep them
    // available but behind a real LAN address.
    let octets = ip.octets();
    if octets[0] == 100 && (64..=127).contains(&octets[1]) {
        score -= 30;
    }
    let name = name.to_ascii_lowercase();
    if [
        "tailscale",
        "netbird",
        "zerotier",
        "wsl",
        "vethernet",
        "docker",
        "vmware",
        "virtualbox",
    ]
    .iter()
    .any(|marker| name.contains(marker))
    {
        score -= 20;
    }
    if ip == std::net::Ipv4Addr::new(192, 168, 56, 1) {
        score -= 10;
    }
    score
}
fn generate_token() -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";
    let mut bytes = [0u8; 20];
    if getrandom::fill(&mut bytes).is_err() {
        return "multideckpairing0000".to_string();
    }
    bytes
        .iter()
        .map(|byte| ALPHABET[(*byte as usize) % ALPHABET.len()] as char)
        .collect()
}

fn local_addresses() -> Vec<String> {
    let mut addresses: Vec<std::net::Ipv4Addr> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|interface| match interface.ip() {
            std::net::IpAddr::V4(ip) if !ip.is_loopback() && !ip.is_link_local() => Some(ip),
            _ => None,
        })
        .collect();
    addresses.sort();
    addresses.dedup();

    // The address used for the default route is the one the phone can reach
    // when both are on the same network, so it goes first.
    let primary = primary_address();
    let mut ordered = Vec::with_capacity(addresses.len());
    if let Some(primary) = primary {
        ordered.push(primary);
    }
    for address in addresses {
        if Some(address) != primary {
            ordered.push(address);
        }
    }
    ordered
        .into_iter()
        .map(|address| address.to_string())
        .collect()
}

fn primary_address() -> Option<std::net::Ipv4Addr> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    match socket.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(ip) => Some(ip),
        std::net::IpAddr::V6(_) => None,
    }
}
async fn bind_listener(port: u16) -> Option<TcpListener> {
    match TcpListener::bind(("0.0.0.0", port)).await {
        Ok(listener) => Some(listener),
        Err(error) => {
            tracing::warn!(port, %error, "mobile listener bind failed");
            None
        }
    }
}

fn load_or_create_key(path: Option<&std::path::Path>) -> [u8; 32] {
    if let Some(path) = path
        && let Ok(text) = std::fs::read_to_string(path)
        && let Some(bytes) = decode_hex(&text)
        && let Ok(key) = <[u8; 32]>::try_from(bytes.as_slice())
    {
        return key;
    }

    let mut key = [0u8; 32];
    if getrandom::fill(&mut key).is_err() {
        tracing::warn!("no system randomness available for the mobile keypair");
    }
    if let Some(path) = path
        && let Err(error) = std::fs::write(path, encode_hex(&key))
    {
        tracing::warn!(%error, "could not persist the mobile host key");
    }
    key
}

fn load_trusted(path: Option<&std::path::Path>) -> Vec<Vec<u8>> {
    let Some(path) = path else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines().filter_map(decode_hex).collect()
}

pub(crate) fn is_trusted(shared: &Shared, key: &[u8]) -> bool {
    shared
        .trusted
        .lock()
        .map(|trusted| trusted.iter().any(|known| known == key))
        .unwrap_or(false)
}

pub(crate) fn remember_trust(shared: &Shared, key: &[u8]) {
    let Ok(mut trusted) = shared.trusted.lock() else {
        return;
    };
    if trusted.iter().any(|known| known == key) {
        return;
    }
    trusted.push(key.to_vec());
    if let Some(path) = &shared.config.trust_file {
        let text = trusted
            .iter()
            .map(|key| encode_hex(key))
            .collect::<Vec<_>>()
            .join("\n");
        if let Err(error) = std::fs::write(path, text) {
            tracing::warn!(%error, "could not remember the trusted phone");
        }
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn decode_hex(text: &str) -> Option<Vec<u8>> {
    let text = text.trim();
    if text.is_empty() || !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16).ok())
        .collect()
}

#[cfg(test)]
mod tests;
