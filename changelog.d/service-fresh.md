### Added

- The program library's function block `SERVICE_FRESH` tells a program whether a service's
  heartbeat moved within a timeout: `fresh(HEARTBEAT := svc_heartbeat, TIMEOUT := T#1s)` gives
  `FRESH` and `AGE`, the task time since the heartbeat last changed. Its first call, after a start or
  a restart, counts no change, so a heartbeat a restart left reads as not fresh until the service's
  next one (specification 08 §5A; specification 48 RU-63). The `iec` stdlib profile does not offer
  it, as it is truST's own block.
