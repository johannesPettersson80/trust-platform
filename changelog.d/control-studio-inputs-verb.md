### Added

- `trust-control-studio inputs --loop <id> --change <file>` records one change of a loop's study inputs
  from the command line, such as the requirements Tune designs to, as Control Studio's editors do
  (specification 35 §0.1b). `--change -` reads the change from standard input. Until now only the
  editors, through `serve`, could set them.
