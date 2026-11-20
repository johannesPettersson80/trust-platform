### Fixed
- The ADS server example (`examples/communication/ads_server_basic`) keeps the `global.Setpoint` a
  client writes: its program ramped that value every scan, so the write-and-read-back proofs its
  README teaches (the pyads smoke, the TwinCAT program, the ADS.NET notification) all failed. The
  README now uses the released `trust-runtime` commands, says that the UDP identify responder
  listens on every interface, and shows what the Doctor prints.
