### Fixed

- An HMI agent simulator journey no longer reports a step as observed when its readback path is one
  the running program does not have, which the runtime answers as unknown; the step now times out
  (specification 43, AT-08).
- An HMI agent simulator journey step that commands a control without a confirmation, such as a
  button without feedback, is observed once the runtime applies the command; it always timed out
  before (specification 43, AT-08).
