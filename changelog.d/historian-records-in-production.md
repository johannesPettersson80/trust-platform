### Fixed
- The historian records with debugging off (`runtime.control.debug_enabled = false`, the setting of
  a new project and of production). It read only the debugger's snapshot, which does not exist then,
  so it recorded nothing and `/metrics` reported no samples. It now reads the scan's published copy
  of the program, as the OPC UA server does.
- Tutorial 23's historian allowlist names the program's variables by their full paths,
  `P1.StartCmd` and `P1.LampOut`; `StartCmd` and `RunLamp` named nothing.
