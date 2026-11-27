# Tutorial 10: Unit Tests for Structured Text

Run the unit tests of a small Structured Text library with `trust-dev test`, read what a passing
and a failing run report, and let a test find a bug. You learn how a test is written, how the
runner runs it and what each kind of result means. Tutorial 11 then tests a program's I/O logic,
and tutorial 21 runs the tests in CI.

## What you learn

- A test is a `TEST_PROGRAM` or a `TEST_FUNCTION_BLOCK` in the project's sources: it calls the
  code under test and checks the results with `ASSERT_*` calls. These are truST extensions, not
  IEC 61131-3 POU kinds.
- `trust-dev test` finds every test, compiles the tests with the project, runs each from a cold
  start and reports it as passed, failed or error, with the file and line of a failed assertion.
- `--list` names the tests without running them, and `--filter` runs those whose name contains a
  text.

## What you need

- This repository, whose `examples/tutorials/10_unit_testing_101` you copy, and `trust-dev`, from
  the truST extension for VS Code or a release archive, on the `PATH`.
- The truST toolchain, which compiles the tests for this computer: the extension installs it on its
  first build, and beside the tools of a release archive `trust-runtime toolchain install` installs
  it once. Without it, `trust-dev test` stops and names that command.
- Linux or macOS, whose shell runs the commands below. About 15 minutes.

## Step by step

1. From the root of the repository, copy the project:

    ```bash
    rm -rf /tmp/trust-tutorial-10
    cp -R examples/tutorials/10_unit_testing_101 /tmp/trust-tutorial-10
    cd /tmp/trust-tutorial-10
    ```

    `src/main.st` holds the code under test and `src/tests.st` its tests. `trust-lsp.toml` sets
    up the editor's language server: the CODESYS dialect, the sources in `src` and the IEC
    standard library.

2. Read the code under test in `src/main.st`. The function `LimitAdd` adds two `INT` values and
   limits the sum to a maximum:

    ```st
    Sum := AddendA + AddendB;
    IF Sum > MaxValue THEN
        LimitAdd := MaxValue;
    ELSE
        LimitAdd := Sum;
    END_IF;
    ```

    `ScaleRawToPercent` turns a raw count into a percentage, a tenth of the count as a `REAL`. The
    function block `StartStopLatch` keeps `Running` on from a start command until a stop command,
    and the stop wins when both come.

3. Read the tests in `src/tests.st`. The `TEST_PROGRAM LimitAddAndScaling` calls the functions and
   checks each result, the expected value first:

    ```st
    TEST_PROGRAM LimitAddAndScaling
    ASSERT_EQUAL(
        EXPECTED := INT#7,
        ACTUAL := LimitAdd(AddendA := INT#3, AddendB := INT#4, MaxValue := INT#10)
    );
    ```

    It goes on with a sum above the maximum, which must give the maximum, a negative sum, the
    scaling, compared with `ASSERT_NEAR` within a tolerance as a `REAL` result should be, and
    `ASSERT_TRUE` and `ASSERT_FALSE`. The `TEST_FUNCTION_BLOCK StartStopSequence` holds an instance
    of the latch, `Dut`, and calls it four times as four scans would: idle, start, no command and
    stop, checking `Dut.Running` after each call. `TEST_PROGRAM ComparisonAssertions` shows the
    assertions that compare a value with a bound.

4. Run the tests:

    ```bash
    trust-dev test --project .
    ```

    ```text
    Running 3 ST test(s) in .
    PASS [1/3] TEST_PROGRAM::LimitAddAndScaling (src/tests.st) [0ms]
    PASS [2/3] TEST_FUNCTION_BLOCK::StartStopSequence (src/tests.st) [0ms]
    PASS [3/3] TEST_PROGRAM::ComparisonAssertions (src/tests.st) [0ms]
    3 passed, 0 failed, 0 errors (265ms)
    ```

    Each line names a test by its kind and name, with its file and how long it ran. The last line
    counts the results and gives the time of the whole run, which varies from run to run.

5. List the tests without running them:

    ```bash
    trust-dev test --project . --list
    ```

    ```text
    TEST_PROGRAM::LimitAddAndScaling (src/tests.st:1)
    TEST_FUNCTION_BLOCK::StartStopSequence (src/tests.st:24)
    TEST_PROGRAM::ComparisonAssertions (src/tests.st:46)
    3 test(s) listed
    ```

    It gives each test with the line of its declaration, in the order of the files and lines.

6. Run one test. `--filter` selects the tests whose name contains its text, in any case:

    ```bash
    trust-dev test --project . --filter StartStop
    ```

    It runs `TEST_FUNCTION_BLOCK::StartStopSequence` alone and answers
    `1 passed, 0 failed, 0 errors`. `--filter startstop` runs the same test.

