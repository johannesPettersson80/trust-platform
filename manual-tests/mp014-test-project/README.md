# MP-014 Sample ST Test Project

This project is a minimal fixture for validating `trust-runtime test`.

## Layout

- `sources/tests_assertions.st`: passing test cases
- `sources/tests_failure.st`: an intentional failing case

## Run

From repository root:

```bash
cargo run -p trust-runtime --bin trust-runtime -- test --project manual-tests/mp014-test-project
```

Run only passing cases:

```bash
cargo run -p trust-runtime --bin trust-runtime -- test --project manual-tests/mp014-test-project --filter Pass
```

Machine-readable formats:

```bash
cargo run -p trust-runtime --bin trust-runtime -- test --project manual-tests/mp014-test-project --output junit
cargo run -p trust-runtime --bin trust-runtime -- test --project manual-tests/mp014-test-project --output tap
cargo run -p trust-runtime --bin trust-runtime -- test --project manual-tests/mp014-test-project --output json
```
