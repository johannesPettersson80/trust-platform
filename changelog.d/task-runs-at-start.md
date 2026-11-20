### Changed

- A periodic task runs at its start, then at every interval: the first cycle after a program
  starts, restarts or starts again after a stop runs every task with an `INTERVAL`, as the schedules
  of IEC 61131-3 Ed.3 §6.8.2 do. Its first run came one interval later before, so that a task of
  1 s first ran a second after the start. Computers and boards alike (specification 10 §4.3).
