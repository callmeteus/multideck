# Stream Deck Mobile protocol: phase 0 decision record

Status: **open**. No capture, no decompilation and no reproducible session yet.
This record exists so the decision is made on evidence instead of optimism.

Date opened: 2026-11-24.

## 1. Question

Should MultiDeck build a host-side adapter that speaks the official Elgato
Stream Deck Mobile protocol (`packages/libs/devices/sdmobile`), with the desktop
app acting as the host for the official phone app?

## 2. Options

| Option | Description | Cost | Risk |
|--------|-------------|------|------|
| A. Adapter | Implement the protocol in `sdmobile`, verified against the real app | High | Vendor changes, legal review, breakage on every app update |
| B. Bridge | Drive the official desktop app or its local API from outside, without implementing the wire protocol | Medium | Fragile, depends on an install we do not control |
| C. Defer | Ship the virtual deck and other adapters, revisit later | Low | The official mobile app stays unsupported |

Option A is the one this record evaluates. B is the fallback if the gate fails.

## 3. Gate

Both gates must pass before any adapter code is written. A partial pass is a
"no" for A.

### Gate A: the link is reproducible

All of the following, from one capture run on our own hardware:

- [ ] Discovery observed, with the mDNS or unicast exchange for `_elg._tcp` in a
      capture file.
- [ ] A complete session, both directions, from the first byte to a key press
      acknowledged by the phone.
- [ ] The desktop log lines that line up with the capture timestamps.
- [ ] The same flow reproduced at least twice, from a cold app start, with the
      same shape (ports, handshake, framing).
- [ ] The binary or script that owns the link, identified by path and hash.

Two identical runs rule out a one-off port or a cached secret. A single run is
not enough, because a random port per session would look stable in one sample.

### Gate B: the channel can be spoken without defeating a lock

Pass if either:

- **B1. Plaintext framing**: the payload is readable and starts with a
  recognisable frame. We can implement it.
- **B2. TLS with a keylog we can obtain**: the app honours `SSLKEYLOGFILE`, so
  the session can be decrypted for analysis and the negotiated protocol can be
  re-implemented.
- **B3. Pinned key that we are allowed to use**: the pinning is documented or
  the key material is shipped in the install and usable by a third-party host.

Fail if:

- TLS with certificate pinning and no keylog, or a custom encrypted handshake we
  cannot read. In that case the protocol is opaque and A is rejected. We do not
  attack the app to get around it.

Note on B3: a shipped key is not automatically usable. It is a legal and
maintenance question as much as a technical one. Treat it as a fail unless
there is a written go-ahead.

## 4. Known public facts

These are reported by third parties. None of them has been reproduced here.

| Fact | Source kind | Confidence | Consequence |
|------|-------------|------------|-------------|
| Discovery uses mDNS service `_elg._tcp` | Open-source clients of the protocol | Reported | First capture filter |
| Frames are called CORA and start with magic `43 93 8A 41` | Open-source protocol notes | Reported | Byte filter and Ghidra search target |
| The desktop app is the host; the phone connects to it | Product behaviour | Verified by using the app | The adapter lives host side |
| Pairing requires a code or confirmation on both ends | Product behaviour | Verified by using the app | Capture must include pairing, not only keys |

"Reported" means we have not confirmed it. If a capture contradicts a reported
fact, the capture wins and this table is corrected.

## 5. Unknowns

Each of these blocks implementation until answered.

- **Transport**: TCP, UDP, or both. Port numbers. Whether the port is fixed.
- **Discovery details**: multicast mDNS or unicast DNS-SD. TXT record contents.
  Whether discovery is also broadcast over a custom packet.
- **Framing**: field order, length encoding, endianness, maximum frame size,
  whether the magic bytes are a delimiter or a header.
- **Handshake**: what the first exchange carries, and whether a session key is
  derived.
- **Pairing**: how the PIN or QR payload is turned into a shared secret, and
  whether it is per device or per account.
- **Authentication**: challenge-response, bearer token, certificate, or nothing.
- **Images**: encoding for key images (PNG, JPEG, raw RGBA, custom palette),
  and the key geometry the app expects (columns, rows, key size in pixels).
- **Inputs**: how a key press and a dial rotate are reported, and whether
  presses are acknowledged.
- **Keepalive**: idle timeout, ping interval, reconnect behaviour.
- **Versioning**: how the app and the desktop negotiate protocol versions, and
  what happens on mismatch.
- **Encryption**: whether any layer is TLS, and if so, which trust store and
  whether pinning is enabled.

## 6. Evidence log

| Date | Run | Discovery | Session | Gate A | Gate B | Notes |
|------|-----|-----------|---------|--------|--------|-------|
| 2026-11-24 | none | no | no | fail (no data) | unknown | Record opened |

A row is added only after a real capture. "No data" is not a failure of the
protocol, only of the schedule.

## 7. Decision

Not made yet. The default is **C (defer)** until Gate A and Gate B are both
satisfied, so that no adapter code is written against guesses.

Re-evaluation triggers:

- A capture run completes and is written up in `capture-playbook.md` terms.
- A binary is analysed and `ghidra-notes.md` has confirmed rows.
- A vendor change makes the reported facts wrong.

## 8. If the gate passes

Order of work, so that a partial result still leaves something useful:

1. Record the raw frames as fixtures, in a binary-safe form, outside git.
2. Write the framing and discovery layer in `sdmobile`, with tests against the
   fixtures. No sockets yet.
3. Add the transport and pairing, behind a `DeviceAdapter`.
4. Validate against the real phone: discovery, pair, press a key, set an image.
5. Only then expose it in the desktop app's device list.

## 9. If the gate fails

- Pick option B or C.
- Keep this record, with the failed gate and the evidence, so the decision is
  not relitigated from memory.
- Keep `docs/research/` honest: a rejected protocol is a result.
