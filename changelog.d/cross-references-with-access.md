### Added

- The `trust-lsp.crossReferences` command lists a symbol's declaration and references, each with
  its access (`declaration`, `read`, `write` or `call`) and the POU it sits in, as CODESYS's
  cross-reference list does, for a client with or without an editor (specification 14 §6.3.2).

### Fixed

- Find references and document highlights tell reads from writes by W012's rule (specification
  28 §6): an output binding (`=>`) and an in-out argument are writes, where they showed as reads,
  and an index of an assigned element is a read, where it showed as a write.
