# Siemens SCL v1: Vendor Profile Tutorial

Docs category: `docs/public/examples/vendor-profiles.md`

This tutorial demonstrates Siemens profile behavior in VS Code and shows how to
compare it against a generic profile.

## What You Learn

- `#`-prefixed identifier support
- Siemens profile formatting/diagnostics behavior
- Hover/completion resolution for `#` symbols
- Runtime/debug launch from this example project
- Direct Siemens `.scl` export for TIA External source import

## Files

- `src/Main.st`
- `src/Configuration.st`
- `trust-lsp.toml`
- `.vscode/launch.json`

## Step 1: Open + Build

```bash
code examples/siemens_scl_v1
trust-runtime build --project examples/siemens_scl_v1 --sources src
trust-runtime validate --project examples/siemens_scl_v1
```

## Step 2: Completion and Hover with `#` Prefix

1. Open `src/Main.st`.
2. Type `#` and trigger completion (`Ctrl+Space`).
3. Hover `#Counter` and `#Total` to inspect resolved types.
4. Confirm no unexpected diagnostics with Siemens profile enabled.

## Step 3: Formatting Comparison

1. Keep `vendor_profile = "siemens"`.
2. Run `Shift+Alt+F` and note style.
3. Temporarily switch to `vendor_profile = "codesys"`.
4. Re-open/format and compare behavior.
5. Switch back to `siemens`.

## Step 4: Profile Comparison Exercise

1. With `codesys`, confirm `#` references produce compatibility issues.
2. Restore `siemens` and confirm diagnostics clear.

## Step 5: Debug/Runtime Launch

1. Set breakpoint in `FB_EdgeCounter`.
2. Press `F5`.
3. Toggle `%IX0.0` (mapped pulse input) in Runtime Panel.
4. Observe `%QX0.0` transition as count indicator.

## Step 6: Export Direct Siemens `.scl` Files for TIA

```bash
trust-runtime plcopen export --project examples/siemens_scl_v1 --target siemens --json
```

Generated importable source files:

- `examples/siemens_scl_v1/interop/plcopen.siemens.xml.scl/*.scl`

Full TIA import walkthrough:

- `docs/guides/SIEMENS_TIA_SCL_IMPORT_TUTORIAL.md`

## Pitfalls

- Forgetting to revert profile back to `siemens` after comparison.
- Assuming generic profile accepts Siemens `#` style everywhere.
- Assuming task/resource wiring is auto-mapped in TIA after source import.
