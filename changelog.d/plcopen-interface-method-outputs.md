### Fixed

- The PLCopen XML import keeps the `VAR_OUTPUT` section of an interface's methods (#309). It
  dropped them, so every function block that implements such an interface, declaring the outputs as
  CODESYS does, was refused with E208, and every call that passes the outputs with E204.
