# Development

## Requirements

- Rust 1.99, pinned by `rust-toolchain.toml`.
- Node.js 20 or newer and pnpm 10 or newer for the documentation site.
- Platform toolchain for the graphical stack: MSVC on Windows, X11 or Wayland headers on Linux.

## Commands

Run from the repository root:

| Task | Command |
| --- | --- |
| Run the app | `cargo run -p multideck-desktop` |
| Build | `cargo build --workspace` |
| Test | `cargo test --workspace` |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` |
| Format | `cargo fmt --all` |
| Format check | `cargo fmt --all --check` |

The workspace uses resolver 3, edition 2024, and `-D warnings` in the lint command, so a warning is a failing build. Development builds set `debug = 0` and `incremental = false` to keep the target directory small; use a release build when profiling.

## Workspace layout

```text
multideck/
  Cargo.toml                 workspace members and shared dependency versions
  rust-toolchain.toml        pinned toolchain
  locales/
    en-US/main.ftl           English strings
    pt-BR/main.ftl           Portuguese strings
  assets/
    brand/                   logo system and social cover, hand written SVG
    icons/                   generated icon.ico, icon-512.png, tray-32.png
    screenshots/             captures used by the README
  docs/                      this VitePress site
  packages/
    apps/desktop/            the binary
    apps/mobile/             reserved
    libs/core/               shared model
    libs/events/             bus and supervisor
    libs/render/             key rendering
    libs/store/              persistence
    libs/i18n/               translations
    libs/actions/            action set and registry
    libs/platform/           OS side effects
    libs/ui/                 colors and styles
    libs/devices/api/        adapter trait
    libs/devices/virtual/    virtual deck
    libs/protocol/mobile/    reserved
    tools/probe/             protocol experiments
```

## Adding an action

Actions register themselves, so three edits and one test cover the whole feature.

1. Add a definition and an executor. A new module in `packages/libs/actions/src/` is enough:

```rust
use crate::{ActionError, Effect, RegisteredAction};
use multideck_core::{ActionDef, ActionInstance, FieldKind, SettingField};

const SLEEP_FIELDS: &[SettingField] = &[SettingField {
    key: "seconds",
    label_key: "field.seconds",
    kind: FieldKind::Number,
    default: "5",
}];

const SLEEP: ActionDef = ActionDef {
    kind: "sleep",
    label_key: "action.sleep",
    description_key: "action.sleep_desc",
    icon: "delay",
    fields: SLEEP_FIELDS,
    title_key: "action.sleep_title",
};

inventory::submit! { RegisteredAction { def: SLEEP, run: run_sleep } }

