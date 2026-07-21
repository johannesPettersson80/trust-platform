# Phase 17 Browser Asset Proof — 2026-07-21

Scope: browser-visible regression proof for consolidating shipped HMI/IDE
JavaScript fragments into complete parser-visible source modules. This evidence
does not promote an invariant or close a specification gap (`proof_kind =
"none"`).

Validation checkout:
`/home/johannes/projects/trust-platform-p17-validation` on `trust-builder`,
branch `plc-verification-program`, baseline HEAD
`0f67f9609b6216154e87454c0ebd148390758e72`, with the local Phase 17 worktree
synced and bootstrap hashes verified before the build.

Focused build and source/fallback lock:

```text
CARGO_TARGET_DIR=$HOME/.cache/codex-targets/trust-platform-gate \
  cargo test -p trust-runtime --lib \
  web::ui_routes::tests::embedded_hmi_module_fallbacks_match_complete_source_modules \
  -- --exact
CARGO_TARGET_DIR=$HOME/.cache/codex-targets/trust-platform-gate \
  cargo build -p trust-runtime --bin trust-runtime
```

Rendered HMI proof used real Chrome through the checked-in Playwright capture
`scripts/captures/browser/hmi-home.spec.mjs`. It passed HTTP, rendered Overview,
live value, connection, freshness, dark-theme, and non-placeholder assertions.
Screenshot: `hmi-home.png`.

Rendered IDE proof used real Chrome against the rebuilt standalone IDE server.
It observed HTTP 200, title `truST IDE — 12_hmi_pid_process_dashboard`, a visible
build control, 19 shipped script assets with HTTP 200, and zero page errors.
Screenshot: `ide-assets.png`.

The existing deeper `ide-home.spec.mjs` journey rendered the IDE, then received
HTTP 403 from its viewer-session project-open request. That authorization
journey is outside the asset-consolidation claim and is not represented as
green here. Four retries for the optional pre-existing
`/ide/wasm/trust_wasm_analysis.js` asset aborted; every consolidated IDE module
and `/ide/ide.js` returned HTTP 200.
