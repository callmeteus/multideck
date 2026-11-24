# Ghidra notes: Stream Deck desktop and mobile link

Rules for this file:

- Every claim needs a binary hash, an address or offset, and what was on screen.
- "Interesting" is not evidence. A string is evidence of a string, not of a
  protocol.
- Never paste vendor code or long decompiler output. Quote the minimum.
- Keep personal data out: no phone names, no account e-mails, no tokens.

## Status

| Area | Status | Notes |
|---|---|---|
| Tooling installed (Ghidra 12.1.4, JDK 21) | Done | JDK 21 at `C:\Users\Gio\scoop\apps\temurin21-jdk\current`, `analyzeHeadless.bat` verified |
| Target acquired | Done | Official 7.6.1 MSI downloaded and extracted with `msiexec /a`, no system install |
| REA MCP reachable (`npx -y rea-agents@6.10.1`) | Not started | Static analysis so far used `multideck-probe strings` and the live app |
| Binary inventory | Done | See below |
| Cheap string pass | Done | Application protocol located |
| Discovery code located | Done | `_vstreamdeck2._tcp`, `_vstreamdeck._tcp`, `_elg._tcp` |
| Session channel located | Done | VSD2 WebSocket, plain `ws://`, port 28198 in the observed run |
| Pairing flow located | Partly | QR template and `Authenticate` messages found; secret unknown |
| TLS or pinning confirmed | Done | No TLS on VSD2; plain WebSocket with HTTP 101 |
| Ghidra decompilation | Not started | Next step: descriptor table and auth path |
| Dynamic capture cross-check | Not started | Needs the phone on the PC hotspot |

## Binary inventory

| File | Size | SHA-256 (first 16) | Version | Role |
|---|---|---|---|---|
| `Stream_Deck_7.6.1.23033.msi` | 219,086,848 | `A42157FA5A44F1BC` | 7.6.1.23033 | Installer, extracted with `msiexec /a` |
| `PFiles64\Elgato\StreamDeck\StreamDeck.exe` | 28,1 MB | `D9487481D42E5C8D` | 7.6.1.23033 | Main process, Qt 6, hosts both device servers |
| `Qt6WebEngineCore.dll` | 195,3 MB | not hashed | Qt 6 | Property inspectors and marketplace UI |

The app bundle also ships `hotkeys_handler.exe`, `crashpad_handler.exe` and
QtWebEngine helpers. The plugin SDK server runs on `127.0.0.1:28196` and is not
part of the phone link.

## Interesting strings

Offsets are absolute file offsets in `StreamDeck.exe` 7.6.1.23033, reproducible
with `multideck-probe strings <exe> --grep <term>`.

| Offset | String | Lead | Followed up |
|---|---|---|---|
| 23758880 | `_vstreamdeck2._tcp` | VSD2 mDNS service | Yes, live server confirmed |
| 23752256 | `_vstreamdeck._tcp` | legacy VSD1 service | Yes |
| 23713760 | `_elg._tcp` | CORA network dock | Not needed for the phone |
| 23751784 | `### [SERVER] port %d is available; starting Bonjour` | VSD1 server lifecycle | Yes, logic read from logs |
| 23759032 | `ESDVSD2Server::onNewWebSocketConnection` | VSD2 transport is WebSocket | Yes, handshake verified |
| 23759128 | `### [VSD2 Server] sodium_init error` | libsodium in the auth path | Partly |
| 23370064 | `... select "Add computer", and scan this QR code.` | Pairing UI text | Yes |
| 23370472 | `https://streamdeck.elgato.com/c/?t=%1&w=%2` | QR payload template | Partly, parameters unknown |
| 22688096 | `vsd2.HelloFromServer workstationID hostname version osName osVersion` | Protobuf descriptors | Yes, full message list recovered |
| 23752720 | `### [SERVER] incoming ctrl inquire -- creating session` | VSD1 ctrl/data channels | Not needed |
| 23770368 | `vsd2.HelloFromServer.workstationID` | Reflection style descriptor name | Confirms protobuf |

Protobuf, OpenSSL, libsodium and nlohmann/json all appear as dependencies. The
crypto strings from OpenSSL are library noise; only `sodium_init` next to the VSD2
server is evidence of application level cryptography.

## Functions

