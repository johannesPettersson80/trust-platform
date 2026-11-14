### Fixed

- The Linux x64 packages start again on Linux with an older glibc. From 0.35.0 to 0.41.1, `trust-runtime` and
  `trust-dev` (in 0.41.1 also `trust-debug`) needed glibc 2.43 and failed on Ubuntu 24.04 and Debian 13 with
  ``version `GLIBC_2.43' not found``, and `trust-lsp` needed glibc 2.38, which Debian 12 and Ubuntu 22.04 lack. The
  Linux x64 binaries are now built as the Linux arm64 ones are, and run on every Linux with glibc 2.28 or newer; a
  release refuses a Linux binary that needs a newer glibc (specification 24).
