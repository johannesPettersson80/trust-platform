# truST PLC CI/CD Guide

Use CI mode to run deterministic build/validate/test checks for a PLC project folder.

## CI commands

Build bytecode with machine-readable output:

```bash
trust-runtime build --project <project-folder> --ci
```

Validate project config + bundle with machine-readable output:

```bash
trust-runtime validate --project <project-folder> --ci
```

Run ST tests in CI mode (defaults to JUnit if `--output` is omitted):

```bash
trust-runtime test --project <project-folder> --ci --output junit
```

Run ST tests with stable machine-readable JSON summary output:

```bash
trust-runtime test --project <project-folder> --ci --output json
```

Generate markdown docs for API review artifacts:

```bash
trust-runtime docs --project <project-folder> --format markdown --out-dir <project-folder>/docs/api
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

Copy and adapt:

`.github/workflows/templates/trust-runtime-project-ci.yml`

Template flow:

1. Build `trust-runtime`.
2. Run `build --ci`.
3. Run `validate --ci`.
4. Run `test --ci --output junit`.
5. Upload JUnit artifact.

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
