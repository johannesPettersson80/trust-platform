### Fixed

- `trust-dev test` names each test's file relative to the project folder in every format, JSON,
  TAP, JUnit and the human output, as `trust-lsp check` does: `src/tests.st`, never the canonical
  whole path with its `\\?\` prefix on Windows, also for a relative `--project`.