7. Let a test find a bug. In `src/main.st`, write `LimitAdd := Sum;` in place of
   `LimitAdd := MaxValue;`, so that the sum is no longer limited, and run the tests again with
   their exit code:

    ```bash
    trust-dev test --project .
    echo "exit $?"
    ```

    ```text
    Running 3 ST test(s) in .
    FAIL [1/3] TEST_PROGRAM::LimitAddAndScaling src/tests.st:6 [0ms]
        reason   : ASSERT_EQUAL failed: expected 5, actual 7
        source   : ASSERT_EQUAL(
    PASS [2/3] TEST_FUNCTION_BLOCK::StartStopSequence (src/tests.st) [0ms]
    PASS [3/3] TEST_PROGRAM::ComparisonAssertions (src/tests.st) [0ms]

    Failure summary:
      1. TEST_PROGRAM::LimitAddAndScaling @ src/tests.st:6
         ASSERT_EQUAL failed: expected 5, actual 7
         source: ASSERT_EQUAL(
    2 passed, 1 failed, 0 errors (279ms)
    Error: 1 ST test(s) failed
    exit 1
    ```

    The second assertion, which begins on line 6, expected 3 plus 4 limited to 5 and got 7. Put
    `LimitAdd := MaxValue;` back and run the tests again: `3 passed, 0 failed, 0 errors`, and
    `exit 0`.

## What happened

**Tests beside the code.** `trust-dev test` reads every `.st` and `.pou` file below the project
folder and takes each `TEST_PROGRAM` and `TEST_FUNCTION_BLOCK` as a test, named by its kind and
name. A production build of the same project leaves the tests out: they are entry points for the
runner, not part of the program a runtime runs, so a project keeps its tests beside its code.

**One cold start per test.** The runner compiles the project and its tests into one program image
for this computer, which is what the toolchain is for, and restarts that program cold before each
test: every variable takes its declared initial value again, so no test sees what another left
behind. A `TEST_PROGRAM` runs its body once, and so does a `TEST_FUNCTION_BLOCK`:
`StartStopSequence`'s four scans are its four calls of `Dut`.

**Passed, failed, error.** A test passes when its body runs to its end with every assertion
holding. A failed assertion ends its test at once as failed: without the limit, the `ASSERT_FALSE`
on line 21 fails too, but in step 7 the test had ended at line 6. The report gives the file and
line where the assertion begins, its first source line and its message, and `--output json` writes
the same results for a program, with the assertion's values typed: `INT` 5 expected and 7 actual
in step 7. Any other runtime error, such as a division by zero, ends the test as an error. The
other tests run anyway, and the command ends with exit code 1 when a test failed or ended in
error.

**The assertions.**

| Assertion | Holds when |
| --- | --- |
| `ASSERT_TRUE(IN)` | `IN` is `TRUE` |
| `ASSERT_FALSE(IN)` | `IN` is `FALSE` |
| `ASSERT_EQUAL(EXPECTED, ACTUAL)` | the two are equal |
| `ASSERT_NOT_EQUAL(EXPECTED, ACTUAL)` | the two differ |
| `ASSERT_GREATER(VALUE, BOUND)` | `VALUE > BOUND` |
| `ASSERT_LESS(VALUE, BOUND)` | `VALUE < BOUND` |
| `ASSERT_GREATER_OR_EQUAL(VALUE, BOUND)` | `VALUE >= BOUND` |
| `ASSERT_LESS_OR_EQUAL(VALUE, BOUND)` | `VALUE <= BOUND` |
| `ASSERT_NEAR(EXPECTED, ACTUAL, DELTA)` | `ABS(EXPECTED - ACTUAL) <= DELTA` |

## Try it yourself

- Filter for a name that no test has: `trust-dev test --project . --filter Pump` answers
  `0 tests matched filter "Pump" (3 tests discovered, all filtered out)` and ends with exit code 0.
- A test can live in any `.st` file of the project. Write one that divides by zero:

  ```bash
  cat > src/errors.st <<'ST'
  TEST_PROGRAM DivideByZero
  VAR
      Divisor : INT;
  END_VAR
  ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := INT#10 / Divisor);
  END_TEST_PROGRAM
  ST
  ```

  The run reports `ERROR [1/4] TEST_PROGRAM::DivideByZero src/errors.st:5` with
  `reason   : division by zero`, an error and not a failure, since no assertion failed, and
  `3 passed, 0 failed, 1 errors`. It runs first, as the tests run in the order of their files.
  Delete it with `rm src/errors.st`.
- A test checks only what it asks. In `src/main.st`, let the start win: write
  `IF StartCmd THEN Running := TRUE; ELSIF StopCmd THEN Running := FALSE; END_IF;` in place of the
  latch's `IF`. The three tests still pass, as none gives both commands at once. Write the test
  that does:

  ```bash
  cat > src/both.st <<'ST'
  TEST_FUNCTION_BLOCK StopWins
  VAR
      Dut : StartStopLatch;
  END_VAR
  Dut(StartCmd := TRUE, StopCmd := TRUE);
  ASSERT_FALSE(Dut.Running);
  END_TEST_FUNCTION_BLOCK
  ST
  ```

  It fails, `FAIL [1/4] TEST_FUNCTION_BLOCK::StopWins src/both.st:6` with
  `ASSERT_FALSE expected FALSE, got TRUE`. Put the latch's `IF` back, and `StopWins` passes.

## Reference

- Specification 22: "Source Discovery", "Test POU discovery", "Test case execution" and "Test
  reporting and CI output"; specification 07, "Non-IEC Extensions (MP-014)", the assertions.
- Tutorial 11, which tests a program's logic with mock I/O, and tutorial 21, which runs the tests
  in CI.
