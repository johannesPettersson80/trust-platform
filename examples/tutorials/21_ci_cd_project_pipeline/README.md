# Tutorial 21: CI/CD Pipeline for a PLC Project

This tutorial turns a local PLC project into a CI-ready pipeline with
machine-readable output and deterministic exit-code handling.

## Why this tutorial exists

Teams often run local commands manually but fail to codify them into CI gates.
This tutorial shows how to produce stable artifacts and clear pass/fail signals.

## What you will learn

- CI command sequence (`build`, `validate`, `test`)
- output formats (`junit`, `json`)
- exit code semantics for automation
- how to wire the provided GitHub Actions template

## Prerequisites

- `trust-runtime` available
- one project with tests (Tutorial 11 is recommended)

## Step 1: Pick a test-bearing project

Why: CI pipeline should exercise both compile and runtime test paths.

Use tutorial 11 project:

```bash
cd /home/johannes/projects/trust-platform
```

Project path used below:

- `examples/tutorials/11_unit_testing_102`

## Step 2: Run build gate in CI mode

Why: `--ci` stabilizes machine-readable output and exit semantics.

```bash
trust-runtime build --project examples/tutorials/11_unit_testing_102 --ci
```

Expected result:
- successful build with CI-style output

## Step 3: Run config + bundle validation gate

Why: valid bytecode is not enough if runtime/config contracts are broken.

```bash
trust-runtime validate --project examples/tutorials/11_unit_testing_102 --ci
```

Expected result:
- validation success

## Step 4: Run tests with JUnit output

Why: JUnit is broadly compatible with CI report viewers.

```bash
trust-runtime test --project examples/tutorials/11_unit_testing_102 --ci --output junit > /tmp/trust-tutorial-21-junit.xml
```

Expected result:
- XML report written to `/tmp/trust-tutorial-21-junit.xml`

## Step 5: Run tests with JSON output

Why: JSON is easier for custom dashboards and programmatic quality gates.

```bash
trust-runtime test --project examples/tutorials/11_unit_testing_102 --ci --output json > /tmp/trust-tutorial-21-report.json
```

Expected result:
- JSON report includes per-test and summary durations

## Step 6: Understand exit codes for pipeline logic

Why: CI needs deterministic failure classes.

Use this mapping:

- `0`: success
- `10`: invalid project/config
- `11`: build/compile failure
- `12`: test failure
- `13`: timeout (reserved)
- `20`: internal/unclassified failure

Operational rule:
- fail fast on first non-zero code
- preserve artifacts for failed stages

## Step 7: Simulate a failing gate

Why: you need to verify CI fails correctly, not only success path.

1. Break one assertion in `examples/tutorials/11_unit_testing_102/sources/tests.st`.
2. Re-run test command with `--ci --output junit`.
3. Verify non-zero exit and failing report entries.

## Step 8: Wire GitHub Actions template

Why: avoid inventing pipeline wiring from scratch.

Use template:

- `.github/workflows/templates/trust-runtime-project-ci.yml`

Template flow should keep this order:
1. build runtime binary
2. `build --ci`
3. `validate --ci`
4. `test --ci --output junit`
5. upload junit artifact

## Step 9: Optional deterministic container runner

Why: containerized CI avoids host drift and missing tools.

```bash
docker build -f docker/ci/trust-runtime-ci.Dockerfile -t trust-runtime-ci:local .
docker run --rm -v "$PWD":/workspace -w /workspace trust-runtime-ci:local \
  cargo run -p trust-runtime --bin trust-runtime -- test --project examples/tutorials/11_unit_testing_102 --ci --output junit
```

## Common mistakes

- running CI without `--ci` and expecting stable parsing
- collecting only pass/fail status without artifacts
- skipping validation gate between build and test

## Completion checklist

- [ ] build/validate/test gates run in CI mode
- [ ] junit and json artifacts produced
- [ ] failing test path validated with non-zero exit code
- [ ] workflow template reviewed and adapted
