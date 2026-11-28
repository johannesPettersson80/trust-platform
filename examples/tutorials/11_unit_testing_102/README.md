# Tutorial 11: Testing Control Logic Without Hardware

Test a tank's control logic with the inputs a plant would give, find a bug from the failure it
causes, and run the same logic in the runtime with its inputs written by hand. You learn to keep
the logic in a function block that a test can drive, the addresses in the configuration and the
program as wiring between them, so that every decision is tested before a plant is connected.

## What you learn

- The logic goes in a `FUNCTION_BLOCK` with plain inputs and outputs, the addresses in the
  configuration's `VAR_CONFIG`, and the `PROGRAM` only wires the two together. A test calls the
  block with the values a plant would give, mock I/O, so it needs neither hardware nor addresses.
- A failure names the assertion, its line and its values, and the scenario around that line leads
  to the branch of the logic that is wrong.
- The block that the tests check is the code the runtime runs: inputs written with
  `trust-runtime ctl` give the outputs that the tests expect.

## What you need

- This repository, whose `examples/tutorials/11_unit_testing_102` you copy, and `trust-runtime`
  and `trust-dev`, from the truST extension for VS Code or a release archive, on the `PATH`, with
  the truST toolchain (tutorial 10, "What you need").
- Linux or macOS, whose shell runs the commands below, and two terminals. About 20 minutes.

## Step by step

1. From the root of the repository, copy the project:

    ```bash
    rm -rf /tmp/trust-tutorial-11
    cp -R examples/tutorials/11_unit_testing_102 /tmp/trust-tutorial-11
    cd /tmp/trust-tutorial-11
    ```

    `src/main.st` holds the logic, the program and the configuration, and `src/tests.st` the
    tests. `runtime.toml` and `io.toml` set up the runtime and its `simulated` driver, which keeps
    each input as it is written.

2. Read the logic, the function block `TankControl` in `src/main.st`. The stop command turns
   everything off. With the start command, a level below 300 opens the fill valve with the pump at
   800, a level above 700 opens the drain valve with the pump at 600, and a level in between holds
   with the pump at 500. Without the start command, everything is off:

    ```st
    IF StopCmd THEN
        FillValveCmd := FALSE;
        DrainValveCmd := FALSE;
        PumpSpeedCmd := INT#0;
    ELSIF StartCmd THEN
        IF LevelRaw < INT#300 THEN
            FillValveCmd := TRUE;
            DrainValveCmd := FALSE;
            PumpSpeedCmd := INT#800;
        ELSIF LevelRaw > INT#700 THEN
            FillValveCmd := FALSE;
            DrainValveCmd := TRUE;
            PumpSpeedCmd := INT#600;
        ELSE
            FillValveCmd := FALSE;
            DrainValveCmd := FALSE;
            PumpSpeedCmd := INT#500;
        END_IF;
    ELSE
        FillValveCmd := FALSE;
        DrainValveCmd := FALSE;
        PumpSpeedCmd := INT#0;
    END_IF;
    ```

3. Read the wiring. The program `TankProgram` calls its instance `Ctrl` of the block with its own
   variables and copies the block's outputs to its own, and the configuration puts those variables
   at the addresses of the I/O:

    ```st
    VAR_CONFIG
        P1.StartCmd AT %IX0.0 : BOOL;
        P1.StopCmd AT %IX0.1 : BOOL;
        P1.LevelRaw AT %IW2 : INT;
        P1.FillValveOut AT %QX0.0 : BOOL;
        P1.DrainValveOut AT %QX0.1 : BOOL;
        P1.PumpSpeedOut AT %QW2 : INT;
    END_VAR
    ```

    Neither makes a decision, so testing the block tests every decision.

4. Read the tests in `src/tests.st`. The `TEST_PROGRAM TankControlWithMockIo` holds its own
   instance of the block, `Dut`, and calls it with three states of the plant: a low level with the
   start command, a high level, and both commands with a low level. After each call it checks the
   valves and the pump:

    ```st
    (* Low level -> fill tank fast. *)
    Dut(StartCmd := TRUE, StopCmd := FALSE, LevelRaw := INT#200);
    ASSERT_TRUE(IN := Dut.FillValveCmd);
    ASSERT_FALSE(IN := Dut.DrainValveCmd);
    ASSERT_EQUAL(EXPECTED := INT#800, ACTUAL := Dut.PumpSpeedCmd);
    ```

    The `TEST_FUNCTION_BLOCK TankControlBand` checks a level in the band, 500.

5. Run the tests:

    ```bash
    trust-dev test --project .
    ```

    ```text
    Running 2 ST test(s) in .
    PASS [1/2] TEST_PROGRAM::TankControlWithMockIo (src/tests.st) [0ms]
    PASS [2/2] TEST_FUNCTION_BLOCK::TankControlBand (src/tests.st) [0ms]
    2 passed, 0 failed, 0 errors (404ms)
    ```

