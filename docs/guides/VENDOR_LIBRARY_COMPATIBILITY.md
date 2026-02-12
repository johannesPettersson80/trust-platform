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
