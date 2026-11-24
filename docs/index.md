---
layout: home

hero:
  name: MultiDeck
  text: One deck, any device.
  tagline: A desktop app for Windows and Linux that drives stream deck hardware, a built-in virtual deck, and mobile decks through a single adapter layer.
  image:
    src: /logo-mark.svg
    alt: MultiDeck symbol
  actions:
    - theme: brand
      text: Get started
      link: /guide/getting-started
    - theme: alt
      text: Read the architecture
      link: /architecture

features:
  - title: Device adapters
    details: Every deck talks to the same trait. The virtual deck is built in, USB HID and mobile adapters plug into the same contract without touching actions, profiles or the UI.
  - title: Profiles, pages, keys
    details: Profiles are plain JSON files with pages, key slots and per-profile brightness. Copy the data folder to a new machine and the layout comes with it.
  - title: Ten native actions
    details: Hotkey, open, text, media, command, delay, multi action, page jump, profile switch and brightness, with typed settings validated before they run.
  - title: Auto-discovered actions
    details: Actions register themselves at compile time with inventory. Adding one is a new module and a submit macro, never a central list to keep in sync.
  - title: Native rendering
    details: Keys are composed with the iced tiny-skia renderer, icons are rasterised from SVG with resvg, and each adapter receives the image format it expects.
  - title: Two languages
    details: The interface ships in English and Brazilian Portuguese through Fluent, with system locale detection and a manual override in settings.
---

## Project status

MultiDeck is in early development. The editor, profile store, render pipeline and virtual deck are usable today, and the first hardware adapter is being written.

| Component | State |
| --- | --- |
| Desktop shell and editor (iced) | Available |
| Virtual deck, 5x3 keys | Available |
| Profile store and JSON schema | Available |
| Native action set | Available |
| Elgato USB HID adapter | In progress |
| Stream Deck Mobile adapter | Research |
| Mirabox and Ajazz adapter | Planned |

Start with [getting started](/guide/getting-started), then read how [device adapters](/architecture) keep hardware support independent from the rest of the app.

## Requirements

- Windows 10 or newer, or a Linux distribution with X11 or Wayland development headers.
- Rust 1.99 or newer for building from source.

Prebuilt packages are not published yet. The [development guide](/development) covers the build, test and lint commands used by the project.
