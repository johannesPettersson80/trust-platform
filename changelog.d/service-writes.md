### Added

- A service writes the program's variables that its project allows. `[services.<name>] writes = [...]`
  in `trust.toml` lists them; the build checks each path and carries the list in the signed program
  image, so a board holds the service to it as a computer does. The control request `variables.write`,
  sent with the service's Engineer service token, writes a set of them together at the next cycle's
  start, as the program's input and never as a force; a path outside the list, or a value its
  variable's type does not take, writes nothing. `trust-runtime ctl write <path>=<value>...` sends it
  (specification 11 §6.8.1, "Writing variables"; specification 44 §5.14; specification 48 RU-63).
