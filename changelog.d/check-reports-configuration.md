### Fixed

- `trust-lsp check` reports the diagnostics of the project's `trust-lsp.toml`, as the editor shows them:
  a broken library or dependency configuration (`L001`–`L007`) now fails the check. Before, the check
  looked only at the source files, so a CI gate passed a configuration the editor marked as an error
  (specification 14 §7.7, issue #213).
