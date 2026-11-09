### Fixed

- `trust-dev test` labels each source file by its path relative to the project folder, as
  `trust-runtime build` does: a compile error of the tests names `src/tests.st`, and the test image
  holds no path of the computer that built it.