fn run_sleep(instance: &ActionInstance) -> Result<Vec<Effect>, ActionError> {
    let seconds = instance.integer("seconds").ok_or_else(|| ActionError::InvalidSetting {
        kind: instance.kind.clone(),
        field: "seconds".into(),
        reason: "expected a number".into(),
    })?;
    Ok(vec![Effect::Delay((seconds.clamp(0, 3600) as u64) * 1000)])
}
```

2. Register the module in `packages/libs/actions/src/lib.rs` with `mod sleep;`.
3. Add the strings to both FTL files: `action.sleep`, `action.sleep_desc`, `action.sleep_title`, plus any new field label such as `field.seconds`.
4. Add an icon if the action uses a new one. Icons are 24x24 path data in `packages/libs/render/src/icons.rs`; the test below fails when the name is missing.

The existing test `every_builtin_is_registered_and_documented` checks that known kinds resolve and each one has an icon. The i18n test `locales_have_the_same_keys` catches a key added to English only. Both run in `cargo test --workspace`.

## Adding a device adapter

1. Create `packages/libs/devices/<name>` and add it to `packages/libs/*` workspace members (they are globbed, so no edit is needed) and to `[workspace.dependencies]`.
2. Depend on `multideck-device-api`, `multideck-core`, `async-trait` and `tokio`.
3. Return a `DeviceDescriptor` from `descriptor()`. Capabilities decide the layout the editor shows, so set `cols`, `rows`, `key_image`, `encoders`, `touch_strip` and `format` to the real model values.
4. Implement `run`. Select over the command channel and the transport, forward `DeviceInput` values, and translate `SetKeyImage` to the device format. Blocking transport reads belong on a dedicated thread that sends into the input channel.
5. Decide how the adapter is created. The desktop app builds the virtual deck in `App::new`; a hardware adapter needs a discovery step and should degrade to "not connected" instead of failing startup.
6. Add tests with a fake transport: descriptor values, one command round trip, and a disconnect path that returns `DeviceError::Failed`.

Adapters must not depend on `multideck-actions`, `multideck-store` or `multideck-ui`. If one needs profile data, the host is missing a command or an input variant.

## Working on protocols

`packages/tools/probe` is the `multideck-probe` helper used for the Stream Deck Mobile research. It has no dependencies and answers three questions from the command line:

```sh
cargo run -p multideck-probe -- strings <binary> --grep streamdeck
cargo run -p multideck-probe -- ports --pid <pid>
cargo run -p multideck-probe -- env
```

The `strings` command scans a binary for printable runs, `ports` lists TCP listeners and can filter by process id, and `env` reports host, build and resolved data directory. Keep protocol experiments there, and move only the parts that become an adapter. Do not commit packet captures or vendor binaries.

## Localization checks

The FTL loader reads `locales/` at compile time. A syntax error fails the build of `multideck-i18n`, and a test compares key sets across languages. When adding an action, add the keys to both files in the same commit.

## Documentation site

The site is VitePress 1.6.4 under `docs/`.

```sh
cd docs
pnpm install --no-frozen-lockfile
pnpm run dev
pnpm run build
pnpm run preview
```

`pnpm run dev` listens on `0.0.0.0:5173`, so the preview is reachable from another machine on the LAN or through Tailscale, not only from localhost. Open `http://<tailscale-ip>:5173` when reviewing the site from a phone or another computer.

`docs/package.json` pins VitePress, `docs/pnpm-workspace.yaml` and the `pnpm.onlyBuiltDependencies` field allow the esbuild build script under both pnpm 10 and pnpm 12, and `docs/pnpm-lock.yaml` is committed. `docs/.vitepress/config.ts` enables local search, excludes `research/` from the build, and lists every page in the sidebar.

`docs/public/` holds build copies of `logo-mark.svg` and `og-cover.svg`. The canonical files are in `assets/brand/`; refresh the copies when the mark changes.

## Brand assets and icons

The logo system is hand written SVG, no icon pack and no generated imagery:

| File | Use |
| --- | --- |
| `assets/brand/logo.svg` | Horizontal lockup, light on dark |
| `assets/brand/logo-mark.svg` | Square symbol |
| `assets/brand/logo-dark.svg` | Lockup in dark ink for light surfaces |
| `assets/brand/og-cover.svg` | 1200x630 social cover |

Icons are generated from the same geometry by a dependency free script:

```sh
node assets/icons/build-icons.mjs
```

It renders the mark with a signed distance field, writes `icon.ico` (16, 32, 48, 64, 128 and 256 pixels, PNG in ICO), `icon-512.png` and `tray-32.png` with a small PNG encoder built on `node:zlib`. Run it after changing the mark so the binaries match the SVG.

## CI

The GitHub Actions workflow runs, in order:

1. `cargo fmt --all --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace`
4. `pnpm install --no-frozen-lockfile` and `pnpm run build` in `docs/`

A pull request is expected to pass all four. Tests that need a display are written against the virtual deck and never open a window, so they run on a headless runner.

## Packaging

- Windows: `powershell -ExecutionPolicy Bypass -File scripts/build-installer.ps1` builds the release binary and compiles `packaging/windows/multideck.iss` with Inno Setup 6. The installer is written to `dist/`, which is ignored by git.
- Linux: `cargo build --release -p multideck-desktop` produces the binary, and `packaging/linux/install-user.sh` installs it in `~/.local` with a desktop entry and icon. `packaging/linux/60-multideck.rules` grants hidraw access to Elgato decks.

## Commit style

Commit messages follow conventional commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`) and are written in English. Keep a change focused: one action, one adapter or one documentation pass per pull request.

Next: [roadmap](/roadmap) lists what is planned after the first hardware adapter.
