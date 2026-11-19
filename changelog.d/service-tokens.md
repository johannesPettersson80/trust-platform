### Added

- Services sign in with a service token of their own: `trust-runtime ctl service-token <name> --out
  <file>` creates it with the runtime's Admin access and writes it into the file, which only its owner
  reads, never to the terminal. A service token is a Viewer unless `--role` asks for Operator or
  Engineer, does not expire, so that a service running unattended keeps its connection, and is
  replaced by running the command again; `pair.list` names its service, and `pair.revoke` revokes it.
  Computers and boards alike (`pair.service`, specification 11 §6.9.6; specification 48 RU-81).
