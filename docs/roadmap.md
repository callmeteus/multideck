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
| Elgato USB adapter | In progress | HID enumeration, input reports, image encoding |
| Stream Deck Mobile adapter | Research | Protocol, pairing and licensing questions open |
| Mirabox and Ajazz adapter | Planned | Shared HID protocol with a model table |
| Delay scheduling | Planned | The effect exists, the host does not sleep yet |
| Installers and packages | Planned | No release artifacts yet |

## Next: finish the Elgato adapter

1. Enumerate connected decks and map each model to `DeviceCaps`.
2. Read key, encoder and touch strip reports.
3. Encode key images at the format each generation expects.
4. Recover from unplug and replug without restarting the app.
5. Add a device selector to the interface so profiles can target one deck.

Acceptance: a Stream Deck MK.2 shows the active page, runs a hotkey, and survives a USB reconnect.

## Then: scheduling and editor depth

- Honour `Delay` effects between multi action steps.
- Replace the profile id text field with a picker driven by the profile list.
- Edit multi action steps in the inspector instead of JSON.
- Show encoder and touch strip bindings when the device has them.
- Add a key label editor next to the action inspector.

These are small changes that make the existing feature set comfortable before more hardware lands.

## Research: Stream Deck Mobile

The adapter does not exist. The research must answer four questions before code starts:

1. Which steps of pairing happen on the local network and which require the vendor service.
2. The image format, size and update cadence the mobile app expects.
3. How input events are delivered and acknowledged, including touch and swipe.
4. Whether the vendor terms permit a third party host, and under which conditions.

If an answer makes a compatible adapter impossible, the research is still useful: it documents the constraint instead of leaving a half written crate in the tree.

## Later: Mirabox and Ajazz

One adapter with a model table, built on the publicly documented HID protocol. The goal is that adding a new variant is a capability row and a test vector, not a new crate.

## Later: distribution

- Windows installer with the icon set from `assets/icons/`.
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
