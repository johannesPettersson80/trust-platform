# Mitsubishi GX Works3 Compatibility v1 (Deliverable 8)

This document defines the Mitsubishi GX Works3 v1 compatibility baseline for
`trust-syntax`, `trust-hir`, `trust-ide`, `trust-lsp`, and `trust-runtime`.

## Scope

- Vendor profile: `vendor_profile = "mitsubishi"` (alias: `gxworks3`)
- Focus: prioritized GX Works3 ST authoring constructs with deterministic
  diagnostics/formatting behavior
- Example project: `examples/mitsubishi_gxworks3_v1/`

## Supported GX Works3 Subset (v1)

### 1) Mitsubishi edge aliases (`DIFU` / `DIFD`)

truST accepts Mitsubishi edge-detection aliases as built-in function blocks:

- `DIFU(CLK := ..., Q => ...)` mapped to IEC `R_TRIG` runtime behavior
- `DIFD(CLK := ..., Q => ...)` mapped to IEC `F_TRIG` runtime behavior

This works in regular ST authoring and runtime execution, not only PLCopen
import shim flows.

### 2) Mitsubishi formatting defaults

With `vendor_profile = "mitsubishi"` (or `"gxworks3"`) formatter defaults are:

- 4-space indent
- uppercase keywords
- spaced operators/assignments
- aligned `END_*` keywords

### 3) Mitsubishi diagnostic defaults

With `vendor_profile = "mitsubishi"` diagnostic defaults keep all warning
categories enabled unless overridden in `[diagnostics]`.

Optional safety rule-pack aliases are supported:

- `mitsubishi-safety`
- `gxworks3-safety`

Both map to the same safety baseline behavior as other safety packs.

## Known Gaps / Incompatibilities

- This is ST language/tooling compatibility, not full GX Works3 project-packaging
  parity.
- Device-parameter project metadata and hardware topology mapping remain manual
  migration work.
- Vendor-specific project settings outside ST source semantics are out of scope
  in v1.

## Migration Guidance

1. Set `vendor_profile = "mitsubishi"` in `trust-lsp.toml`.
2. Keep edge calls in IEC-compatible shape (`CLK` input, `Q` output) for stable
   cross-tooling behavior.
3. For PLCopen imports, keep using migration report diagnostics as the source of
   truth for shimmed/unsupported items.

## Regression Coverage

- HIR semantic coverage:
  - `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs`
- Runtime alias behavior coverage:
  - `crates/trust-runtime/tests/fb_edges.rs`
- LSP profile/diagnostic coverage:
  - `crates/trust-lsp/src/handlers/tests/formatting_and_navigation.rs`
  - `crates/trust-lsp/src/handlers/tests/core.rs`
- Runtime/example compile coverage:
  - `crates/trust-runtime/tests/tutorial_examples.rs`
