### Fixed

- The Runtime Cloud federation guide runs as written. It continues the quickstart's two projects as
  `runtime_a` of `site-a` and `runtime_b` of `site-b`, with a site CA, one token and HTTPS, and
  walks through the cross-site deny, the allowlist rule, the dispatch and its audit ID. Its payloads
  and the example pack name runtimes as specification 11 does (`site-b/runtime_b`). The new
  profiles `runtime-a-wan.toml` and `runtime-b-wan.toml` and the payload
  `dispatch-cross-site-cfg-apply.json` go with it.
- `examples/runtime_cloud/runtime-plant.toml` and `runtime-wan.toml` load and build. They name
  resources a program can declare (`runtime_plant`, `runtime_wan`) instead of `runtime-plant` and
  `runtime-wan`, which no build accepted, and they hold the control token placeholder that their
  token authentication needs: without it, loading them failed with `runtime.web.auth=token
  requires runtime.control.auth_token`. They name their site and trust the site CA.
- The profile cookbook and the troubleshooting guide reach `plant` and `wan` runtimes over HTTPS
  with the token, and the troubleshooting guide no longer sends users to the repository's test
  scripts.

### Removed

- `docs/guides/RUNTIME_CLOUD_UI_WALKTHROUGH.md`, which described a fleet web page that does not
  exist.
