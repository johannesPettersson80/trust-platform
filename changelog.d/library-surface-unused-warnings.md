### Changed

- Library files no longer receive the unused-symbol warnings W001, W002, and W009, in the editor
  and in `trust-lsp check` alike: a library is API surface that an application only uses, and it
  is judged in its own project, where its files are that project's own sources and every warning
  is on. For each file, the deepest folder that holds it among the project's own source folders
  (its `include_paths`, or the project folder) and its library folders (`[[libraries]]`,
  `[project] library_paths`, and resolved `[dependencies]` packages) decides, and an own folder
  wins over an equally deep library folder. A dependency outside the project folder, as in the
  OSCAT examples, is diagnosed with the project's settings. Every other diagnostic stays on for
  library files (specifications 14 §7.6 and §7.6.4, 28 §1).
