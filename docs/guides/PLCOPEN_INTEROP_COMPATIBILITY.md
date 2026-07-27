# PLCopen Interop Compatibility (Deliverable 5)

This document defines the current PLCopen XML interoperability contract for
`trust-runtime plcopen` after Deliverable 7 (ST-complete coverage + v1
multi-vendor export adapters).

## Scope

- Namespace: `http://www.plcopen.org/xml/tc6_0200`
- Profile: `trust-st-complete-v1`
- Command surface:
  - `trust-runtime plcopen profile [--json]`
  - `trust-runtime plcopen export [--project <dir>] [--output <file>] [--target <generic|ab|siemens|schneider>] [--json]`
  - `trust-runtime plcopen import --input <file> [--project <dir>] [--json]`
- Product decision for this phase:
  - ST-only PLCopen project support.
  - FBD/LD/SFC graphical network bodies are out of scope and are rejected
    before any source file is emitted.

## ST-complete Profile Contract

The public profile identity is the TC6 namespace
`http://www.plcopen.org/xml/tc6_0200` with profile name
`trust-st-complete-v1`. Both the library profile and
`trust-runtime plcopen profile --json` expose that identity, at least one
supported compatibility row, and nonempty round-trip-limit and known-gap
lists. The library profile additionally publishes the ST POU path in its
strict subset and a partial compatibility-shim row. These fields describe the
selected product profile; they do not claim complete TC6 interoperability.

## ST-complete Interchange Contract

For the supported subset, import and export preserve the tested POU signature
triples of name, POU kind, and normalized ST body across
`export -> import -> export`. A data-type-only document is also a successful
import: supported elementary, enumeration, subrange, structure, and
single-dimension array declarations are emitted together in
`src/plcopen_data_types.st`.

The current data-type-only test does not establish the migration report's
semantic-loss percentage or compatibility verdict. Those report values remain
outside this contract until their zero-POU scoring policy is decided and
tested.

## ST-only Import Profile

A supported ST POU body is emitted as ST source under `src/`. Benign
`addData` beside or inside that body does not replace it and is not classified
as an unknown executable body. Supported and unsupported sibling POUs are
handled independently.

## Import Failure Contract

Malformed XML returns an import error. The CLI also exits unsuccessfully for a
missing input path and reports that the input does not exist. Exact error
types, stable exit-code numbers, and proof that every failure leaves the
destination untouched remain explicit gaps.

## Non-ST Body Diagnostics

FBD, LD, SFC, and unknown executable bodies are skipped with error diagnostics
`PLCO215`, `PLCO216`, `PLCO217`, and `PLCO218`, respectively. A document
containing only one POU of each kind reports four discovered and four skipped
POUs, imports none, emits no ST source, and returns the reviewed
`no importable PLCopen ST content` failure.

## Compatibility Matrix

| Capability | Status | Notes |
|---|---|---|
| ST POU import/export (`PROGRAM`, `FUNCTION`, `FUNCTION_BLOCK`) | supported | Includes common aliases (`PRG`, `FC`, `FUN`, `FB`). Import also reconstructs ordered TC6 body worksheets and standard interface var sections, including `globalVars` and `accessVars`. |
| ST `types/dataTypes` import (`elementary`, `derived`, `array`, `struct`, `enum`, `subrange`) | supported | Imported into generated ST `TYPE` declarations under `src/`. |
| ST `TYPE` export to `types/dataTypes` | partial | Supported ST declarations are emitted; unsupported forms are skipped with warnings. |
| `instances/configurations/resources/tasks/program instances` import/export | supported | Deterministic ST mapping with name normalization and structured diagnostics. |
| CODESYS `addData/globalVars` import/export | supported | Import prefers `interfaceasplaintext` for pragma fidelity and falls back to `<variable>` synthesis. Default import now materializes native truST vendor-parity globals (file-scope GVLs and namespaced GVLs for `qualified_only` lists) without mandatory `VAR_EXTERNAL` injection. Adapter callers can still request strict IEC reshaping through `PlcopenImportGlobalVarMode::StrictIecAdapter`, which restores the older wrapper + injected-`VAR_EXTERNAL` flow for external consumers that require it. Export emits deterministic CODESYS `globalVars` metadata. |
| CODESYS `addData/method` on `FUNCTION_BLOCK` POUs | supported | Import reconstructs ST `METHOD` members from vendor metadata and export emits deterministic `Method` objects/object IDs on the owning POU and in `projectstructure`. Other vendor OOP objects such as properties remain out of scope. |
| CODESYS `addData/projectstructure` folder mapping import/export | partial | Import/export mirrors deterministic folder placement for POUs/GVLs; library/device-tree semantics remain metadata-only. |
| Source map metadata (`trust.sourceMap`) | supported | Embedded `addData` payload + sidecar `*.source-map.json`. |
| Vendor extension preservation (`addData`) | partial | Preserved/re-injectable, but not semantically interpreted. |
| Vendor ecosystem migration heuristics | partial | Advisory signal only; not semantic equivalence. |
| Vendor library shim normalization | partial | Selected aliases are mapped to IEC FB names during import; each mapping is reported. |
| Multi-vendor export adapters (`--target ab|siemens|schneider`) | partial | Exports PLCopen XML + target diagnostics/manual-step report; Siemens target also emits direct `.scl` source bundle; native vendor project package generation is out of scope in v1. |
| Graphical bodies (FBD/LD/SFC) | unsupported | ST-complete contract remains ST-only by product decision. Import rejects these bodies with named diagnostics instead of scraping XML text into ST. |
| Vendor AOI/library internal semantics | unsupported | Advanced behavior remains manual migration work beyond symbol-level shims. |

