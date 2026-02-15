# Siemens TIA Import Tutorial (Direct `.scl` Export)

This tutorial shows the practical Siemens flow:

1. Export from truST as Siemens target.
2. Import generated `.scl` files into TIA Portal.
3. Generate blocks and finish manual mapping steps.

## Prerequisites

- `trust-runtime` built and available on PATH.
- A truST ST project with `src/`.
- TIA Portal project where blocks will be imported.

## Step 1: Export Siemens Target

Run export with Siemens target:

```bash
trust-runtime plcopen export --project <project-dir> --target siemens --json
```

Example:

```bash
trust-runtime plcopen export --project examples/siemens_scl_v1 --target siemens --json
```

## Step 2: Check Generated Artifacts

Siemens export writes:

- PLCopen XML: `interop/plcopen.siemens.xml`
- Source map: `interop/plcopen.siemens.xml.source-map.json`
- Adapter report: `interop/plcopen.siemens.xml.adapter-report.json`
- Siemens SCL bundle directory: `interop/plcopen.siemens.xml.scl/`
- Siemens SCL files: `interop/plcopen.siemens.xml.scl/*.scl`

Notes about generated SCL:

- `PROGRAM` declarations are exported as `ORGANIZATION_BLOCK "<name>"`.
- `FUNCTION`/`FUNCTION_BLOCK` are exported as SCL source blocks.
- `TYPE` declarations are exported in `000_types.scl`.
- Configuration/resource/task/program-instance wiring is migration metadata and still needs manual OB/task mapping in TIA.

## Step 3: Import `.scl` Files in TIA Portal

In TIA Portal:

1. Open your target TIA project.
2. In the project tree, locate your CPU and open `External source files`.
3. Right-click `External source files` -> `Add new external file...`.
4. Select all `.scl` files from:
   - `<project-dir>/interop/plcopen.siemens.xml.scl/`
5. Confirm import.

## Step 4: Generate Blocks from Imported Sources

1. In `External source files`, select imported `.scl` files.
2. Right-click -> `Generate blocks from source`.
3. Resolve any naming collisions if prompted.
4. Open generated blocks and run compile.

## Step 5: Apply Manual Mapping from Adapter Report

Open:

- `<project-dir>/interop/plcopen.siemens.xml.adapter-report.json`

Use `adapter_diagnostics[]` and `adapter_manual_steps[]` to finish:

- OB/task scheduling mapping.
- `%I/%Q/%M` address reconciliation.
- Retain/commissioning behavior checks.
- Vendor-library semantic review beyond symbol-level mapping.

## Troubleshooting

- No `.scl` files generated:
  - Ensure the project has importable ST declarations (`PROGRAM`, `FUNCTION`, `FUNCTION_BLOCK`, `TYPE`).
- TIA import accepted source but block generation failed:
  - Open the failing source in TIA error view and fix syntax/runtime-model differences.
- Task/program wiring missing after block generation:
  - Expected. Apply manual mapping using the adapter report checklist.

## Scope Reminder

This flow is for direct source import convenience. It does not generate native TIA project archives and does not auto-generate hardware topology or safety metadata.
