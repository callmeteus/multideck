# MultiDeck glossary

MultiDeck stores local layouts and runs actions on virtual, USB and mobile decks.

## Language

**Profile**:
A named layout with one or more pages and a brightness level.
_Avoid_: Layout, workspace

**Page**:
One visible screen of key slots inside a profile.
_Avoid_: Screen, layer

**Key slot**:
The saved position for one key on a page. It can hold an action and a title override.
_Avoid_: Button, tile

**Action**:
One operation assigned to a key, with its own settings and validation.
_Avoid_: Macro, command when the operation is not a shell command

**Effect**:
A result returned by an action that only the host can apply, such as switching a page or changing brightness.
_Avoid_: Side effect for host-owned changes

**Device**:
A virtual, USB or mobile deck that receives images and commands and reports input.
_Avoid_: Adapter, hardware when the virtual deck is included

**Pairing**:
The host-approved trust relationship between MultiDeck and a mobile phone.
_Avoid_: Login, connection

**Plugin**:
A YAML file that adds command actions without rebuilding MultiDeck.
_Avoid_: Extension, add-on
