### Added

- The example `room_heating` keeps a commissioning trace, `traces/commissioning.trusttrace`, 30 s of
  its start recorded on a computer, and a machine test that replays it, which CI runs on Linux,
  macOS and Windows. The guide "Machine Tests" shows how to record a trace, replay it with
  `trust-dev replay` or under `cargo test`, and read a difference.

### Fixed

- A machine test that fails on a trust-sim error prints the error's message as it reads, a failed
  build's messages and a replay's report line by line, where it printed them escaped on one line.
