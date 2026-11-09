### Fixed

- `trust-dev test` reports a failed test at the statement that failed: the assertion's file, line
  and source line, also when it lies in a function or function block the test calls, on this
  computer and on a device. The human output and TAP name that statement; JSON adds a `failure`
  object (`file`, `line`, `source`) and JUnit an `at:` line beside the test's own file and line.
  Before, every report named the test's declaration.
- A fault raised after a call returns, in the statement that made the call, such as a failed
  assertion on a function's result, is recorded at that statement, not at the called function's
  last statement.
