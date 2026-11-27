# Tutorial 21: CI for a PLC Project

Check a PLC project the way a CI server does: build, validate and test it in CI mode, read the JSON
and JUnit reports, see each kind of failure end with an exit code of its own, and copy the workflow
that runs the same steps on every change. It uses a copy of tutorial 11's tested project.

## What you learn

- `--ci` makes `trust-runtime build`, `trust-runtime validate` and `trust-dev test` write reports
  that a program reads and end with an exit code for each kind of failure: 10 for an invalid
  project or configuration, 11 for a build failure and 12 for a failed test.
- `trust-dev test --output junit` and `--output json` write the test reports that a CI server shows
  and keeps.
- The project CI template installs a pinned truST release, checked against its `SHA256SUMS`, and
  runs these steps on every change, so the project's repository needs no Rust.

## What you need

- This repository, whose `examples/tutorials/11_unit_testing_102` you copy, and `trust-runtime` and
  `trust-dev`, from the truST extension for VS Code or a release archive.
- Linux or macOS, whose shell runs the commands below.
- For step 7, a GitHub repository of your own for the project. About 20 minutes.

## Step by step

1. From the root of the repository, copy tutorial 11's project:

    ```bash
    rm -rf /tmp/trust-tutorial-21
    cp -R examples/tutorials/11_unit_testing_102 /tmp/trust-tutorial-21
    cd /tmp/trust-tutorial-21
    ```

    Its program is in `src/main.st`, and `src/tests.st` holds its two tests.

2. Build in CI mode and read the exit code:

    ```bash
    trust-runtime build --project . --ci
    echo "exit $?"
    ```

    The build report is JSON: among its members `"status": "ok"`, `"source_count": 2` and
    `"image": "./program.tpi"`. Then `exit 0`.

3. Validate in CI mode:

    ```bash
    trust-runtime validate --project . --ci
    echo "exit $?"
    ```

    The report names the project's resource and its I/O driver, `"resource": "TankControlRes"` and
    `"io_driver": "simulated"`, with `"status": "ok"`; then `exit 0`. Validation checks the program
    image too, so it follows the build.

4. Run the tests with a JUnit report:

    ```bash
    trust-dev test --project . --ci --output junit > /tmp/trust-tutorial-21-junit.xml
    echo "exit $?"
    ```

    It answers `exit 0`, and the report holds
    `<testsuite name="trust-runtime" tests="2" failures="0" errors="0" skipped="0">` with a
    `<testcase>` for each test, `TEST_PROGRAM::TankControlWithMockIo` and
    `TEST_FUNCTION_BLOCK::TankControlBand`, each with its file and line.

5. Write the same results as JSON:

    ```bash
    trust-dev test --project . --ci --output json > /tmp/trust-tutorial-21-report.json
    ```

    The report's `summary` holds `"passed": 2`, `"failed": 0`, `"total": 2` and the run's
    `duration_ms`, and `tests` lists each test with its `kind`, `file`, `line`, `status` and
    `message`.

6. Make each step fail, and read its exit code:

    - **A failed test.** In `src/tests.st`, change the first `ASSERT_EQUAL(EXPECTED := INT#800, …)`
      to `INT#900` and run step 4 again. It prints `Error: 1 ST test(s) failed` and `exit 12`, and
      the report's first test holds
      `<failure message="ASSERT_EQUAL failed: expected 900, actual 800">` with
      `at: src/tests.st:10`. Undo the change.
    - **A build failure.** Add the line `THIS IS NOT ST` at the end of `src/main.st` and run step
      2 again: the report has `"status": "failed"` and lists each issue with its `code`, `E002`,
      file and line, and the exit code is 11. Delete the line.
    - **An invalid configuration.** In `io.toml`, misspell the driver as `driver = "simulatd"`
      and run step 3 again: `Error: invalid config 'unsupported io.driver 'simulatd''` and
      `exit 10`. Put `simulated` back.

7. Run the steps on every change. Copy `.github/workflows/templates/trust-runtime-project-ci.yml`
   from this repository to `.github/workflows/trust-runtime-project-ci.yml` in the project's own
   repository, and set its `TRUST_VERSION` to the truST release the project is built with. Each
   push or pull request that changes `src/` or a `.toml` file at the project's root then installs
   that release, checks its archive against the release's `SHA256SUMS`, installs its toolchain,
   runs steps 2 to 4, and keeps the JUnit report as the artifact `st-tests-junit`, a failing run's
   too. On a Linux computer, the template's steps with release 0.44.1 printed
   `trust-runtime-linux-x64.tar.gz: OK`, downloaded and checked the 222 MB toolchain, and built,
   validated and tested this project.

## What happened

**CI mode.** With `--ci` each command writes its report for a program to read, and its exit code
tells the kind of failure. Without it the commands write for a person: `trust-dev test` lists each
test and a failure summary, and a failed test ends with exit code 1.

**The exit codes.** `trust-runtime` and `trust-dev` share the codes: 0 for success, 10 for an
invalid project or configuration, 11 for a build or compile failure, 12 for a failed test, 13 for a
timeout and 20 for an internal failure. A pipeline can stop at the first step that fails and still
tell a broken configuration from broken code.

**The order.** The build writes the program image that validation checks, beside the
configuration files and the I/O drivers, and the tests run last. A failure stops the pipeline where
its cause is.

**The reports.** The JUnit report is what CI servers show as test results, test by test, with a
failed assertion's message and line. The JSON report carries the same results for checks of your
own, such as a dashboard or a rule on durations.

**The template.** It installs truST from a release instead of building it, so the project's
repository holds only the project. `TRUST_VERSION` pins the release, so CI builds with the same
truST as the engineer's computer; the check against `SHA256SUMS` makes sure the archive is the one
released. The report is uploaded even when a step failed, which is when it is needed.

## Try it yourself

- Run step 4 without `--ci` while the assertion is broken: `trust-dev test --project .` prints
  `FAIL [1/2] TEST_PROGRAM::TankControlWithMockIo src/tests.st:10` with the reason and source, a
  failure summary and `1 passed, 1 failed, 0 errors`, and ends with exit code 1 instead of 12.
- Misspell a key of `runtime.toml`, `cycle_intervall_ms` for `cycle_interval_ms`, and run step 3
  again: validation points at line 6 with
  ``unknown field `cycle_intervall_ms`, expected one of `name`, `cycle_interval_ms`, …`` and ends
  with exit code 10.
- Check a release archive as the template does, in an empty folder:

  ```bash
  base=https://github.com/johannesPettersson80/trust-platform/releases/download/v0.44.1
  curl -fsSLO "$base/trust-runtime-linux-x64.tar.gz"
  curl -fsSLO "$base/SHA256SUMS"
  expected=$(awk '$2 ~ /(^|\/)trust-runtime-linux-x64\.tar\.gz$/ { print $1 }' SHA256SUMS)
  echo "$expected  trust-runtime-linux-x64.tar.gz" | sha256sum --check --strict -
  ```

  It answers `trust-runtime-linux-x64.tar.gz: OK`. Append a line to the archive,
  `echo changed >> trust-runtime-linux-x64.tar.gz`, and run the last two lines again:
  `trust-runtime-linux-x64.tar.gz: FAILED`.

## Reference

- Specification 22, "Developer CI exit classification" and "Project CI template"; specification 44
  §4.6, the toolchain and its installation.
- Tutorial 11, which wrote the tests, and the testing pages of the documentation.
