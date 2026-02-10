# Unit Testing 102: Mock I/O and Failure Triage

This tutorial shows the production pattern you should use in truST:

1. keep physical I/O mapping (`%I`, `%Q`, `%M`) inside a `PROGRAM`,
2. keep control logic inside a `FUNCTION_BLOCK` with plain inputs/outputs,
3. unit test the function block with simulated values (mock I/O).

This gives fast tests, deterministic behavior, and easier debugging.

## What you will build

- A production `PROGRAM` (`TankProgram`) that maps real I/O addresses.
- A logic block (`FB_TANK_CONTROL`) that can be tested without hardware.
- Tests that simulate low/high/stop conditions and verify outputs.

## Project layout

- `sources/main.st`: production code (I/O mapping + logic FB)
- `sources/tests.st`: unit tests
- `trust-lsp.toml`: project config

## Step 1: Understand the architecture

Open `sources/main.st`:

- `FB_TANK_CONTROL` contains decision logic.
- `PROGRAM TankProgram` maps field I/O to `%IX/%IW/%QX/%QW`.
- Program calls FB and copies FB outputs to mapped outputs.

This separation is the key to mockable tests.

## Step 2: Review the tests

Open `sources/tests.st`:

- `TEST_PROGRAM TEST_TANK_CONTROL_WITH_MOCK_IO`:
  - instantiates the FB directly (`VAR DUT : FB_TANK_CONTROL;`)
  - drives input scenarios (`StartCmd`, `StopCmd`, `LevelRaw`)
  - verifies expected outputs with `ASSERT_*`
- `TEST_FUNCTION_BLOCK TEST_TANK_CONTROL_BAND`:
  - checks normal operating band behavior in a separate test case

No physical addresses are needed in tests.

## Step 3: Run tests

From repository root:

```bash
cargo run -p trust-runtime --bin trust-runtime -- test --project examples/tutorials/11_unit_testing_102
```

## Step 4: Triage a failure (red-green-refactor)

1. Introduce a bug on purpose:
   - in `FB_TANK_CONTROL`, change `PumpSpeedCmd := INT#800;` to `INT#700;`
2. Run tests again.
3. Read failure details:
   - test name (`TEST_PROGRAM::...`)
   - file and line
   - `reason` (`ASSERT_EQUAL failed ...`)
   - `source` line (assertion that failed)
4. Fix code.
5. Re-run until green.

## Step 5: Debug only one test while iterating

```bash
cargo run -p trust-runtime --bin trust-runtime -- test --project examples/tutorials/11_unit_testing_102 --filter BAND
```

## Step 6: Export machine-readable output for CI

```bash
cargo run -p trust-runtime --bin trust-runtime -- test --project examples/tutorials/11_unit_testing_102 --output junit
```

```bash
cargo run -p trust-runtime --bin trust-runtime -- test --project examples/tutorials/11_unit_testing_102 --output json
```

## Rule of thumb

If logic is hard to unit test, move more logic from `PROGRAM` into an FB/function and keep `PROGRAM` as wiring only.
