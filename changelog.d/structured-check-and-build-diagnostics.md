### Changed

- `trust-runtime check --json` reports each compiler diagnostic as its own issue, with the
  compiler's code (`E101`), its severity and message, the file, and where it is: `line`, `column`,
  `end_line` and `end_column`, counted as the editor and `trust-lsp check` count them. Before, every
  diagnostic of a compile was one `compile` issue whose text named byte offsets.

### Added

- `trust-runtime build --json`. With `--json` or `--ci`, a build whose sources have errors prints
  the same issues as `check` instead of the error text.