## Export Adapter Contract (Deliverable 7)

When `plcopen export` runs with `--target ab|siemens|schneider`, the export report
includes target-specific adapter fields:

- `target`
- `adapter_report_path`
- `siemens_scl_bundle_dir` (Siemens target only)
- `siemens_scl_files[]` (Siemens target only)
- `adapter_diagnostics[]`:
  - `code`
  - `severity`
  - `message`
  - `action`
- `adapter_manual_steps[]`
- `adapter_limitations[]`

The command also writes:

- target-specific default XML path:
  - `interop/plcopen.ab.xml`
  - `interop/plcopen.siemens.xml`
  - `interop/plcopen.schneider.xml`
- Siemens-target direct source bundle:
  - `<output-file>.scl/*.scl`
- sidecar adapter report:
  - `<output-file>.adapter-report.json`

Adapter diagnostics are migration guidance only; they are not proof of semantic
equivalence on target runtimes.

## Export Adapter Artifact Contract

An Allen-Bradley-targeted library export records target `allen-bradley`, writes
the requested XML plus `<xml>.adapter-report.json`, embeds adapter metadata,
reports `PLCO7AB1`, and exposes nonempty manual-step and limitation lists.
A Siemens-targeted library export writes `<xml>.scl/`, lists each generated
SCL artifact, renders the tested `Main` program as an
`ORGANIZATION_BLOCK`, and writes the matching adapter report.

When the CLI omits `--output`, target `ab` writes
`interop/plcopen.ab.xml` and a real adapter report whose path is returned in
JSON. These artifacts are migration aids, not native vendor project packages
or proof of target-runtime equivalence.

## Migration Report and Vendor Extension Contract

`plcopen import` writes `interop/plcopen-migration-report.json` with:

- Coverage metrics:
  - `discovered_pous`
  - `imported_pous`
  - `skipped_pous`
  - `imported_data_types`
  - `discovered_configurations`
  - `imported_configurations`
  - `imported_resources`
  - `imported_tasks`
  - `imported_program_instances`
  - `discovered_global_var_lists`
  - `imported_global_var_lists`
  - `imported_project_structure_nodes`
  - `imported_folder_paths`
  - `source_coverage_percent`
  - `semantic_loss_percent`
  - `compatibility_coverage`:
    - `supported_items`
    - `partial_items`
    - `unsupported_items`
    - `support_percent`
    - `verdict` (`full` | `partial` | `low` | `none`)
- Structured diagnostics (`unsupported_diagnostics`) with:
  - `code`
  - `severity`
  - `node`
  - `message`
  - optional `pou`
  - `action`
- Applied shim summary (`applied_library_shims`) with:
  - `vendor`
  - `source_symbol`
  - `replacement_symbol`
  - `occurrences`
  - `notes`
- Per-POU migration entries (`entries`) with `status` and `reason`.

Non-ST POU body diagnostics are fail-closed:

- `PLCO215`: unsupported FBD graphical body.
- `PLCO216`: unsupported LD graphical body.
- `PLCO217`: unsupported SFC graphical body.
- `PLCO218`: unknown non-ST body element.

When one of these diagnostics is emitted, the POU is skipped and no ST source is
synthesized from the graphical or unknown XML payload.

For a supported ST POU accompanied by unsupported type metadata and opaque
vendor `addData`, import reports the unsupported type, returns partial
compatibility, writes the ST source, and preserves the opaque vendor payload in
`plcopen.vendor-extensions.imported.xml`. Export re-injects the reviewed
`plcopen.vendor-extensions.xml` hook into PLCopen `addData`. The generic CLI
JSON report exposes the tested export counts/source-map path and import
ecosystem, compatibility verdict, and diagnostic code.

