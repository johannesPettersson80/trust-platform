### Removed

- The bytecode engine's built-in import contracts, with the stable error code
  `runtime_import_unsupported`. A compiled program image imports nothing, because the program
  library is compiled into it, so a device checks no import contract; a deployment record that
  lists an import is refused as a record of another program (specification 34 §5.2.1).
