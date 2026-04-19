# Vendor Library Stubs: User-Extensible Interop Pattern

Docs category: `docs/public/examples/vendor-profiles.md`

This tutorial shows how to index vendor-specific symbols that are not covered by
built-in shim mappings.

## Goal

Provide local ST library stubs so editor workflows stay productive during
migration:

- completion
- hover
- go-to-definition
- missing-symbol diagnostic suppression for declared vendor blocks

## Project Layout

- `src/Main.st`: program that uses vendor symbols (`TCON`, `NORM_X`, `SCALE_X`)
- `src/Configuration.st`: minimal task/program wiring
- `vendor/siemens/*.st`: local stub library declarations
- `trust-lsp.toml`: `[[libraries]]` wiring for stub indexing

## Step 1: Open This Example in VS Code

```bash
code examples/vendor_library_stubs
```

## Step 2: Verify Library Wiring

Open `trust-lsp.toml`:

```toml
[[libraries]]
name = "siemens-stubs"
path = "vendor/siemens"
```

This tells `trust-lsp` to index the vendor stub folder as a library source.

## Step 3: Test LSP Features

1. Open `src/Main.st`.
2. Trigger completion on `TCON`, `NORM_X`, `SCALE_X` usage.
3. Ctrl+Click `TCON` to navigate to `vendor/siemens/tcon.st`.
4. Hover symbol names and parameter names to inspect type info.

## Step 4: Add a New Missing Vendor Block

1. Create `vendor/siemens/my_block.st`.
2. Add a declaration, for example:

```st
FUNCTION_BLOCK MY_VENDOR_FB
VAR_INPUT
    Enable : BOOL;
END_VAR
VAR_OUTPUT
    Active : BOOL;
END_VAR
END_FUNCTION_BLOCK
```

3. Use `MY_VENDOR_FB` in `src/Main.st`.
4. Save and verify completion/hover/definition support.

## Optional Runtime Build Check

Because runtime build scans source roots, include both `src/` and `vendor/` when
building this tutorial project:

```bash
trust-runtime build --project examples/vendor_library_stubs --sources .
```

## Limits

- These stubs provide type/symbol contracts, not vendor runtime semantics.
- Real behavior still requires migration or implementation work.
