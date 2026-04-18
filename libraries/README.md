# Reusable Structured Text Libraries

Reusable truST Structured Text packages belong under `libraries/` in this repo.

Package shape:

```text
libraries/
  my_library/
    trust-lsp.toml
    src/
```

Use `[dependencies]` in a consuming project's `trust-lsp.toml` to reference a
package under `libraries/`.

Use `[[libraries]]` for external/index-only stub packs or vendor documentation
attachments, not for normal reusable truST library packages.

`crates/.../tests/fixtures/` is test-only infrastructure and must not be used as
a user library location.

Current shipped package roots include:

- `libraries/plcopen_motion/`
- `libraries/oscat/`
