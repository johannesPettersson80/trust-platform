### Fixed

- A runtime that ends cleanly (Ctrl+C, SIGINT, SIGTERM or `ctl shutdown --exit`) now removes its Unix
  control socket as well as its endpoint file, so no `control.sock` or fallback socket stays behind
  (specification 11 §6.8.0a). A socket that another runtime has bound at the same path since is kept.