No function has been decompiled yet. The class names below come from RTTI strings
in the binary, which is why they are listed as leads and not as confirmed code:

| Symbol (RTTI string) | Why it matters | Confidence |
|---|---|---|
| `ESDVSD2Server` | Owns the WebSocket listener and sessions | Likely |
| `ESDVSD2Server::onNewWebSocketConnection` | Entry point for every phone connection | Confirmed by log line |
| `ESDVSD2Handler` | Message dispatch for `vsd2.*` | Likely |
| `ESDVSD2NotifierImp` | Server to client notifications | Likely |
| `ESDVSDServer` | Legacy ctrl/data TCP server | Confirmed by log line |
| `ESDCoraDocksManagerEx` | CORA dock discovery and pairing | Confirmed by strings |
| `ESDMobileConnectivityView` | Pairing dialog with the QR code | Confirmed by strings |
| `ESDPairingHandler`, `ESDCoraPairingDialog` | Pairing state machines | Likely |

## Cross references

No decompiled edges yet. The path to replay in Ghidra:

1. From string `vsd2.HelloFromServer` to the generated protobuf descriptor table.
2. From `onNewWebSocketConnection` to the handler that reads the first frame.
3. From `sodium_init` to the `Authenticate` implementation.

## Live observations

The extracted build was started directly from its directory (no install):

```text
### [VSD2 Server] init with workstationID=WE1qigCf1de1Iw5uzt8fhg
### [VSD2 Server] port 28198 is available; starting Bonjour
### [VSD2 Server] resumed
```

Listening sockets owned by the process:

```text
[::]:28198          VSD2 WebSocket (phone, virtual devices)
127.0.0.1:28196     plugin SDK WebSocket
```

WebSocket upgrade against `ws://[::1]:28198/` returned `101 Switching Protocols`
with `Server: VSD2 Server` and `Access-Control-Allow-Origin: *`. The server sent
no frame before the client spoke.

The log also shows a Windows credential read/write per connection
(`EGQTCredentialStore`, `CredRead() err: 1168` on a fresh profile), which is where
the pairing secret is likely stored.

## Evidence log

```text
2026-11-24
Ghidra 12.1.4 (headless verified), JDK 21.0.x, multideck-probe built from this repository
Binary: Stream_Deck_7.6.1.23033.msi sha256:A42157FA5A44F1BC...
        StreamDeck.exe 7.6.1.23033 sha256:D9487481D42E5C8D...
Steps: downloaded the official MSI, extracted with msiexec /a, ran the app from the
       extracted tree, listed sockets, performed a manual WebSocket upgrade,
       scanned strings with multideck-probe.
Observed: the VSD2 server, its Bonjour service, the protobuf package names, the
       QR template and the authentication message pair.
Conclusion: confirmed transport and message schema names; field numbers and the
       auth secret remain unknown.
Next: capture one pairing session from the phone on a PC hotspot.
```

## Open questions

- [x] Is the mobile link owned by a native service, a bundled script, or the main
      window process? Native, inside `StreamDeck.exe`.
- [x] Does discovery use multicast mDNS or unicast DNS-SD? mDNS via Bonjour,
      `_vstreamdeck2._tcp`.
- [x] Is the session channel plaintext framing, TLS, or TLS with pinning? Plain
      WebSocket, no TLS.
- [ ] Where does the pairing secret live, and is it per-device or per-account?
      It is read and written through the Windows credential store; the value and
      the QR parameter `t` relationship are unverified.
- [ ] Does the protocol carry key images as PNG, JPEG, raw RGBA or a custom
      encoding? `vsd2.Image` carries ImageFormat and ImageType; the enumerations
      still need mapping.
- [ ] What is the maximum frame size, and is there a length field before the
      message bytes? Unknown until a session is captured.

## TODOs

- [x] Install JDK 21 and confirm Ghidra starts.
- [x] Acquire the installer and extract it without touching the system.
- [x] Run the cheap string pass on the candidates.
- [ ] Import `StreamDeck.exe` in Ghidra and recover the protobuf descriptor table.
- [ ] Read the `Authenticate` path and the credential storage calls.
- [ ] Capture one pairing session with the phone on a PC hotspot.
- [ ] Update `sdmobile-protocol.md` when the unknowns close.
