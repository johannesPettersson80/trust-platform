### Fixed

- On Windows, the build of a project with Rust POUs no longer fails when the project's folder is given as a
  canonical path (`\\?\D:\...`), as trust-sim gives it to a machine test: cargo refused the path that the build
  wrote into its package's manifest as an "invalid path url".
