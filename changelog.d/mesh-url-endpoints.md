### Fixed
- A mesh `connect` or `listen` entry in the URL form `tcp://host:port` or `tls://host:port`, the form
  of a control endpoint, works: it was handed to Zenoh as it was, Zenoh refused it, and the runtime
  ran on without a mesh and without saying so. A mesh that does not start now logs the warning
  event `mesh_unavailable` with the reason.
