### Fixed

- `trust-dev test --device`, `trust-dev rust bench --device` and the conformance suite's device runs
  install their programs with an event store of 16 events, so a chip keeps the debugger's buffer of
  16 stops beside them, about 1.4 KB, instead of its default of 200, 17,860 bytes on the ESP32-C6.
  With Wi-Fi on, a C6 refused a test program of over about 4.7 KB.
