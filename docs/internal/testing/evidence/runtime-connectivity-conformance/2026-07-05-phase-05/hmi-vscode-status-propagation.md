# Phase 5 - HMI And VS Code Status Propagation

Date: 2026-07-05
Builder copy: `trust-builder:/home/johannes/projects/trust-platform-rtconn-validation`
Builder target: `trust-builder:/home/johannes/.cache/codex-targets/trust-platform-rtconn-validation`
Remote log dir: `trust-builder:/home/johannes/projects/trust-platform-rtconn-validation/.phase5-logs-20260704T234923Z`

## API Inventory

HMI consumes these runtime surfaces for the connector/status slice:

- `GET /hmi`, `/hmi/styles.css`, `/hmi/app.js`, and `/hmi/modules/*`.
- `POST /api/control` with `hmi.schema.get`, `hmi.descriptor.get`,
  `hmi.values.get`, `hmi.trends.get`, `hmi.alarms.get`, `hmi.alarm.ack`,
  phase-gated `hmi.write`, and the new Viewer-readable `connectors.status`.
- `/ws/hmi` remains the live value push path; connector status uses the bounded
  HTTP polling path at `CONNECTOR_POLL_MS = 2000`.

VS Code Devices & Connections consumes these runtime surfaces for the
connector/status and discovery-label slice:

- `comm.capabilities` and `comm.schema` for protocol/action availability.
- `fleet.topology` for runtime/endpoint topology.
- `connectors.status` for shared `ConnectorState`, `ConnectorHealth`,
  `DiscoveryConfidence`, and connector point-count projection.
- `comm.discover` for discovery candidates and confidence labels.
- The extension now merges `connectors.status` into topology endpoints before
  graph model construction; it does not stay on legacy ADS/OPC UA/I/O JSON for
  this slice.

## Source Changes

- HMI header status chips: `crates/trust-runtime/src/web/ui/hmi.html`.
- HMI connector summary logic: `crates/trust-runtime/src/web/ui/chunks/hmi-js/hmi-01.js`.
- HMI polling transport: `crates/trust-runtime/src/web/ui/modules/hmi-transport.js`.
- HMI startup hook: `crates/trust-runtime/src/web/ui/chunks/hmi-js/hmi-03.js`.
- HMI chip CSS: `crates/trust-runtime/src/web/ui/chunks/hmi-css/hmi-01.css`.
- VS Code connector merge module: `editors/vscode/src/networkCanvas/connectorsStatus.ts`.
- VS Code topology/model/webview propagation:
  `fleetTopology.ts`, `fleetModel.ts`, `graphData.ts`, `webview/types.ts`,
  `webview/layout.ts`, `webview/nodes.tsx`, and `webview/DiscoverPane.tsx`.
- Generated shipped webview assets:
  `editors/vscode/media/networkCanvasWebview.{js,css,js.map,css.map}`.
- Tests:
  `crates/trust-runtime/tests/hmi_readonly_integration/*` and
  `editors/vscode/src/test/suite/network-canvas.test.ts`.

## Visual Evidence

HMI Playwright proof used a temporary local HTTP server that served the checked-in
HMI HTML plus the same concatenated JS/CSS chunks served by `crates/trust-runtime/src/web.rs`.
Only control responses were mocked so the browser could render deterministic
connector states without lab hardware.

- Screenshot:
  `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-05/screenshots/hmi-connector-status-summary.png`
- JSON assertion:
  `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-05/screenshots/hmi-connector-status-summary.json`
- Rendered labels:
  - `connectors: ready 1 · not-ready 1`
  - `health: ok 1 · unknown 1`
  - `proof: confirmed 1 · tcp-only 1`
  - `points: good 2 · issue 2`

Network Canvas browser proof loaded the shipped `networkCanvasWebview` bundle with
an initial graph carrying connector status, then hovered the MQTT endpoint.

- Screenshot:
  `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-05/screenshots/network-canvas-connector-hover.png`
- DOM assertion:
  `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-05/screenshots/network-canvas-connector-hover-dom.json`
- Hover card text includes `connector state stale`, `connector health degraded`,
  `proof tcp-only`, and `point quality good 1 · issue 2`.

The HMI mock server does not implement `/ws/hmi`, so the HMI screenshot JSON
records an expected websocket 404 while proving the HTTP connector polling path.

## Remote Gates

Disk preflight before targeted gates:

- `/home/johannes`: 111G free.
- `/tmp`: 6.8G free.

Targeted HMI/runtime tests:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" cargo test -p trust-runtime --test hmi_readonly_integration hmi_connector_summary_renders_shared_status_contract'
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" cargo test -p trust-runtime --test hmi_readonly_integration hmi_dashboard_routes_render_without_manual_layout'
```

Results:

- `hmi_connector_summary_renders_shared_status_contract`: passed, 1 test, 185s
  after the clean target rebuild.
- `hmi_dashboard_routes_render_without_manual_layout`: passed, 1 test, 0s
  using the warmed target.

VS Code gates:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation/editors/vscode" && npm run lint'
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation/editors/vscode" && npm run compile'
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation/editors/vscode" && ST_LSP_TEST_SERVER="$HOME/.cache/codex-targets/trust-platform-rtconn-validation/debug/trust-lsp" xvfb-run -a -s "-screen 0 1920x1080x24" npm test'
```

Results:

- `npm run lint`: passed, 4s.
- `npm run compile`: passed, 12s.
- Direct `npm test` without Xvfb failed before tests with `Missing X server or $DISPLAY`.
  The Xvfb rerun passed with 443 tests in 33s and included
  `connector status surface flows into endpoint graph metadata`.

Common Phase 5 builder gate:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && just fmt'
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && just clippy'
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && just test'
```

Results:

- `just fmt`: passed, 3s.
- `just clippy`: passed, 94s.
- `just test`: passed, fallback `cargo test -p trust-runtime --lib`,
  831 passed, 16 ignored, 38s.

Disk after the common gate:

- `/home/johannes`: 96G free.
- `/tmp`: 6.7G free.

## Release Hygiene

- `CHANGELOG.md` now records HMI and Devices & Connections consuming the shared
  connector status/discovery-confidence contract.
- Workspace version, `editors/vscode/package.json`, and the root
  `editors/vscode/package-lock.json` version fields are synchronized at
  `0.24.28`, the current target release for this board.
