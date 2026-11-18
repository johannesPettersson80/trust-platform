### Added

- A program image built from a project with a Rust crate lists the crates its module holds, each
  with its version, where it comes from and the checksum `Cargo.lock` pins for it, in a table that
  the image's signature covers, so a deployed program says which Rust code it holds. Its deployment
  record carries the same list, and `build/build.json` gives it as `crates` (specification 44 §5.12,
  specification 48 RU-79).

### Changed

- Program images are format version 5 and deployment records version 3. A runtime or board firmware
  of an earlier release refuses them with its reason: install this release's firmware on a board
  with `trust-runtime firmware install`.
