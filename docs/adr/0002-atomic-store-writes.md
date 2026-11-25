# Atomically replace settings and profile files

## Status

Accepted

## Context

Settings and profiles were written directly to their final paths. A failed write could leave a truncated TOML or JSON file. `Store::update_settings` also changed memory before the disk write succeeded, so memory and disk could diverge.

## Decision

Write each file to a `NamedTempFile` in the destination directory, flush it, and persist it over the final path. For settings changes, mutate a candidate copy and assign it to `Store.settings` only after the file replacement succeeds.

## Consequences

A crash or failed write leaves the previous complete file in place. Each save now performs a sync and a rename. The TOML and JSON schemas do not change.
