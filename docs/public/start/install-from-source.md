# Install From Source

Build truST from source when you are contributing, testing unreleased changes,
or packaging it yourself.

## Contributor Path

1. Install Rust and Cargo from <https://rustup.rs/>.
2. Clone the repository.
3. Build the shipped binaries:

```bash
cargo build -p trust-lsp -p trust-runtime -p trust-debug
```

4. For release-profile binaries:

```bash
cargo build --release -p trust-lsp -p trust-runtime -p trust-debug
```

`trust-harness` is built from the `trust-runtime` package, so you do not pass
`-p trust-harness` separately.

## Verify The Build

```bash
command -v trust-lsp
command -v trust-runtime
command -v trust-debug
command -v trust-harness
trust-runtime --version
trust-runtime --help
```

## Use release installs unless you need source builds

- If you only want the editor workflow, use [Installation](installation.md).
- If you only need runtime binaries, use the GitHub release download path from
  [Installation](installation.md).

## Next

- [Installation](installation.md)
- [Contribute](../contribute.md)
- [Maintaining Docs](../MAINTAINING.md)
