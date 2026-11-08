### Fixed

- Control Studio's setup, the HMI Builder's variable list and the language server's other commands
  that compile the project now compile it under its `vendor_profile`, as its build does
  (specification 14 §6.6.2). A CODESYS project whose function block method has no access
  specifier, which CODESYS treats as `PUBLIC`, was refused there under the IEC rules.
