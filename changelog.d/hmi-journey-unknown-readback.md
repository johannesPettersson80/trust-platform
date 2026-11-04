### Changed

- An HMI agent simulator journey no longer takes a method command (`invoke`) as a step, stops
  before its next command when the target does not say it runs in simulation, and is refused while
  the language server trace logs each message's parameters (`verbose`, or the JSON trace format),
  which would log the named session's cookie. Changing the selected run target ends the simulator
  sessions (specification 43, AT-08).

### Fixed

- An HMI agent simulator journey no longer reports a step as observed when its readback path is one
  the running program does not have, which the runtime answers as unknown; the step now times out
  (specification 43, AT-08).
- An HMI agent simulator journey step that commands a control without a confirmation, such as a
  button without feedback, is observed once the runtime applies the command; it always timed out
  before (specification 43, AT-08).
- An HMI agent simulator journey step waits for its control's own confirmation time, so a control
  that confirms later than 5 seconds is no longer reported as timed out, and it ends with the
  runtime's own outcome and reason when the runtime reports the command timed out, refused or
  unknown (specification 43, AT-08).
