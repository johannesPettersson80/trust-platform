### Changed

- Deployment records are version 4: section 10 carries `[runtime.trace] flight_recorder`, the
  flight recorder's cycles, for a chip, which never reads `runtime.toml`. A runtime or board
  firmware of an earlier release refuses a record of this release with its reason: install this
  release's firmware on a board with `trust-runtime firmware install` (specification 34 §5.1.1).
