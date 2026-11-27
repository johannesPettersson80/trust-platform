### Fixed

- The MQTT traffic-light example's README walks through a run against a broker step by step, with the values of a real
  run: the three lamps published by name at every cycle, in the program's phase order, with no I/O address in the
  project. Its Try-its come from real runs: a mapping that publishes the phase number, a tag the program does not
  declare, which `validate` passes and the start refuses, a lamp given a safe state through an address, and a longer
  phase. CI runs the steps against a test broker.
