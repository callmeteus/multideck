# Require an authenticated phase for mobile device commands

## Status

Accepted

## Context

The Stream Deck Mobile session created a cipher during `Hello` and accepted device commands without calling `Authenticate` first. The pairing decision was stored in one global slot, so one client could read an approval intended for another. Waiting for that decision blocked the WebSocket loop for up to 120 seconds and stopped keepalive frames.

## Decision

Track `SessionPhase` with `AwaitingHello`, `AwaitingAuthentication` and `Authenticated`. Reject device commands outside `Authenticated`. Give each session its own pairing decision and notification handle. Wait for the decision from the main `select!` loop, with a 120 second timeout.

## Consequences

An unauthenticated client cannot open a virtual device or send key presses. A client only receives the decision for its own request. Keepalive, pings and socket closure continue to work while a pairing is pending. The trust file format and public API stay unchanged.
