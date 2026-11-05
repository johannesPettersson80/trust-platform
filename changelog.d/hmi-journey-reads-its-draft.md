### Changed

- An HMI agent simulator journey reads its draft in the language server. A draft at another revision, or a program
  that changed since the draft was composed, ends the journey before its first step with `CONFLICT` in its answer, and
  a program that does not compile with `COMPILATION_FAILED`; the journey no longer loads the project's render context
  first (specification 43, AT-08).
