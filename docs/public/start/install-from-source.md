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

## Optional OpenOT Reference Checkout

Normal source builds fetch the OpenOT Rust crates through pinned public Git
dependencies. Some OpenOT IEC examples, conformance fixtures, and telemetry
tests also read the OpenOT ST source package from a sibling checkout. Create it
only when you need those OpenOT example or test flows:

```bash
bash scripts/checkout_openot_ref.sh
```

The script creates `../open-ot-ref` from the public `open-ot-experiments`
repository. If you build from a GitHub source archive instead of a Git clone,
run the script from the extracted `trust-platform` directory.

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
