# OSCAT consumer

Use the OSCAT library from a project of your own: take it as a dependency, call its functions and
function blocks, and check their results with Structured Text tests. You learn which of OSCAT's
constants need loading before you use them, and how a test shows it.

## What you learn

- A project takes OSCAT through `[dependencies]` in `trust-lsp.toml`, with the library's folder and
  version. The build and the editor find the library there; nothing is copied into the project.
- OSCAT's scalar constants, `MATH.PI` and `PHYS.T0` among them, have their values from the start.
  Its tables of names, the months, weekdays and compass points, and `MATH.FACTS` stay empty until
  `OSCAT_BASIC_Constants()` fills them, so a program calls it once at its start, as `Main` does.
- `trust-dev test` runs the `TEST_PROGRAM`s beside the program, each after a cold restart, and
  names each assertion that fails with its expected and its actual value.

## What you need

- `trust-runtime` and `trust-dev`, from the truST extension for VS Code or a release archive, and
  this repository, whose `libraries/oscat` the example depends on.
- About 10 minutes.

## Step by step

1. Open a terminal at the root of the repository and build the project:

    ```text
    trust-runtime build --project examples/oscat_smoke --sources src
    ```

    It writes the program's image, here on an x86-64 Linux computer, and then names the image's
    memory figure, the key that signed it and its 27 sources, the example's three and OSCAT's 24:

    ```text
    Wrote examples/oscat_smoke/program.tpi
    Wrote examples/oscat_smoke/build/install.x86_64-unknown-linux-gnu.bin for trust-runtime ctl install
    Debug image 491716 bytes: code 324525, read-only data 8309, starting memory 7392
    ```

2. Open `trust-lsp.toml`. Its `[dependencies]` entry takes the library from the repository's
   `libraries/oscat` folder:

    ```toml
    [project]
    include_paths = ["src"]
    stdlib = "iec"

    [dependencies]
    OSCAT = { path = "../../libraries/oscat", version = "0.1.0" }
    ```

3. Open `src/Main.st`. Its first statement loads the constants, and the rest calls OSCAT's helpers
   and function blocks each scan, from temperature and speed conversions to dates, text and the
   parsing of numbers:

    ```st
    ConstantsReady := OSCAT_BASIC_Constants();
    Kelvin := C_TO_K(Celsius := Celsius);
    Fahrenheit := C_TO_F(celsius := Celsius);
    ```

4. Run the tests of `src/Tests.st`:

    ```text
    trust-dev test --project examples/oscat_smoke
    ```

    The four tests pass; the time differs from run to run:

    ```text
    Running 4 ST test(s) in examples/oscat_smoke
    PASS [1/4] TEST_PROGRAM::OscatConvertsTemperaturesAndSpeeds (src/Tests.st) [0ms]
    PASS [2/4] TEST_PROGRAM::OscatNamesMonthsAndDirectionsOnceItsConstantsAreLoaded (src/Tests.st) [0ms]
    PASS [3/4] TEST_PROGRAM::OscatReadsCalendarDates (src/Tests.st) [0ms]
    PASS [4/4] TEST_PROGRAM::OscatFormatsBytesAndText (src/Tests.st) [0ms]
    4 passed, 0 failed, 0 errors (13168ms)
    ```

## What happened

**The dependency.** The build and `trust-dev` read `[dependencies]` and compile OSCAT's sources
with the project's own, so the program calls `C_TO_K` or `EASTER` as if it had declared them.
`include_paths = ["src"]` holds the project's sources, and the build's `--sources src` names the
same folder.

**The constants.** `C_TO_K` subtracts `PHYS.T0`, absolute zero in degrees Celsius, which the type
of `PHYS` gives the value -273.15 from the start. `MONTH_TO_STRING`, `DEG_TO_DIR` and
`DIR_TO_DEG` read the tables that only `OSCAT_BASIC_Constants()` fills, in three languages:
English is 1, German 2.

**The tests.** Each test checks values worked out by hand: 25 °C are 298.15 K and 77 °F; 36 km/h
are 10 m/s, which is Beaufort 5; 29 February 2024 is day 60 of a leap year, and Easter Sunday 2026
is 5 April; 165 is `A5` in hexadecimal and `10100101` in binary. The tests need no resource and
no I/O, so the same pattern checks the OSCAT calls of a project of your own.

## Try it yourself

- Delete the two lines `Ready := OSCAT_BASIC_Constants();` and `ASSERT_TRUE(IN := Ready);` from
  `OscatNamesMonthsAndDirectionsOnceItsConstantsAreLoaded` and run the tests again. Each test
  starts cold, so the table of month names is empty, and the test fails with
  `ASSERT_EQUAL failed: expected 'April', actual ''`. The temperature test still passes, as
  `PHYS.T0` has its value from the start.
- Add `ASSERT_EQUAL(EXPECTED := 'Mai', ACTUAL := MONTH_TO_STRING(MTH := INT#5, LANG := INT#2,
  LX := INT#0));` to the months test and run the tests again: language 2 is German, and the test
  passes.

## Reference

- The OSCAT manual in the documentation (Develop, Libraries, OSCAT) and
  `docs/guides/OSCAT_LIBRARY_GUIDE.md`, which list the helpers and function blocks.
- Specification 22, the test POUs and `trust-dev test`.
- `trust-lsp.toml`'s `[dependencies]`, in the reference of the documentation.
