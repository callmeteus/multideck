# Language

The interface ships in English (United States) and Brazilian Portuguese. Translations are Fluent files compiled into the binary, so a missing key or a syntax error is a build failure rather than a runtime surprise.

## Current languages

| Tag | Name in the interface | File |
| --- | --- | --- |
| `en-US` | English | `locales/en-US/main.ftl` |
| `pt-BR` | Português (Brasil) | `locales/pt-BR/main.ftl` |

English is the fallback. If a key is missing in the selected language, the English text is used; if it is missing there too, the key itself is shown. This makes a missing translation visible during development without breaking the window.

## Detection and override

When `settings.toml` has an empty `locale`, the app asks the operating system for its locale through `sys-locale`. Any tag that starts with `pt` selects Portuguese; everything else selects English. That rule is intentionally simple, since there is no partial translation to distinguish.

Changing the language in Settings writes the tag to `settings.toml` and rebuilds the affected labels immediately. No restart is required.

## How strings are used

| Place | Key source |
| --- | --- |
| Action name | `action.<kind>` |
| Action description | `action.<kind>_desc` |
| Default key title | `action.<kind>_title` |
| Inspector field label | `field.<setting>` |
| Status line | `status.*` |
| Navigation and panels | `nav.*`, `editor.*`, `inspector.*` |

Actions declare translation keys, never literal text, so a new language only needs the FTL file. The fallback title is used when the user has not set a key label.

## Adding a language

1. Add a variant to `Locale` in `packages/libs/i18n/src/lib.rs` with its tag, Fluent language id, native name and parse rule.
2. Create `locales/<tag>/main.ftl` with every key used by `en-US`.
3. Add the language to the selector in Settings.
4. Run the tests. `locales_have_the_same_keys` fails when the key sets diverge, which is the guard against half translated interfaces.

The loader picks the directory up at compile time, so there is no runtime file lookup and no path configuration to change.

## Writing strings

- Use the same key names as English, never translate keys.
- Keep placeholders in the same positions, for example `Page { $number }`.
- Use correct accents and punctuation in Portuguese.
- Prefer short labels: key titles are rendered at small sizes and are truncated.

## Documentation language

The application is bilingual. The documentation, code comments, commit messages and README are English only, so a contributor reads one language for the project and two for the interface.

Next: [architecture](/architecture) explains where the translation crate sits in the workspace.
