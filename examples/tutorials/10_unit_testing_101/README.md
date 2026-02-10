# Unit Testing in truST (Tutorial 10)

This tutorial teaches how to write and run unit tests in truST, and how the truST test model works.

## Learning goals

After this tutorial, you will be able to:

- write tests with `TEST_PROGRAM` and `TEST_FUNCTION_BLOCK`
- use `ASSERT_TRUE`, `ASSERT_FALSE`, `ASSERT_EQUAL`, and `ASSERT_NEAR`
- test both pure logic (functions) and stateful logic (function blocks)
- run all tests, run filtered tests, and export machine-readable reports for CI

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

1. discovers test POUs in the project,
2. executes each test in deterministic order,
3. isolates test execution so one test does not leak state into another,
4. reports pass/fail with file and line context,
5. optionally emits `junit`, `tap`, or `json` output for CI tooling.

Note: `TEST_PROGRAM`, `TEST_FUNCTION_BLOCK`, and `ASSERT_*` are truST extensions (not IEC standard keywords).

## Project layout

- `sources/main.st`: production logic under test
- `sources/tests.st`: test cases
- `trust-lsp.toml`: minimal project configuration

## Step 1: Review the production code

Open `sources/main.st`. It contains:

- `LIMIT_ADD` (pure function with clamping),
- `SCALE_RAW_TO_PERCENT` (integer-to-real conversion),
- `FB_START_STOP` (stateful start/stop behavior).

## Step 2: Review how tests are written

Open `sources/tests.st`.

- `TEST_PROGRAM TEST_LIMIT_ADD_AND_SCALING` tests pure function behavior.
- `TEST_FUNCTION_BLOCK TEST_FB_START_STOP_SEQUENCE` tests state transitions across scan cycles.

This mirrors real PLC testing: pure algorithm checks + state machine checks.

## Step 3: Run all tests

From repository root:

```bash
cargo run -p trust-runtime --bin trust-runtime -- test --project examples/tutorials/10_unit_testing_101
```

Expected summary:

- `2 passed, 0 failed, 0 errors`

## Step 4: Run a subset (filter)

```bash
cargo run -p trust-runtime --bin trust-runtime -- test --project examples/tutorials/10_unit_testing_101 --filter START_STOP
```

Use this in large projects to run only a specific test family while iterating.

## Step 5: Export CI-friendly results

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

## Step 6: Practice red-green-refactor

1. Break one expectation in `sources/tests.st` (for example set an expected value to the wrong number).
2. Run tests and confirm the failing assertion output.
3. Fix either code or expected value.
4. Re-run until green.

That is the normal development loop with truST unit testing.
