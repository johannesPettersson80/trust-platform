### Fixed

- The runtime cloud quickstart starts again: its dev profiles name their resources `runtime_a` and `runtime_b`,
  identifiers that the program's `RESOURCE` can carry, and the steps copy each profile into an empty folder before
  the first start, so that `play` writes the program for the profile's resource. The profiles' `runtime-a` and
  `runtime-b` could never match a program's `RESOURCE`, and the build refused both runtimes.
