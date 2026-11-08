### Added

- A `STRING` variable marked `{attribute 'instance-path'}` and `{attribute 'noinit'}` in a function
  block, class or program marked `{attribute 'reflection'}` starts with the path of its instance, as
  CODESYS initializes it: `Plant.Line1.Cell.pumps[2]`, the configuration's and the resource's names
  first where the project declares them.
