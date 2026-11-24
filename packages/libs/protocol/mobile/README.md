# `multideck-protocol-mobile` (reserved)

This is a placeholder for a contract crate that does not exist yet. There is no
`Cargo.toml`, no source and no workspace member.

## Scope

When implemented, this crate owns the wire contract between:

- the MultiDeck desktop app (host side), and
- our own mobile client in `packages/apps/mobile`.

It will define the messages, their serialization, version negotiation and the
pairing handshake. Both ends depend on this crate, so neither end needs to know
the other's implementation.

## What it will not be

- Not the Elgato Stream Deck Mobile protocol. That belongs to
  `packages/libs/devices/sdmobile`, which is a host-side device adapter.
- Not a transport. It will describe messages; the caller chooses how they move.
- Not a place for UI, rendering or action logic. Those stay in `libs/ui`,
  `libs/render` and `libs/actions`.

## Status

Nothing is implemented. No decisions about serialization, framing,
authentication or version negotiation have been made. When the first real
version is written, this file is replaced by documentation of the actual
contract, and the crate is added to the root workspace members.