## CODESYS Metadata Contract

Export emits deterministic CODESYS `globalVars`, `projectstructure`, POU
`addData`, interface-as-plain-text, object-ID, and `METHOD` metadata for the
tested supported declarations. Importing that exported method metadata
reconstructs the owning `FUNCTION_BLOCK`, method signature, inputs, and tested
method body. The contract does not extend to CODESYS properties, device trees,
or executable equivalence inside CODESYS.

## Vendor Library Shim Contract

For recognized ecosystems, import replaces only the reviewed symbol positions
with the catalogued alias and records the ecosystem, source symbol,
replacement symbol, occurrence count, and `PLCO301` migration diagnostic.
The Siemens fixture maps `SFB3 -> TP` and `SFB4 -> TON` while preserving an
unrelated variable named `SFB4`. The OpenPLC CLI fixtures detect `openplc`,
map `R_EDGE -> R_TRIG`, and remain exportable with the source-map sidecar.
These are symbol-level migration transforms, not semantic-equivalence claims.

## CODESYS ST Fixture Pack and Parity Gate

Deliverable 5 includes deterministic CODESYS ST fixture packs for
`small`, `medium`, and `large` project shapes:

- XML fixtures:
  - `crates/trust-runtime/tests/fixtures/plcopen/codesys_st_complete/small.xml`
  - `crates/trust-runtime/tests/fixtures/plcopen/codesys_st_complete/medium.xml`
  - `crates/trust-runtime/tests/fixtures/plcopen/codesys_st_complete/large.xml`
- Expected migration artifacts:
  - `crates/trust-runtime/tests/fixtures/plcopen/codesys_st_complete/*.expected-migration.json`
- CI parity regression gate:
  - `crates/trust-runtime/tests/plcopen_st_complete_parity.rs`

The parity test enforces deterministic import/export signature stability for
supported ST-project structures and fails CI on schema-drift regressions.

The vendor-family PLCopen fixtures in
`crates/trust-runtime/tests/fixtures/plcopen/synthetic-*.xml` are synthetic,
IP-clean regression fixtures. They are not real exports from vendor tools.
Real CODESYS and TwinCAT export fixtures must be generated from scratch demo
projects and recorded with provenance before they are added to this corpus.

## Supported Ecosystem Detection (Advisory)

Detected values currently include:

- `codesys`
- `beckhoff-twincat`
- `siemens-tia`
- `rockwell-studio5000`
- `schneider-ecostruxure`
- `mitsubishi-gxworks3`
- `openplc`
- fallback: `generic-plcopen`

## Round-Trip Limits

Round-trip means `import -> export -> import -> export` through the
ST-complete contract.

Guaranteed for supported ST-project structures:

- ST POU signature-level stability.
- Ordered ST worksheet import for supported multi-`body` POU inputs.
- Supported `dataTypes` signature stability.
- Supported configuration/resource/task/program-instance wiring intent stability.
- Supported CODESYS globalVars declaration stability.
- Supported CODESYS `FUNCTION_BLOCK` method metadata stability.
- Supported CODESYS folder-placement intent stability for deterministic projectstructure trees.
- Deterministic fallback insertion for missing FUNCTION result assignments (`<FuncName> := <FuncName>;`) during import synthesis.
- Stable source-map sidecar contract.

Not guaranteed:

- Original vendor formatting/layout in XML payloads.
- Preservation of graphical network semantics.
- Import of runtime deployment/safety metadata.
- Exact source file names (imports use sanitized unique names under `src/`).

## Known Gaps

- No semantic import/export for SFC/LD/FBD bodies.
- Export-side `dataTypes` remains subset-based for supported ST `TYPE` forms; unsupported ST type syntax is skipped with warnings.
- Vendor library shim coverage is intentionally limited to the baseline alias catalog.
- No semantic translation for vendor-specific AOI/FB internals and pragmas.
- Vendor OOP objects beyond CODESYS `METHOD` members, such as vendor `PROPERTY` metadata, are not yet semantically imported/exported.
- Vendor extension nodes are preserved as opaque metadata, not executed.
- Core truST does not yet enforce CODESYS `{attribute 'qualified_only'}` as a semantic restriction; qualified access works, but bare global access is still accepted by the vendor-parity language/runtime path.
- Export adapters do not generate native vendor package formats (`.L5X`, TIA project archives, EcoStruxure project archives).

## Example Project

A complete import/export walkthrough project is available in:

- `examples/plcopen_xml_st_complete/`
