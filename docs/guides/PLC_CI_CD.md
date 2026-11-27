# truST PLC CI/CD Guide

Use CI mode to run deterministic build/validate/test checks for a PLC project folder.

## CI commands

Build the program image (`program.tpi`) with machine-readable output:

```bash
trust-runtime build --project <project-folder> --ci
```

The build needs the compiler toolchain. `trust-runtime build` installs it on the first build that
needs it; install it beforehand on a runner with `trust-runtime toolchain install`, or from a
bundle file with `trust-runtime toolchain install --from <bundle>`.

Validate project config + bundle with machine-readable output:

```bash
trust-runtime validate --project <project-folder> --ci
```

Run ST tests in CI mode (defaults to JUnit if `--output` is omitted):

```bash
trust-dev test --project <project-folder> --ci --output junit
```

Run ST tests with stable machine-readable JSON summary output:

```bash
trust-dev test --project <project-folder> --ci --output json
```

Report the language server's diagnostics for every source file, as the editor shows
them (same `trust-lsp.toml`: include paths, libraries, vendor profile,
`[diagnostics]` toggles and severity overrides):

```bash
trust-lsp check --project <project-folder>                  # path:line:col: severity[code]: message
trust-lsp check --project <project-folder> --format json    # machine-readable
trust-lsp check --project <project-folder> --deny-warnings  # warnings fail the job too
```

`--file PATH` (repeatable) limits the report to some files while the whole project is
still analyzed; a file that is not one of the project's source files is reported as a
usage error instead of passing unchecked. Paths are printed relative to the project, in
the platform's form (`src\Main.st` on Windows). Exit status: `0` no errors, `1` errors
(or warnings with `--deny-warnings`), `2` usage or I/O problem (including a project
without source files).

Generate markdown docs for API review artifacts:

```bash
trust-dev docs --project <project-folder> --format markdown --out-dir <project-folder>/docs/api
```

## Exit codes (`--ci`)

| Code | Meaning |
|---|---|
| `0` | Success |
| `10` | Invalid project/configuration input |
| `11` | Build/compile failure |
| `12` | Test failure (assertion/runtime test error) |
| `13` | Timeout (reserved for CI wrappers) |
| `20` | Internal/unclassified failure |

## GitHub Actions template

Copy `.github/workflows/templates/trust-runtime-project-ci.yml` to
`.github/workflows/trust-runtime-project-ci.yml` in the project's repository, and set its
`TRUST_VERSION` to the truST release the project is built with (specification 22, "Project CI
template"). On each push or pull request that changes `src/`, a `.toml` file at the project's root
or the workflow itself, it:

1. installs that release's `trust-runtime-linux-x64.tar.gz`, checked against the release's
   `SHA256SUMS` before it is unpacked, and the release's toolchain;
2. runs `trust-runtime build --project . --ci`;
3. runs `trust-runtime validate --project . --ci`;
4. runs `trust-dev test --project . --ci --output junit`;
5. uploads the JUnit report as the artifact `st-tests-junit`, after a failed step too.

The project's repository needs no Rust. Tutorial 21 walks through it.

## Docker image (deterministic runner)

Build:

```bash
docker build -f docker/ci/trust-runtime-ci.Dockerfile -t trust-runtime-ci:local .
```

Run:

```bash
docker run --rm -v "$PWD":/workspace -w /workspace trust-runtime-ci:local \
  cargo run -p trust-runtime --bin trust-runtime -- test --project . --ci --output junit
```
