### Fixed

- Control Studio reads a loop's signals from a global of a namespace, the form of a CODESYS GVL
  member, by its qualified name (`GVL_IO.lrLT101_Level`), and from a global scalar by its own name.
  Before, setup offered them and the first read refused them with `program instance not found`.
- A simulated plant can be wired to such a variable in `simulation.toml` (`command_sources`,
  `feedback_targets`), as to a program's.
