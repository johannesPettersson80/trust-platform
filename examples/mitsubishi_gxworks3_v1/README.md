# Mitsubishi GX Works3 v1: Vendor Profile Tutorial

Docs category: `docs/public/examples/vendor-profiles.md`

This tutorial demonstrates Mitsubishi profile behavior and its mapping to
standard IEC edge-detection concepts.

## What You Learn

- `DIFU` / `DIFD` compatibility behavior
- mapping to standard IEC equivalents (`R_TRIG` / `F_TRIG`)
- profile comparison vs generic codesys settings
- VS Code navigation/debug workflow for vendor syntax

## Files

- `src/Main.st`
- `src/Configuration.st`
- `trust-lsp.toml`
- `.vscode/launch.json`

## Step 1: Open + Build

```bash
code examples/mitsubishi_gxworks3_v1
trust-runtime build --project examples/mitsubishi_gxworks3_v1 --sources src
trust-runtime validate --project examples/mitsubishi_gxworks3_v1
```

## Step 2: Understand Alias Mapping

In this profile:

- `DIFU` behaves as rising-edge detector (`R_TRIG` equivalent)
- `DIFD` behaves as falling-edge detector (`F_TRIG` equivalent)

Open `src/Main.st` and inspect `FB_EdgeBridge`.

## Step 3: Go To Definition Exercise

1. Ctrl+Click `DIFU`.
2. Inspect resolved standard-edge definition path (profile-provided aliasing).
3. Repeat for `DIFD`.

## Step 4: Profile Comparison

1. Keep `vendor_profile = "mitsubishi"` and confirm clean diagnostics.
2. Temporarily switch to `vendor_profile = "codesys"`.
3. Re-open `src/Main.st` and observe unsupported alias behavior.
4. Revert to `mitsubishi`.

## Step 5: Debug + Runtime Panel

1. Set breakpoint after `Bridge(Signal := Pulse);`.
2. Press `F5`.
3. Toggle `%IX0.0` (pulse input).
4. Observe `%QX0.0`/`%QX0.1` outputs for rising/falling edge detection.

## Pitfalls

- Forgetting to revert vendor profile after comparison tests.
- Expecting `DIFU`/`DIFD` behavior in non-Mitsubishi profiles.
