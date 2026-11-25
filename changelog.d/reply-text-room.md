### Fixed

- A runtime whose heap has no room for a reply's result text answers `no room for a reply longer
  than <n> bytes` instead of aborting. `status` and the I/O reads wrote their text into a block
  that grew and then shrank without asking the heap whether it had room: on an ESP32-C6 with Wi-Fi
  on, a `status` after the reference workload's install, or after an online change while a cycle
  trace was kept, found no block of 4 KiB and reset the chip (specification 34 §7.4).
