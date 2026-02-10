# Unit Testing in truST (Tutorial 10)

This tutorial teaches how to write and run unit tests in truST, and how the test runner behaves.

## Learning goals

After this tutorial, you will be able to:

- write tests with `TEST_PROGRAM` and `TEST_FUNCTION_BLOCK`
- use all built-in `ASSERT_*` helpers
- test both pure logic (functions) and stateful logic (function blocks)
- run all tests, list tests, filter tests, set per-test timeout, and export CI-friendly output

## How unit testing works in truST

truST adds test-focused ST constructs:

- `TEST_PROGRAM ... END_TEST_PROGRAM`
- `TEST_FUNCTION_BLOCK ... END_TEST_FUNCTION_BLOCK`
- assertion functions (`ASSERT_*`)

When you run:

```bash
trust-runtime test --project <project-dir>
```

the runtime:

1. discovers test POUs in project sources,
2. executes each test in deterministic order,
3. isolates each test execution from others,
4. reports pass/fail/error with file and line context,
5. optionally emits `junit`, `tap`, or `json` output for CI.

Note: `TEST_PROGRAM`, `TEST_FUNCTION_BLOCK`, and `ASSERT_*` are truST extensions (not IEC standard keywords).

## Available assertions

| Assertion | Signature | Purpose |
|---|---|---|
| `ASSERT_TRUE` | `ASSERT_TRUE(IN: BOOL)` | Fails unless `IN` is `TRUE` |
| `ASSERT_FALSE` | `ASSERT_FALSE(IN: BOOL)` | Fails unless `IN` is `FALSE` |
| `ASSERT_EQUAL` | `ASSERT_EQUAL(EXPECTED, ACTUAL)` | Fails unless values are equal |
| `ASSERT_NOT_EQUAL` | `ASSERT_NOT_EQUAL(EXPECTED, ACTUAL)` | Fails unless values differ |
| `ASSERT_GREATER` | `ASSERT_GREATER(VALUE, BOUND)` | Fails unless `VALUE > BOUND` |
| `ASSERT_LESS` | `ASSERT_LESS(VALUE, BOUND)` | Fails unless `VALUE < BOUND` |
| `ASSERT_GREATER_OR_EQUAL` | `ASSERT_GREATER_OR_EQUAL(VALUE, BOUND)` | Fails unless `VALUE >= BOUND` |
| `ASSERT_LESS_OR_EQUAL` | `ASSERT_LESS_OR_EQUAL(VALUE, BOUND)` | Fails unless `VALUE <= BOUND` |
| `ASSERT_NEAR` | `ASSERT_NEAR(EXPECTED, ACTUAL, DELTA)` | Fails when `ABS(EXPECTED-ACTUAL) > DELTA` |

## Project layout

- `sources/main.st`: production logic under test
- `sources/tests.st`: test cases
- `trust-lsp.toml`: minimal project configuration

## Step 1: Review the production code

Open `sources/main.st`. It contains:

- `LIMIT_ADD` (pure function with clamping),
- `SCALE_RAW_TO_PERCENT` (integer-to-real conversion),
- `FB_START_STOP` (stateful start/stop behavior).

## Step 2: Review test cases

Open `sources/tests.st`.

- `TEST_PROGRAM TEST_LIMIT_ADD_AND_SCALING` tests pure function behavior.
- `TEST_FUNCTION_BLOCK TEST_FB_START_STOP_SEQUENCE` tests state transitions across scan cycles.
- `TEST_PROGRAM TEST_COMPARISON_ASSERTIONS` demonstrates comparison assertions.

## Step 3: Run all tests

From repository root:

```bash
cargo run -p trust-runtime --bin trust-runtime -- test --project examples/tutorials/10_unit_testing_101
```

Expected summary:

- `3 passed, 0 failed, 0 errors`

## Step 4: List tests without running

```bash
cargo run -p trust-runtime --bin trust-runtime -- test --project examples/tutorials/10_unit_testing_101 --list
```

Use this to verify discovery and names before selecting filters.

## Step 5: Run a subset (filter)

```bash
cargo run -p trust-runtime --bin trust-runtime -- test --project examples/tutorials/10_unit_testing_101 --filter START_STOP
```

If a filter matches nothing but tests exist, the runner prints a filtered-out message instead of reporting no discovery.

## Step 6: Set per-test timeout

```bash
cargo run -p trust-runtime --bin trust-runtime -- test --project examples/tutorials/10_unit_testing_101 --timeout 5
```

`--timeout` is per test case in seconds. `--timeout 0` disables timeout.

## Step 7: Export CI-friendly results

JUnit:

```bash
cargo run -p trust-runtime --bin trust-runtime -- test --project examples/tutorials/10_unit_testing_101 --output junit
```

TAP:

```bash
cargo run -p trust-runtime --bin trust-runtime -- test --project examples/tutorials/10_unit_testing_101 --output tap
```

JSON:

```bash
cargo run -p trust-runtime --bin trust-runtime -- test --project examples/tutorials/10_unit_testing_101 --output json
```

JSON includes test-level and summary `duration_ms` fields.

## Step 8: Practice red-green-refactor

1. Break one expectation in `sources/tests.st`.
2. Run tests and confirm the failing assertion output.
3. Fix code or expected value.
4. Re-run until green.

That is the normal development loop with truST unit testing.
