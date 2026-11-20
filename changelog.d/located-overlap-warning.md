### Added
- A warning, W017, for located variables whose process-image storage overlaps, such as `%IX0.0` inside `%IW0`, or
  `%IW0` and `%IW1`, which share byte 1 (an address is a byte offset): writing one changes the other. It is reported
  on each of them, for `AT` declarations and `VAR_CONFIG` entries across a project's files, and names the others.
  Two declarations of the same address and size, as two programs reading one input, are not reported.
