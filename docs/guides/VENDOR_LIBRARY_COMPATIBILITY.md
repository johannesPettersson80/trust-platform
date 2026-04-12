# Vendor Library Compatibility Baseline (Deliverable 4)

This guide defines the v1 compatibility baseline for vendor library migration
through `trust-runtime plcopen import`.

## Scope

- Interchange entrypoint: PLCopen XML ST-complete import (`trust-runtime plcopen import`).
- Goal: normalize selected high-demand vendor aliases into IEC FB names where
  semantic intent is clear.
- Non-goal: claim full semantic equivalence for proprietary AOI/library internals.

## Compatibility Matrix

| Capability | Status | Notes |
|---|---|---|
| IEC standard FB names (`TON`, `TOF`, `TP`, `R_TRIG`, `F_TRIG`) | supported | Native runtime/LSP support. |
| Mitsubishi edge aliases (`DIFU`, `DIFD`) in authored ST | supported | Native runtime/LSP support maps to IEC edge behavior. |
| Vendor alias shims for selected timer/edge blocks | partial | Import rewrites known aliases and records each applied shim in migration reports. |
| Vendor library package/AOI indexing hints via ecosystem detection | partial | Detection is advisory and tied to migration diagnostics, not full semantic import. |
| Vendor AOI internals, safety annotations, proprietary pragmas | unsupported | Requires manual migration and validation. |

## Shim Catalog (v1)

The following symbol-level mappings are applied during PLCopen import when the
corresponding ecosystem is detected.

| Ecosystem | Source Symbol | Replacement Symbol | Notes |
|---|---|---|---|
| `siemens-tia` | `SFB3` | `TP` | Siemens pulse timer alias mapped to IEC TP. |
| `siemens-tia` | `SFB4` | `TON` | Siemens on-delay timer alias mapped to IEC TON. |
| `siemens-tia` | `SFB5` | `TOF` | Siemens off-delay timer alias mapped to IEC TOF. |
| `rockwell-studio5000` | `TONR` | `TON` | Retentive behavior may differ; review manually. |
| `schneider-ecostruxure` / `codesys` | `R_EDGE` | `R_TRIG` | Edge alias mapped to IEC R_TRIG. |
| `schneider-ecostruxure` / `codesys` | `F_EDGE` | `F_TRIG` | Edge alias mapped to IEC F_TRIG. |
| `mitsubishi-gxworks3` | `DIFU` | `R_TRIG` | Differential-up alias mapped to IEC R_TRIG. |
| `mitsubishi-gxworks3` | `DIFD` | `F_TRIG` | Differential-down alias mapped to IEC F_TRIG. |

## Report Contract

`interop/plcopen-migration-report.json` and `plcopen import --json` include:

- `applied_library_shims[]` entries with:
  - `vendor`
  - `source_symbol`
  - `replacement_symbol`
  - `occurrences`
  - `notes`
- `unsupported_diagnostics[]` entries with code `PLCO301` for each shimmed POU.

## Operational Guidance

- Treat shimmed imports as migration assists, not proof of semantic equivalence.
- Run conformance and project-level runtime tests after import.
- If a required vendor block is missing from this catalog, add a fixture, define
  a deterministic mapping contract, and extend this guide before claiming support.

## User-Provided Library Definitions (Recommended)

When your project uses vendor blocks that are not in the shim catalog, the
recommended path is to provide local ST library stubs and index them in
`trust-lsp.toml`.

This gives deterministic editor semantics while you incrementally migrate vendor
code.

Use this `[[libraries]]` path for stub/index packs. For reusable truST
Structured Text packages that should participate in normal project compilation
and runtime builds, use `[dependencies]` instead.

### 1) Create a stub library folder

Example layout:

```text
my-project/
  src/
  vendor/
    siemens/
      tcon.st
      norm_scale.st
  trust-lsp.toml
```

### 2) Add `trust-lsp.toml` library entries

```toml
[project]
include_paths = ["src"]

[[libraries]]
name = "siemens-stubs"
path = "vendor/siemens"
version = "0.1.0"
```

Multiple vendor packs can be added with additional `[[libraries]]` entries.

### 3) Write stub declarations

Use interface-style ST declarations for missing vendor symbols. Minimal method
or body logic is enough; semantic behavior is not required for indexing.

Example:

```st
FUNCTION_BLOCK TCON
VAR_INPUT
    REQ : BOOL;
END_VAR
VAR_OUTPUT
    DONE : BOOL;
    ERROR : BOOL;
    STATUS : WORD;
END_VAR
END_FUNCTION_BLOCK
```

### 4) Reload workspace and verify

In VS Code:

1. Reload window (`Developer: Reload Window`).
2. Open project ST files using vendor symbols.
3. Verify diagnostics clear for declared symbols.
4. Verify completion/hover/go-to-definition resolve into your stub files.

### What Works With Stub Libraries

- Symbol resolution and missing-symbol diagnostics suppression for declared items.
- Completion, hover, and go-to-definition for stubbed FB/FUNCTION/TYPE symbols.
- Cross-file navigation and reference indexing for the provided declarations.

### What Stub Libraries Do Not Provide

- Runtime-equivalent vendor behavior.
- Automatic migration of proprietary AOI internals or safety metadata.
- Verified semantic equivalence without project-specific validation/testing.

Reference tutorial/example:

- `examples/vendor_library_stubs/`
