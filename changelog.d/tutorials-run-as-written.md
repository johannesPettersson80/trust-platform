### Fixed
- Tutorial 13 ("Bootstrap a PLC Project from Zero to First Running App") builds and runs as written:
  its `runtime.toml` no longer holds the empty `auth_token = ""`, which the runtime refuses, and it
  takes the `simulated` I/O driver, which keeps each input as you set it, so step 9's inverted
  output can be seen. Tutorials 14, 18 and 19 start from its project.
- Tutorial 12's PID process dashboard takes the `simulated` driver: with `loopback`, its outputs
  overwrote the commands it reads, and the bypass never opened.
- Tutorial 18's `simulation.toml` and the example of the `simulation.toml` reference page load: their
  coupling no longer sets `on_true` and `on_false` without a `threshold`, and the reference example's
  coupling, physics joint and plant no longer write the same input bytes.
- Tutorial 10's filter example selects its test (`--filter StartStop`).
- Tutorials 21 and 22 build and validate a runnable project: tutorial 11 has a `runtime.toml` and an
  `io.toml`, and tutorial 22 no longer passes a `--sources` folder that does not exist.
