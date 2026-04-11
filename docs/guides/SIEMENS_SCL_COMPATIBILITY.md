# Siemens SCL Compatibility (Deliverable 3)

This document defines the Siemens SCL v1 compatibility baseline for `trust-syntax`,
`trust-hir`, `trust-ide`, and `trust-lsp`.

## Scope

- Vendor profile: `vendor_profile = "siemens"`
- Focus: syntax, formatting, and diagnostics compatibility improvements for common
  Siemens SCL authoring patterns
- Example project: `examples/siemens_scl_v1/`

## Supported SCL Subset (v1)

### 1) `#`-prefixed local references

The parser accepts Siemens-style `#` prefixes for local and instance references in:

- expressions (`#input + 1`)
- assignment/call statements (`#counter := #counter + 1;`, `#fb(Enable := #pulse);`)
- `FOR` loop control variables (`FOR #i := 0 TO 3 DO ... END_FOR;`)

Malformed `#` usage reports a targeted parse diagnostic:

- `expected identifier after '#'

### 2) Siemens formatting defaults

With `vendor_profile = "siemens"` the formatter defaults to:

- 2-space indent
- uppercase keywords
- compact operator spacing
- aligned `END_*` keywords

### 3) Siemens diagnostic defaults

With `vendor_profile = "siemens"` default warning behavior is:

- `W004` (missing ELSE): disabled
- `W005` (implicit conversion): disabled
- other warning categories remain enabled unless overridden in config

## Known Gaps / Deviations

- This is language/tooling compatibility, not full TIA project parity.
- Siemens project metadata and hardware configuration semantics are out of scope.
- Siemens-specific pragmas/attributes are parsed as pragmas (trivia), not executed semantics.

## Related Runtime Export Path

For direct Siemens source handoff to TIA via external source files:

- `trust-runtime plcopen export --target siemens` emits a `.scl` bundle sidecar.
- Tutorial: `docs/guides/SIEMENS_TIA_SCL_IMPORT_TUTORIAL.md`

See `docs/IEC_DEVIATIONS.md` for the formal deviation record.

## Regression Coverage

- Parser coverage:
  - `crates/trust-syntax/tests/parser_expressions.rs`
  - `crates/trust-syntax/tests/parser_statements.rs`
  - `crates/trust-syntax/tests/parser_error_recovery.rs`
- LSP coverage:
  - `crates/trust-lsp/src/handlers/tests/formatting_and_navigation.rs`
  - `crates/trust-lsp/src/handlers/tests/core.rs`
- Runtime/example compile coverage:
  - `crates/trust-runtime/tests/tutorial_examples.rs`
