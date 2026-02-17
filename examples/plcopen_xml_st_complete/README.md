# PLCopen XML ST-Complete: VS Code-First Tutorial

This is the primary PLCopen XML tutorial for users.

Primary flow: import from VS Code command palette.

Secondary flow: CLI commands for CI/automation.

## What This Covers

- VS Code command: `Structured Text: Import PLCopen XML`
- Post-import code exploration in editor
- CLI import/export round-trip alternative
- OpenPLC ecosystem detection note

## Tutorial Assets

- Native ST project source: `src/`
- PLCopen fixtures:
  - `interop/codesys-small.xml`
  - `interop/openplc.xml`

## Step 1 (Primary): Import XML via VS Code

1. Open repository in VS Code:

```bash
code /path/to/trust-platform
```

2. `Ctrl+Shift+P` -> `Structured Text: Import PLCopen XML`
3. Select input XML:
   - start with `examples/plcopen_xml_st_complete/interop/codesys-small.xml`
4. Select target folder (for example `/tmp/trust-plcopen-import`)
5. Open migration report when prompted

## Step 2: Post-Import Exploration (Editor)

In the imported project folder:

1. Open generated ST files under `src/`.
2. Confirm diagnostics are clean.
3. Ctrl+Click imported type names to verify go-to-definition works.
4. Run `Shift+Alt+F` on imported files to normalize formatting.

### CODESYS GlobalVars Note (`qualified_only`)

For CODESYS exports that use `{attribute 'qualified_only'}` global variable
lists (for example `GVL.start`), import now materializes a compiler-valid model:

- Global list file contains `TYPE + CONFIGURATION/VAR_GLOBAL` wrapper content.
- Referencing POUs get injected `VAR_EXTERNAL` declarations for the list
  (for example `GVL : GVL_TYPE;`).
- Imported functions without explicit result assignment get a deterministic
  fallback (`<FunctionName> := <FunctionName>;`) to keep diagnostics clean.

## Step 3: OpenPLC Detection Note

Repeat Step 1 using:

- `examples/plcopen_xml_st_complete/interop/openplc.xml`

In migration report, expect:

- `detected_ecosystem = "openplc"`
- possible shim entries such as `R_EDGE -> R_TRIG`

## Step 4 (Alternative): CLI Import/Export

Import:

```bash
mkdir -p /tmp/trust-plcopen-import
trust-runtime plcopen import \
  --input examples/plcopen_xml_st_complete/interop/codesys-small.xml \
  --project /tmp/trust-plcopen-import --json
```

Export:

```bash
trust-runtime plcopen export \
  --project /tmp/trust-plcopen-import \
  --output /tmp/trust-plcopen-import/interop/roundtrip.xml --json
```

## Step 5: Deterministic Round-Trip Check

```bash
trust-runtime plcopen export \
  --project examples/plcopen_xml_st_complete \
  --output examples/plcopen_xml_st_complete/interop/exported.xml --json
```

Then import the exported XML to a new folder and export again. Supported
ST-structure signatures should remain stable.

## Pitfalls and Fixes

- Importing into non-empty target folder:
  - fix: choose empty folder or confirm overwrite intentionally.
- Non-ST content in source XML (FBD/LD/SFC):
  - fix: treat as expected unsupported diagnostics in migration report.
- Imported files look inconsistent:
  - fix: run format document after import.

## Debug/Launch

Use `.vscode/launch.json` for quick debug entry from this example folder.
