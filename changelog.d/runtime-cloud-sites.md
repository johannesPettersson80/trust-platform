### Added

- `[runtime.cloud] site` names the site a runtime belongs to. A runtime names the runtimes of its own
  site by their resource names and those of another site by `<site>/<resource>`, such as
  `site-b/runtime_b`, so a project keeps its names when its runtimes are given a site. Discovery
  announces the site, and the Runtime Cloud state, the action requests and the WAN allowlist use these
  IDs. Before, no runtime could name a site, so the federation guide's `site-b/runtime-b` IDs and the
  `site-b/*` allowlist rules could never match a real runtime (specification 11, "Runtime Cloud
  sites").

### Changed

- Whether a Runtime Cloud target is cross-site follows the runtime's own site and the site of the
  target's ID; the actor's SPIFFE site no longer stands in for the runtime's site.
