# trust-twin component library v1

This directory is the publication location for the built-in trust-twin
component library. `trust-twin-compiler` embeds `components.toml` with
`include_str!`, so normal workspace checkouts and packaged crate artifacts use
the same definitions without workspace-relative path guesses.

The v1 format is a single TOML file with:

- `version` and `grid` defaults.
- `[[kind]]` component definitions.
- `[[kind.ports]]` named public ports with domain, direction, size, origin, and
  axis metadata.
- `[[kind.signals]]` signal vocabulary with target `bind3d` property metadata
  and optional primitive visual node metadata for compiled `.view.toml` output.
