# Applications

Three things live under `packages/apps`, and they are not interchangeable:

| Path | What it is | Status |
|------|------------|--------|
| `desktop` | The MultiDeck product: an iced desktop application that hosts devices, profiles, actions and the key grid. | In development |
| `mobile` | Our own mobile client, if we build one. | Reserved, empty |
| (`packages/libs/devices/sdmobile`) | Adapter for the official Elgato Stream Deck Mobile app. Not an app, a device adapter. | Planned, empty |

## The boundary

`desktop` is the only user-facing product. It owns the UI, the profile model, the
action registry and the rendering pipeline. It does not know how any particular
device speaks on the wire.

A device is reached through a `DeviceAdapter` from
`packages/libs/devices/api`. Adapters translate between
`multideck_core::DeviceCommand` / `DeviceInput` and whatever the device needs.
The desktop app only ever holds a command sender and an input receiver.

That means:

- `desktop` must not contain protocol code, sockets, mDNS discovery or pairing
  flows for a specific device.
- `libs/devices/*` must not depend on `iced`, on the desktop app, or on each
  other.
- A second device family is added by writing another adapter, not by branching
  inside the UI.

## Status

- `desktop`: being written. It is the crate that CI builds as `multideck`.
- `mobile`: a placeholder directory with a README only. There is no Cargo
  package yet, so `cargo build --workspace` ignores it.
- `libs/devices/sdmobile`: does not exist yet. When it lands it will implement
  `DeviceAdapter` and speak the Elgato protocol to the official mobile app.
  Until the protocol work in `docs/research/` clears its gate, nothing is
  written there.

## Why keep the Elgato path in an adapter

The official Stream Deck Mobile app is a closed third-party client. Its protocol
can change without notice and may be pinned to Elgato's keys. Isolating it in
`libs/devices/sdmobile` keeps that risk away from the product: if the adapter
breaks, the desktop app and our own virtual deck still work, and `mobile` has a
contract of its own to follow.
