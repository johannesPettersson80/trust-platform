### Fixed

- Clients that send a running runtime a new request as soon as one is answered no longer hold its cycles back. Between
  two cycles the scan thread serves the commands queued when it begins to serve them, at most 1,024, and a command
  queued meanwhile waits for the next boundary. Before, it served commands until the queue was empty, so a steady
  stream of requests, such as three readers of the program's variables on a busy Windows machine, could keep any cycle
  from running (specification 11 §6.9).
