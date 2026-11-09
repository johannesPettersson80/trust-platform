### Changed

- `trust-runtime build`, `play` and `run` no longer download the toolchain silently when a build
  needs it and none is installed. In a terminal they ask first, naming where the bundle comes from,
  its size (about 180 to 210 MB, 630 MB installed) and its folder; without a terminal, as in a
  script or CI, they fail with the advice to run `trust-runtime toolchain install`, unless
  `--install-toolchain` gives the consent. The VS Code extension installs the toolchain as before.
