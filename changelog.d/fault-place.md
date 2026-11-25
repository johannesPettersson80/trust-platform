### Added

- A fault that the program raised names where: `faults`, `events.tail` and a simulator's advance give
  its POU, its source file and the statement's line (`"pou"`, `"file"`, `"line"`) when the program is
  a debug build, on a computer and on a chip alike, as long as that program runs; and a machine
  test's `trust_sim::Error::Fault` reads `division by zero in Main at src/Main.st:17` (specification
  11, the fault entry; specification 34 MEM-05).
