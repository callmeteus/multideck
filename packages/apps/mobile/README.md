# `mobile` (reserved)

This directory is reserved. There is no code here yet, no `Cargo.toml`, and no
crate in the workspace.

## What it will be

A first-party MultiDeck client for phones and tablets: our own app, distributed
by us, showing the key grid that the desktop app hosts. It is not a replacement
for the Elgato app and it does not pretend to be one.

## What it will speak

Our protocol, defined by `packages/libs/protocol/mobile`. Not the Elgato
protocol, not CORA, not `_elg._tcp`. The two links stay separate on purpose:

- Official Elgato app to desktop: handled by `libs/devices/sdmobile`, a device
  adapter on the host side.
- Our mobile client to desktop: handled by the contract crate above, which both
  ends will implement.

Keeping them separate means a change in Elgato's app cannot force a change in
our client, and our client can use whatever transport and authentication we
choose.

## Do not

- Do not add the Elgato adapter here.
- Do not reuse Elgato constants, service names or framing.
- Do not add a `Cargo.toml` until the contract crate exists; an empty crate in
  the workspace only slows CI down.

When work starts, this README is replaced by a real one with build and pairing
instructions.
