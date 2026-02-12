# Mitsubishi GX Works3 v1 Example

This example demonstrates the GX Works3 compatibility baseline in truST:

- Mitsubishi GX Works3 vendor profile (`vendor_profile = "mitsubishi"`)
- `DIFU` / `DIFD` edge-detection aliases mapped to IEC behavior
- deterministic formatting and diagnostics for the supported subset

## Files

- `src/Main.st`: edge bridge function block using `DIFU` and `DIFD`
- `sources/Main.st`: runtime build input mirror for CLI workflows
- `trust-lsp.toml`: enables Mitsubishi vendor profile

## Run

```bash
trust-runtime build --project .
```

To inspect GX Works3 formatting behavior in the editor, open this folder in VS Code with the truST extension and run `Structured Text: Format Document`.
