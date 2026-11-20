### Fixed
- Example projects no longer map their command bits into the bytes of their analog words: in the
  OSCAT examples, `filling_line` and `plant_demo` the words move up two bytes, so writing a level or
  a speed no longer rewrites the start and stop commands, and `filling_line`'s scenario table works
  as written.
- The OSCAT examples' integration maps list the I/O their code binds, and their MQTT inputs keep the
  last value between messages instead of faulting the runtime at its first scan.
