### Fixed

- Tutorial 18 walks through a simulation step by step, with the values of a real run: a coupling that plays a lamp's
  feedback contact 3 s after the lamp, disturbances that press and release the start button, a scheduled fault, and
  the same run at ten times the speed. It no longer says that a `simulation.toml` needs `--simulation`, which only
  turns on a file that says `enabled = false`, and no longer offers a "pre-hardware gate" that ran no test. CI runs
  its schedule on the simulator's clock and checks its Try-its.
