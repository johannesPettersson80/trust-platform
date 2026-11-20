### Fixed
- Tutorial 11 (`examples/tutorials/11_unit_testing_102`) gives its level and pump-speed words bytes of their own,
  `%IW2` and `%QW2`. On `%IW0` and `%QW0` they shared byte 0 with the start and stop commands and the valves,
  so on the runtime writing the level 200 cleared the start command and the tank never filled.
