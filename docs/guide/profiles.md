# Profiles and pages

A profile is a named layout. Each profile holds one or more pages, and each page holds one slot per key. Only one page is visible on a device at a time.

```text
Profile
  brightness           0 to 100, applied to the device
  pages[]              ordered list, at least one
    slots[]            one per key, row major
      action           optional action instance
      title_override   optional label
  active_page          index of the visible page
```

## Key order

Slots are stored in row major order. For a device with `cols` columns, the index of the key at column `c` and row `r` is:

```text
index = r * cols + c
```

On a 5x3 deck the top row is keys 0 to 4, the middle row is 5 to 9, and the bottom row is 10 to 14. This index is what device adapters use in `SetKeyImage` and what profiles use in `slots`.

## Pages

Pages exist to multiply the key count. A 5x3 deck with three pages offers 45 slots while only showing 15 at a time.

- **New page** appends a page to the profile and switches to it.
- The page switcher changes `active_page` and redraws the deck.
- The **Page** action jumps to a page from a key press. It stores a one based page number; page 1 is the first page.
- Pages are resized when the active device changes. Growing adds empty slots, shrinking truncates the tail, and the active index is clamped.

## Brightness

Brightness belongs to the profile, not to the device, so switching profiles can change how bright the deck is. The value is a percentage from 0 to 100. It is sent as `SetBrightness` when the profile loads and when the user drags the slider. The brightness action can also change it from a key.

## On disk

Profiles are JSON files in `<data dir>/profiles/`, one file per profile, named after the profile id with unsafe characters replaced by underscores. The schema is versioned so a future change can migrate old files.

```json
{
  "schema": 1,
  "id": "6a1f95d2-3c4e-4a1e-9a8e-3d5a0f2b7c11",
  "name": "Default",
  "brightness": 80,
  "pages": [
    {
      "id": "0f4c1a2e-5b6d-4c3a-9e8f-1a2b3c4d5e6f",
      "name": "Main",
      "slots": [
        {
          "action": {
            "id": "c1e2d3f4-a5b6-4c7d-8e9f-0a1b2c3d4e5f",
            "kind": "hotkey",
            "settings": { "hotkey": "ctrl+alt+p" },
            "state": 0
          }
        },
        {
          "action": {
            "id": "d2f3e4a5-b6c7-4d8e-9f0a-1b2c3d4e5f6a",
            "kind": "open",
            "settings": { "target": "https://github.com/callmeteus/multideck" },
            "state": 0
          },
          "title_override": "Repository"
        },
        {}
      ]
    }
  ],
  "active_page": 0
}
```

An empty slot serializes as `{}` because absent fields are omitted. `state` holds the internal toggle counter used by multi actions; leave it at `0` when writing a file by hand.

## Validation rules

A profile is rejected when:

- the JSON is malformed;
- `pages` is empty.

Reading a profile stamps `schema` with the current version. Invalid profiles found while listing are skipped with a warning and left on disk, so a broken file never blocks startup. If the profile recorded as active cannot be loaded, a new `Default` profile is created and becomes active.

## Editing by hand

The interface is the intended editor, but the format is plain JSON and stable enough to generate. Two practical uses:

- **Template profiles**: keep a known good file and copy it into `profiles/` with a new `id`.
- **Bulk changes**: edit `settings` with a script, for example to point every open action to a new URL.

Keep ids unique. Two files with the same `id` will produce two entries in the profile list and the active profile will resolve to whichever file matches the id.

## Backups

Back up the whole data directory, not just `profiles/`. `settings.toml` records which profile is active, and `assets/` may hold files referenced by actions. Copying the directory while the app is closed is enough; there is no database and no lock file.

Next: [actions](/guide/actions) documents every action and its settings.


## Placing and removing actions

Drag an action from the palette on the right onto a key. Releasing over a key assigns the action. To remove one, select the key and press Delete; the editor always asks for confirmation before removing it.
