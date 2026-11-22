### Fixed

- After the safe state of a stop or a fault, `io.read`, `io.list` and Live Values show the outputs as the safe
  state drove them, marked `"safe_state": true` with the number of the last scan that completed, and the event
  `safe_state_applied` records each output with its value, on a computer and on a board alike (specification 11,
  "Safe state in the reads"). They showed the last scan's outputs, and nothing recorded a safe state that
  succeeded, so a safety commissioning step could not see it.
- Tutorial 19 sets `[runtime.fault] policy = "safe_halt"`, the policy that applies the safe state on a program
  fault, and verifies the safe state through Live Values, `ctl io-read` and `ctl events`. Under its former `halt`
  a program fault halts without the safe state that the tutorial expected.
