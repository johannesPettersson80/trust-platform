### Changed

- Files inside a `[[libraries]]` folder or a resolved `[dependencies]` package no longer receive
  the unused-symbol warnings W001, W002, and W009, in the editor and in `trust-lsp check` alike:
  a library is API surface that an application only uses, and it is judged in its own project,
  where its files are that project's own sources and every warning is on. Every other diagnostic
  stays on for library files. A library entry whose folder is the project root, or contains it,
  is refused with the configuration diagnostic `L008` and exempts nothing (specifications 14
  §7.6, 28 §1).
