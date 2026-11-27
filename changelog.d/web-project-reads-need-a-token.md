### Security

- With `[runtime.web] auth = "token"`, the web interface no longer serves the project's sources or its setup's
  defaults without a token: `GET /api/program`, `GET /api/source` and `GET /api/setup/defaults` now require the
  Viewer role, as the other reads of the project do. Before, any client that reached the web port could list and
  read the Structured Text sources in `src` and the setup's defaults, among them the project's folder, resource,
  cycle and first driver. Specification 11 §6.9.7 now names what answers without a token: the pages and their files,
  the pairing claim, the interface's mode and the QR code route.
