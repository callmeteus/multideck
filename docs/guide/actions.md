# Actions

An action is one thing a key can do. MultiDeck ships ten native actions. Each one is registered at compile time, declares its settings, and validates them before touching the operating system.

## How an action runs

1. The adapter reports `KeyDown` or the user presses a key on the virtual deck.
2. The application looks up the action stored in that key slot.
3. The action executor reads its settings and performs either a side effect (a keystroke, an open, a command) or returns effects that only the host can apply.
4. The host applies those effects in order: switch profile, switch page, change brightness, wait.

Effects are separate from side effects on purpose. An action crate never needs to know which profile is open or how many keys a device has.

### Effects

| Effect | Applied by the host |
| --- | --- |
| `SwitchProfile(id)` | Loads the profile with that id |
| `SwitchPage(index)` | Changes the visible page when the index exists |
| `SetBrightness(percent)` | Updates the profile brightness and sends it to the device |
| `Delay(millis)` | Scheduling point between steps |

::: warning Current limitation
The editor schedules every `Delay` effect, so a multi action runs its steps with real pauses between them. The queue continues on a task, which keeps the interface responsive while the delay runs.
:::

## Hotkey

Presses a key combination in the focused window.

| Setting | Type | Default |
| --- | --- | --- |
| `hotkey` | text | `ctrl+alt+p` |

The parser accepts modifiers `ctrl`, `control`, `shift`, `alt`, `option`, `meta`, `win`, `super`, `cmd` and exactly one non modifier key, separated by `+`. A non modifier key is either a single character (`a`, `1`, `-`) or one of the named keys:

`space`, `enter`, `return`, `tab`, `escape`, `esc`, `backspace`, `delete`, `del`, `insert`, `ins`, `home`, `end`, `pageup`, `page_up`, `pagedown`, `page_down`, `up`, `arrowup`, `down`, `arrowdown`, `left`, `arrowleft`, `right`, `arrowright`, `printscreen`, `print_scr`, `f1` through `f12`.

Invalid combinations are reported before any key is pressed: an unknown token, more than one non modifier key, or no key at all.

```text
ctrl+shift+p
meta+space
f5
```

## Open

Opens an app, file, folder or URL with the user's default handler.

| Setting | Type | Default |
| --- | --- | --- |
| `target` | text | `https://elgato.com` |

A URL must include its scheme (`https://`). For a local path, use an absolute path; relative paths resolve against the process working directory, which is not predictable for a desktop app. An empty target is rejected.

## Text

Types a block of text into the focused window.

| Setting | Type | Default |
| --- | --- | --- |
| `text` | text | `Hello from MultiDeck` |

The text is typed character by character through the platform input backend, not pasted from the clipboard. Long text takes time and lands wherever focus is at the moment of each character.

## Media

Sends one media or volume key.

| Setting | Type | Default | Values |
| --- | --- | --- | --- |
| `media` | select | `play_pause` | `play_pause`, `next`, `previous`, `volume_up`, `volume_down`, `mute` |

Aliases `playpause` and `prev` are accepted when the value comes from an older profile. Unknown values are rejected.

## Command

Runs a shell command.

| Setting | Type | Default |
| --- | --- | --- |
| `command` | text | `echo MultiDeck` |

On Windows the command is passed to `cmd /C`, on other platforms to `sh -c`. The process is spawned and reaped in the background: output is not captured and the action does not wait for completion. Interactive commands and full screen programs should not be used. An empty command is rejected.

## Delay

Waits before the next step of a multi action.

| Setting | Type | Default | Range |
| --- | --- | --- | --- |
| `seconds` | number | `1` | 0 to 3600 |

The executor returns a `Delay` effect in milliseconds. Used alone the action does nothing visible, because there is no following step.

## Multi action

Runs several actions in sequence.

| Setting | Type | Default |
| --- | --- | --- |
| `steps` | JSON array | empty |

Each step has a `kind` and a `settings` object. Steps use the same settings schema as the action they call, including further multi actions. Steps without a `kind` are skipped.

```json
{
  "steps": [
    { "kind": "brightness", "settings": { "percent": "20" } },
    { "kind": "delay", "settings": { "seconds": "1" } },
    { "kind": "hotkey", "settings": { "hotkey": "ctrl+shift+m" } },
    { "kind": "brightness", "settings": { "percent": "80" } }
  ]
}
```

The inspector does not edit the step list yet, so multi actions are configured by editing the profile JSON. If a step fails, the whole multi action fails and the remaining steps do not run.

## Page

Jumps to another page of the active profile.

| Setting | Type | Default | Range |
| --- | --- | --- | --- |
| `page` | number | `1` | 1 or greater |

The stored number is one based. Internally the action emits `SwitchPage(page - 1)`, and a page index beyond the last page is ignored by the host. Pair this action with a key on every page that jumps back, otherwise a deck can end up on a page with no way out.

## Profile

Switches the active profile.

| Setting | Type | Default |
| --- | --- | --- |
| `profile` | text | empty |

The inspector lists every profile by name and stores its id. An empty value is rejected; a missing profile is reported as a failed load and the current profile stays active.

## Brightness

Sets the brightness of the active profile and of the device showing it.

| Setting | Type | Default | Range |
| --- | --- | --- | --- |
| `percent` | number | `80` | 0 to 100 |

Values outside the range are clamped. Brightness is stored in the profile, so the next time that profile loads the same level is restored.

## Validation errors

An action fails with one of these before or during execution:

| Error | Cause |
| --- | --- |
| Unknown kind | The profile references an action that is not registered in this build |
| Missing setting | A required text field is empty or absent |
| Invalid setting | A number cannot be parsed or a select value is unknown |
| Platform error | The operating system refused the keystroke, open or spawn |

Failures are shown as a status line and logged with the action kind. The key image and the profile are not changed by a failure.

Next: [language](/guide/language) explains how action labels and settings reach the interface.
