### Added

- A service's bindings: `trust-dev service bindings <name> --out <file>` writes a Rust module, from the
  image the project's build wrote, that names each variable of the program holding one value with its
  Rust type, so that a service writes `plant::P1::level` and the compiler checks its paths and types.
  Only the variables the service's `writes` lists are writable handles, which `Service::set` alone
  takes (specification 48 RU-62).
