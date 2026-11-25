# Queue device key presses between polls

## Status

Accepted

## Context

The desktop polls the event bus every 250 ms. A single `Option<u8>` kept only the last `KeyDown`, so two presses inside one poll window lost the first press. A delayed action also needs to finish before the next queued press starts.

## Decision

Keep `KeyDown` events in a `VecDeque<u8>` and consume them in order. When an action returns a delay, stop consuming the queue and continue from `EffectsReady` after the delay completes.

## Consequences

Rapid presses keep their order and multiplicity. The queue can grow when the device reports input faster than the host completes actions, which is intentional because dropping input is worse than applying it later.
