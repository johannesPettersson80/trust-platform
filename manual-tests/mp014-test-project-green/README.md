# MP-014 All-Green CI Fixture

This fixture is intentionally all-green for CI validation of `trust-runtime test`.

## Layout

- `sources/tests_green.st`: passing `TEST_PROGRAM` and `TEST_FUNCTION_BLOCK` cases

## Run

From repository root:

```bash
cargo run -p trust-runtime --bin trust-runtime -- test --project manual-tests/mp014-test-project-green
```

Machine-readable formats:

```bash
cargo run -p trust-runtime --bin trust-runtime -- test --project manual-tests/mp014-test-project-green --output junit
cargo run -p trust-runtime --bin trust-runtime -- test --project manual-tests/mp014-test-project-green --output tap
cargo run -p trust-runtime --bin trust-runtime -- test --project manual-tests/mp014-test-project-green --output json
```
