### Added

- `trust-runtime check` and `trust-runtime build` take `--sources` more than once.
  `--sources src --sources vendor/stubs` checks a project's sources together with vendor library
  stubs outside `src/`, without any other tree of `.st` files that the project folder holds. A
  file that two of the named folders hold is compiled once.
