### Added

- `trust-dev rust bench` measures a project's Rust functions, function blocks and methods on this
  computer, each case once a scan for 1,000 scans (`--scans`), and writes each one's longest call
  into `trust.toml` `[timing.measured]`, with the hash of the files the program is built from. A
  Rust POU takes the inputs it is measured with from `#[bench_case(...)]`, one attribute per case;
  without one it is measured at its declared initial values. The command names each POU whose
  longest call is over its budget, and each case that faulted, which measures nothing
  (specification 48 RU-48).
