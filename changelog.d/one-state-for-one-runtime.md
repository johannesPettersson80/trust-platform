### Fixed

- The status bar no longer says a runtime is connected while the side panel says "Not connected" for
  the same runtime. A runtime that answers without a debug session is "Not connected" on both until
  Connect attaches one, and a runtime that keeps debugging disabled is "Debugging disabled" in the
  side panel too, as in the status bar and Live Values (specification 25 §5.1, §6.9).
