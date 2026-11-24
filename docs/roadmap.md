# Roadmap

MultiDeck is built in small, testable steps. This page records what is done, what is next, and what is deliberately out of scope. Dates are not promised; order is.

## Status

| Area | State | Notes |
| --- | --- | --- |
| Editor and profile store | Done | Sections, pages, key slots, per-profile brightness |
| Virtual deck | Done | 5x3, same contract as hardware |
| Native actions | Done | Ten actions with typed settings |
| Rendering | Done | SVG icons, system fonts, BMP, JPEG, PNG, raw RGBA |
| Localization | Done | English and Brazilian Portuguese |
| Elgato USB adapter | Implemented | Model detection, images, keys, encoders and touch; hardware validation pending |
| Stream Deck Mobile adapter | Implemented, experimental | VSD2 WebSocket, mDNS, protobuf, pairing and key images; needs a real phone pass |
| Mirabox and Ajazz adapter | Planned | Shared HID protocol with a model table |
| Delay scheduling | Done | Multi actions wait between steps without blocking the interface |
| Profile sync | Done | A directory watcher reloads profiles edited outside the app |
| Installers and packages | Partial | Windows installer via `scripts/build-installer.ps1`; Linux packages planned |

## Next: validate the Elgato adapter

1. Connect a MK.2, an XL and a Plus and confirm the reported grid and image size.
2. Run a hotkey, a media action and a page jump from the hardware.
3. Turn an encoder and touch the strip on the Plus.
4. Unplug and replug: the app should report the disconnect and reconnect on the next launch.
5. Add a device selector so profiles can target one deck when several are attached.

Acceptance: a Stream Deck MK.2 shows the active page and runs a hotkey, and a Stream Deck + reports encoder and touch input.

## Then: scheduling and editor depth

- Add a visual multi action editor instead of raw JSON steps.
- Show the last external reload and the watcher state in Settings.
- Show encoder and touch strip bindings when the device has them.
- Add a key label editor next to the action inspector.

These are small changes that make the existing feature set comfortable before more hardware lands.

## Next: Stream Deck Mobile with a real phone

The adapter exists in `packages/libs/devices/sdmobile` and is enabled with `MULTIDECK_DEVICE=mobile`. It hosts the VSD2 WebSocket server, advertises `_vstreamdeck2._tcp`, completes the `HelloFromClient` / `HelloFromServer` exchange, answers `Authenticate`, opens a virtual device and streams key images while forwarding presses. The protocol record is at `docs/research/sdmobile-protocol.md`.

What remains is a real phone pass:

1. Pair the phone, compare the six digit code logged by the host, and confirm that both sides show the same value.
2. Confirm the pairing card code matches the phone and that trusting it persists across reconnects.
3. Check the requested keypad sizes, image formats and page messages against the phone, then adjust the image encoder if the phone asks for LZ4 or LZFSE.
4. Repeat after a phone sleep and a network change.

## Later: Mirabox and Ajazz

One adapter with a model table, built on the publicly documented HID protocol. The goal is that adding a new variant is a capability row and a test vector, not a new crate.

## Later: distribution

- Linux packages (`.deb`, Flatpak) and a desktop entry.
- Signed binaries and a reproducible build in CI.
- A public download page on this site.

## Not planned

- Cloud accounts, sync or telemetry. Profiles are local files, and that is the feature.
- A plugin marketplace. Actions are compile time registrations for now.
- Emulating a deck for other software. MultiDeck drives decks, it does not pretend to be a device.

## Contributing to the roadmap

Open an issue that names the item and the acceptance check, or send a pull request with tests. Items move from planned to in progress when a branch exists, not when someone intends to start.

Read [development](/development) for commands, the action recipe and the adapter checklist.
