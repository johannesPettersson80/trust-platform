### Fixed

- A `trust-dev` command whose output goes to a program that stops reading early, as `| head -1` does, no longer panics
  with "failed printing to stdout: Broken pipe": the rest of its output is dropped and the command ends as it would.
