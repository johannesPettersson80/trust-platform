### Fixed

- A runtime started on a program store's `current` link now keeps its endpoint file, its default
  control socket and its runtime ID in the store folder, not in the bundle it started on. After an
  install, a deployment or a rollback moved `current` to another bundle, `trust-runtime ctl` and the
  other clients given `current` no longer found the running runtime: on Windows, whose default
  control endpoint takes a port the system chooses, `ctl` reported a refused connection to port 0.
  The next start also gave the runtime a new runtime ID.
