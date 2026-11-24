# Devices

A device is anything that can show a grid of keys and report presses. MultiDeck ships one device adapter today, the virtual deck, and keeps hardware support behind the same contract.

## Supported devices

| Device | Transport | State | Notes |
| --- | --- | --- | --- |
| Virtual deck | In process | Available | 5x3 keys, RGBA images, always present |
| Elgato Stream Deck (USB) | USB HID | In progress | First hardware adapter; model table drives capabilities |
| Stream Deck Mobile | Network | Research | Protocol and pairing flow not implemented |
| Mirabox and Ajazz decks | USB HID | Planned | Same protocol family as the Mirajazz project |

The devices screen lists what the current build can drive. It does not list planned adapters, so an empty hardware section means the build has no hardware adapter compiled in.

## The virtual deck

The virtual deck is a real adapter, not a simulator bolted onto the UI. It follows the same command and input contract as hardware:

| Property | Value |
| --- | --- |
| Id | `virtual` |
| Name | Virtual Deck |
| Model | `multideck-virtual` |
| Transport | Virtual |
| Grid | 5 columns, 3 rows (15 keys) |
| Key image | Raw RGBA |

It is useful for three things: developing actions without hardware, reproducing layouts before buying a deck, and taking screenshots for documentation. Pressing a key on the virtual deck sends the same `KeyDown` and `KeyUp` inputs a USB deck would send.

## What an adapter sees

Adapters know nothing about actions, profiles or the UI. They receive images and report input.

### Descriptor

Every adapter reports a descriptor before it starts:

| Field | Meaning |
| --- | --- |
| `id` | Stable id used by profiles and logs |
| `name` | Label shown in the interface |
| `model` | Hardware or software model, for example `Stream Deck MK.2` |
| `transport` | `virtual`, `usb` or `network` |
| `caps` | Grid size, key image size, encoders, touch strip, image format |

### Capabilities

`DeviceCaps` drives layout and rendering:

| Field | Effect |
| --- | --- |
| `cols`, `rows` | Grid shape; `key_count()` is their product |
| `key_image` | Pixel size the device expects per key |
| `encoders` | Number of rotary encoders |
| `touch_strip` | Whether the device has a touch strip above the keys |
| `format` | `bmp`, `jpeg`, `png` or `raw`, applied when encoding |

Profiles are resized to the capabilities of the active device with `fit_to`: pages grow with empty slots or shrink, and the active page index is clamped.

### Commands

The host sends three commands through one channel:

| Command | Meaning |
| --- | --- |
| `SetKeyImage { key, image }` | Replace the pixels of one key |
| `SetBrightness { percent }` | Set panel brightness from 0 to 100 |
| `Reset` | Clear every key and return to defaults |

Images arrive as `Arc<RgbaImage>`. Encoding to the device format is the adapter's job, because only the adapter knows which format its model accepts.

### Inputs

Adapters report inputs through a second channel:

| Input | Meaning |
| --- | --- |
| `KeyDown { key }` | Key pressed |
| `KeyUp { key }` | Key released |
| `EncoderRotate { index, ticks }` | Encoder moved; negative is counter clockwise |
| `EncoderPress { index }` | Encoder pressed |

The supervisor in `multideck-events` forwards each input to the application bus with the device id. The UI never talks to an adapter directly, which is why a failing device cannot freeze the interface.

## Lifecycle

1. The adapter is created and its descriptor is published as `DeviceConnected`.
2. Its `run` task receives commands and emits inputs until the channel closes or the transport fails.
3. On exit, `DeviceDisconnected` is published and the failure reason, when there is one, is shown as a status message.

An adapter that loses its device mid session should return a `DeviceError::Failed` with a human readable reason. The application keeps running and the profile stays intact.

## Elgato USB (in progress)

The Elgato adapter targets the official USB HID decks. The work in progress covers:

- Enumerating supported models and mapping each one to a `DeviceCaps` entry.
- Reading key, encoder and touch strip input reports.
- Writing 72x72 or 96x96 key images in the format each generation accepts (BMP on first generation, JPEG on later models).
- Handling disconnect and reconnect without restarting the app.

The adapter lives in its own crate and is compiled into the desktop build when ready. No HID permissions are requested until the adapter is enabled.

## Stream Deck Mobile (research)

Stream Deck Mobile is the official phone and tablet app. It is not a USB device: the phone and the desktop have to find each other and exchange images and input over the local network, and the app uses Elgato's own account and pairing service.

The research phase is limited to answering:

- Which discovery and pairing steps happen locally, and which require the vendor service.
- What image format and size the app expects for a key.
- How input events are delivered and acknowledged.
- What the vendor terms allow for a third party host.

No `devices/sdmobile` crate exists yet. Until those questions have answers, the roadmap keeps the adapter in research rather than in progress.

## Mirabox and Ajazz (planned)

Several Mirabox and Ajazz decks speak the same HID protocol, which the open source Mirajazz project documents. The plan is a single adapter with a model table, so adding a new variant means adding a row, not a new crate.

Next: [profiles](/guide/profiles) explains how keys are stored, and [development](/development) shows how to add an adapter.
