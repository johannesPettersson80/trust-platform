### Changed

- W012 follows CODESYS's rules for globals that tasks share (#294). It warns when two or more tasks
  write a global, or when one task writes data that one access does not cover (a string, an array,
  a structure, a function block instance or a 64-bit value, `TIME` included, which truST stores in
  eight bytes) and another task reads it. A global that one task writes and others read is silent
  when its value is 32 bits or less. `{attribute 'cross_task'}` before a global, or before its
  `VAR_GLOBAL` list, declares it shared by design and silences W012 for it.
