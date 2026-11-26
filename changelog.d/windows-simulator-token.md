### Fixed

- Debugging in VS Code on Windows holds at breakpoints and shows the program's frames and variables. The simulator that
  a launch started listened on a TCP port that requires a control token, and the launch gave it none, so it refused
  every request, the debug adapter's own included. The language server now names a loopback port per workspace and a
  new token on Windows, as it names a socket and a token on Linux and macOS, and the debug adapter takes a new token
  for a TCP endpoint that a launch names without one.
