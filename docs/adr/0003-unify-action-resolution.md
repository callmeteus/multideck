# Resolve built-in and plugin actions in one place

## Status

Accepted

## Context

The palette, inspector and executor used separate lookup paths. `definition()` preferred a built-in action, while `execute()` preferred a plugin. A plugin with the same kind as a built-in could therefore appear as the built-in but run plugin code.

## Decision

Use one resolver for `definitions()`, `definition()` and `execute()`. The resolver checks compile-time built-ins first, then YAML plugins. The public function signatures stay unchanged.

## Consequences

The palette, inspector and executor always agree on the selected action. Built-in actions keep priority over duplicate plugin kinds. Plugin loading remains cached for the process lifetime.
