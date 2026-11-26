### Fixed

- A Rust POU's own fault names its number and its two numbers in the runtime's faults, `events.tail`, the
  run log and machine tests, as `Rust fault 3 (2000, 2010)`. It said only `runtime_rust_fault`, so a
  sequence whose `.within` ran out named neither the wait point nor its times.
