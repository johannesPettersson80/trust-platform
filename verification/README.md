# Verification Control Plane

Status: initial seed for the PLC verification program.

This directory holds machine-readable verification metadata. It does not move
or replace executable tests. Native tests stay in `crates/*/tests`,
`crates/*/src`, `editors/vscode/src/test`, `conformance`, `fuzz`, and gate
scripts.

Current seed scope:

- source-build/OpenOT public-build truth for GitHub issue #93,
- bytecode/VM specification sources,
- runtime safety specification sources,
- first public-docs claim scan with explicit gaps.

TOML shape convention:

- invariants are one record per file under
  `verification/invariants/<area>/<INVARIANT_ID>.toml`;
- flat registries use plural wrapper arrays, for example `[[spec_sources]]`,
  `[[spec_gaps]]`, and `[[evidence]]`;
- committed case tables will live under `verification/cases/<area>/**` and are
  referenced from test-catalog rows by path plus SHA-256 digest.
- `verification/spec-matrix.toml` will define required specification tags per
  canonical area. A tag is satisfied by an active spec source whose `covers`
  list includes the tag, or by an open spec gap with an owner.

Do not mark records `validated` until the validators, suite definitions, and
evidence index checks described in
`docs/internal/testing/checklists/plc-verification-program/metadata-model.md`
exist and pass.
