### Fixed

- `trust-lsp` no longer writes a line to its log, the language server's output in an editor, for
  every analysis query it runs: Salsa's query events are logged only when `RUST_LOG` names
  `salsa` (specification 14 §7.10). Ten edited documents wrote about 5,600 such lines.
