### Added

- `[runtime.cloud] site` names the site a runtime belongs to. Outside the default site its Runtime
  Cloud ID is `<site>/<resource>`, such as `site-b/runtime_b`. Discovery announces the site, and the
  Runtime Cloud state, the action requests and the WAN allowlist name the runtime by that ID.
  Before, every real runtime was in the default site, so the federation guide's
  `site-b/runtime-b` IDs and the `site-b/*` allowlist rules could never match a runtime
  (specification 11, "Runtime Cloud sites").

### Changed

- Whether a Runtime Cloud target is cross-site follows the sites of the runtimes' IDs alone: a
  bare ID is a runtime of the default site, and the actor's SPIFFE site no longer stands in for the
  site of the runtime the request is connected through.
