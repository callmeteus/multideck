# Getting started

MultiDeck is a desktop application. It does not need a physical stream deck to be useful: the built-in virtual deck runs in the same window, so the editor and the action set can be tested before any hardware is connected.

## Requirements

| Platform | Requirement |
| --- | --- |
| Windows | Windows 10 or newer and the MSVC toolchain (Visual Studio Build Tools with the C++ workload) |
| Linux | A distribution with X11 or Wayland development headers, `pkg-config`, and `libxkbcommon` |
| Both | Rust 1.99 or newer; the repository pins the toolchain in `rust-toolchain.toml` |

The graphical stack is iced 0.14 with the tiny-skia renderer, so no web view or system browser is involved.

## Build from source

```sh
git clone https://github.com/callmeteus/multideck.git
cd multideck
cargo run -p multideck-desktop
```

The binary is called `multideck`. For a release build:

```sh
cargo build --release -p multideck-desktop
```

Compiled binaries are not published yet, but the repository can build the Windows installer locally:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/build-installer.ps1
```

It wraps the release build and Inno Setup 6, and leaves the installer in `dist/`. Linux packages are tracked in the [roadmap](/roadmap).

## First run

1. The window opens on the editor with a default profile sized for the virtual deck (5 columns by 3 rows).
2. Click a key, pick an action from the palette on the right, and press **Apply**.
3. Press the key on the virtual deck to run the action.
4. Use **Settings** to change the interface language or the default brightness.

Everything is saved as soon as it changes. There is no save button and no unsaved state.

## Where data lives

All state lives under one directory, chosen by `dirs::config_dir()`:

| Platform | Default path |
| --- | --- |
| Windows | `%APPDATA%\multideck` |
| Linux | `~/.config/multideck` |

Set `MULTIDECK_DATA_DIR` to use another directory, for example to keep separate profiles per device or to run from a portable folder:

```sh
MULTIDECK_DATA_DIR=./portable cargo run -p multideck-desktop
```

The layout is stable and safe to back up:

```text
<data dir>/
  settings.toml          locale, default brightness, active profile
  profiles/<id>.json     one file per profile
  assets/                user assets referenced by actions
```

To move a setup to another machine, copy the whole directory. To share a single layout, copy one file from `profiles/`.

## Settings file

`settings.toml` is small on purpose:

```toml
locale = "pt-BR"
brightness = 80
active_profile = "6a1f95d2-3c4e-4a1e-9a8e-3d5a0f2b7c11"
```

An empty `locale` means "detect from the operating system". A malformed file is reported in the log and replaced with defaults; it never blocks startup.

## Command line options

The binary currently takes no arguments. Environment variables are the supported configuration surface:

| Variable | Effect |
| --- | --- |
| `MULTIDECK_DATA_DIR` | Overrides the data directory |
| `RUST_LOG` | Log filter, for example `RUST_LOG=multideck=debug` |

## Troubleshooting

**The window opens but keys are blank.** Keys are rendered after the first profile load. If they stay blank, run with `RUST_LOG=multideck=debug` and check for render errors; an action with an invalid icon name is reported per key.

**Hotkeys or typed text do nothing on Linux Wayland.** Input simulation goes through the compositor. Some compositors require the `uinput` permission or a portal enabled; X11 sessions do not have this restriction.

**A command action does nothing.** Commands run without a visible console and without waiting for completion. Test the exact string in a terminal first, then check the log for a spawn error.

**A profile disappears from the list.** Profiles that fail to parse are skipped with a warning, never deleted. The file is still in `profiles/`; fix the JSON or remove the file to reset that profile.

Next: [devices](/guide/devices) explains what the virtual deck is and how adapters fit, and [actions](/guide/actions) documents every native action.
