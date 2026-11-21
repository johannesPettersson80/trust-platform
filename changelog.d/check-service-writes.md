### Fixed

- Compile (`trust-runtime check`) checks the variables a project's services may write
  (`[services.<name>] writes` in `trust.toml`) as the build does, without the toolchain: a path that
  no write may reach, a constant, an instance, a structure or a variable two services write now
  fails Compile with the build's words, instead of passing it and failing only when the project
  starts.
