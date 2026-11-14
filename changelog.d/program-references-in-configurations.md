### Fixed

- Renaming a program renames it in the configuration that runs it (`PROGRAM P1 WITH Cycle : MainProgram;`),
  where before it left the configuration naming the old program, so the project no longer built. Find
  references lists the configuration, and the reference count above the program counts it (specification
  14 §6.3.2).