6. Triage a failure. In `src/main.st`, write `PumpSpeedCmd := INT#700;` in place of
   `PumpSpeedCmd := INT#800;`, as a mistyped speed would be, and run the tests again:

    ```text
    Running 2 ST test(s) in .
    FAIL [1/2] TEST_PROGRAM::TankControlWithMockIo src/tests.st:10 [0ms]
        reason   : ASSERT_EQUAL failed: expected 800, actual 700
        source   : ASSERT_EQUAL(EXPECTED := INT#800, ACTUAL := Dut.PumpSpeedCmd);
    PASS [2/2] TEST_FUNCTION_BLOCK::TankControlBand (src/tests.st) [0ms]

    Failure summary:
      1. TEST_PROGRAM::TankControlWithMockIo @ src/tests.st:10
         ASSERT_EQUAL failed: expected 800, actual 700
         source: ASSERT_EQUAL(EXPECTED := INT#800, ACTUAL := Dut.PumpSpeedCmd);
    1 passed, 1 failed, 0 errors (358ms)
    Error: 1 ST test(s) failed
    ```

    Line 10 belongs to the low-level scenario, and the pump's speed in that scenario comes from
    the fill branch, `LevelRaw < INT#300`, which now gives 700. The band's test still passes, as
    its branch is untouched. Put `INT#800` back, and both pass.

7. Run the logic in the runtime. Build the program with
   `trust-runtime build --project . --sources src`, start the runtime in the second terminal with
   `trust-runtime run --project .`, and in the first give the start command and a low level:

    ```bash
    trust-runtime ctl --project . io-write %IX0.0 true
    trust-runtime ctl --project . io-write %IW2 200
    trust-runtime ctl --project . io-read
    ```

    In the answer of `io-read`, the outputs read as the first scenario's assertions expect:
    `%QX0.0` (`P1.FillValveOut`) `Bool(true)`, `%QX0.1` (`P1.DrainValveOut`) `Bool(false)` and
    `%QW2` (`P1.PumpSpeedOut`) `Int(800)`. Write the level `850` to `%IW2`, and the drain valve
    opens with the pump at `Int(600)`; write `true` to `%IX0.1`, the stop, and all three outputs go
    off, the pump at `Int(0)`. Stop the runtime with
    `trust-runtime ctl --project . shutdown --exit`.

## What happened

**Logic, wiring and addresses.** The function block makes every decision and knows no address,
the program connects the block to its own variables, and the configuration puts those variables at
the I/O addresses. A change of the I/O, another card or another address, touches the configuration;
a change of behaviour touches the block, and the tests follow the block. Logic that is hard to test
usually sits in the program: moving it into a block, and keeping the program as wiring, makes it
testable.

**Mock I/O.** A test uses the block as the program does: it calls the block with the values a
plant would give and checks what the block commands, one call per scan. The block's inputs come
from the test, not from a driver, so the tests need no hardware, run in a fraction of a second and
run the same way on every computer. Each test starts from a cold start (tutorial 10), and each holds
its own instance of the block.

**From a failure to the fault.** The failure names the assertion's file and line, its message and
its values. The scenario around the line says which state of the plant the block handled wrongly,
and so which branch to read: the low level and the pump's speed lead to the fill branch.

**The tested block runs in the PLC.** The runtime runs the program, which calls the same block.
With the simulated driver, the inputs are those written with `ctl`, and the outputs are the ones the
tests expect; with a real I/O driver, the same addresses connect the block to the plant.

## Try it yourself

- Test the band's edges. The tests check the levels 200, 850, 100 and 500, but not 300 and 700,
  where the band begins and ends. Write the test:

  ```bash
  cat > src/boundaries.st <<'ST'
  TEST_FUNCTION_BLOCK TankControlBoundaries
  VAR
      Dut : TankControl;
  END_VAR

  (* 300 and 700 are the band's own edges: hold. *)
  Dut(StartCmd := TRUE, StopCmd := FALSE, LevelRaw := INT#300);
  ASSERT_EQUAL(EXPECTED := INT#500, ACTUAL := Dut.PumpSpeedCmd);
  Dut(StartCmd := TRUE, StopCmd := FALSE, LevelRaw := INT#700);
  ASSERT_EQUAL(EXPECTED := INT#500, ACTUAL := Dut.PumpSpeedCmd);
  END_TEST_FUNCTION_BLOCK
  ST
  ```

  The three tests pass. Now write `LevelRaw <= INT#300` in place of `LevelRaw < INT#300` in
  `src/main.st`, a slip that the shipped tests miss: the new test fails,
  `FAIL [1/3] TEST_FUNCTION_BLOCK::TankControlBoundaries src/boundaries.st:8` with
  `ASSERT_EQUAL failed: expected 500, actual 800`. Put `<` back.
- Write a test that never ends:

  ```bash
  cat > src/endless.st <<'ST'
  TEST_PROGRAM EndlessLoop
  VAR
      Count : DINT;
  END_VAR
  WHILE TRUE DO
      Count := Count + 1;
  END_WHILE;
  END_TEST_PROGRAM
  ST
  ```

  Run it with `trust-dev test --project . --filter Endless`: within a fraction of a second it ends
  as an error, `ERROR [1/1] TEST_PROGRAM::EndlessLoop src/endless.st:6` with
  `reason   : execution timed out: work limit 100000000, used 100000000, requested 1`. The runner
  limits the work each test may do, so an endless loop cannot hang the run. Delete it with
  `rm src/endless.st`.

## Reference

- Specification 22, "Test case execution" and "Test reporting and CI output"; specification 18,
  configurations, resources and tasks, and `VAR_CONFIG`.
- Tutorial 10, unit tests and their results; tutorial 13, `trust-runtime ctl`; tutorial 21, which
  runs this project's steps in CI.
