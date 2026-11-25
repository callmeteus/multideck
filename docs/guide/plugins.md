# Plugins

MultiDeck can load extra actions from YAML files, no rebuild needed.

## Where plugins live

Put `.yml` or `.yaml` files in:

- Windows: `%APPDATA%\multideck\plugins`
- Linux: `$XDG_CONFIG_HOME/multideck/plugins`, or `~/.config/multideck/plugins`
- Anywhere: point `MULTIDECK_PLUGINS` at a folder before starting the app

The app reads the folder once at startup. Restart MultiDeck after changing a
plugin.

## Format

```yaml
name: My tools
actions:
  - id: my-tool
    title: My tool
    description: Runs my tool with the selected mode
    icon: command
    tags: [tool, cli]
    command: my-tool
    args: ["--mode", "{mode}"]
    fields:
      - key: mode
        label: Mode
        default: fast
        kind: select
        options: [fast, full]
```

- `id` is the action kind stored in the profile. Use a unique value.
- `title` and `description` are shown in the palette and the inspector.
- `icon` is one of the built-in icon names (see below). Unknown names fall
  back to `command`.
- `tags` are extra words the palette search matches.
- `command` is the executable, and `args` are its arguments. Every `{field}`
  placeholder is replaced by that field's value before the command runs
  through the shell.
- `fields` become the inspector controls:
  - `kind: text` (default) or `kind: path` shows a text box.
  - `kind: number` shows a number box.
  - `kind: select` shows a picker built from `options`.

## Icons

Names come from `multideck-render`: `brightness`, `command`, `delay`,
`folder`, `hotkey`, `media`, `multi`, `open`, `profile`, `text`.

## Search

The palette search box matches the translated title, the description, the
kind, and every tag, in any language. Tags such as `cli`, `hex`, `voz` or
`midia` make actions easier to find.

## Tips

- Quote arguments that contain spaces: `args: ["--title", "Hello world"]`.
- Remember that the command runs in a shell, so pipes and redirection work:
  `command: "echo hello > %TEMP%/hello.txt"`.
- A plugin action with a duplicate `id` is ignored, and the built-in action
  wins.
