### Fixed

- A `trust-runtime` command whose output goes to a program that stops reading early, as `| head -1` does, no longer
  panics with "failed printing to stdout: Broken pipe". The rest of its output is dropped and the command ends as it
  would: a listing completes, a deploy completes, and a running runtime keeps running when the reader of its log lines
  goes away.
