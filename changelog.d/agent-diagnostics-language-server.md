### Fixed

- `trust-dev agent serve` reports a project's diagnostics as the language server does. `lsp.diagnostics` and
  `runtime.compile_reload` now give the same findings as `trust-lsp check`, from the project's `trust-lsp.toml`:
  vendor profile, include paths, libraries and severity overrides. Before, they analyzed the project's folder under
  the IEC rules without its libraries, so a CODESYS project showed narrowing stores as errors and every symbol of a
  library elsewhere as undefined (#236).

### Added

- `lsp.diagnostics` of the agent API takes `page` and `page_size` together to bound the listed issues; a page
  reports `page`, `pageSize`, `totalIssues` and `totalPages` (specification 20).
