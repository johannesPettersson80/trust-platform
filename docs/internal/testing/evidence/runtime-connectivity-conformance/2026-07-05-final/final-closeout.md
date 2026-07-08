# Final Closeout - Runtime Connectivity And Conformance Remediation

Date: 2026-07-05

## Deferred Item

`RTCONN-P9-004` is intentionally deferred rather than completed.

Required external input:

- A real CODESYS PLCopen XML export generated from a scratch demo project owned
  by this repository.
- A real TwinCAT PLCopen XML export generated from a scratch demo project owned
  by this repository.
- Fixture provenance documenting tool name/version, project source files,
  export command or UI path, export date, and confirmation that no third-party
  project content was copied.

Local and `trust-builder` inspection found no CODESYS/TwinCAT export tooling and
no acceptable real vendor export fixtures. Existing tiny CODESYS/TwinCAT-named
fixtures were renamed as synthetic and kept separate from the future real-export
corpus.

## Release Hygiene Evidence

- `CHANGELOG.md` records the runtime connector status surface, HMI/VS Code
  propagation, honest discovery/degraded status behavior, Modbus depth, MQTT
  typed/Sparkplug outbound depth, device-in-loop gates, conformance v2, PLCopen
  non-ST rejection, and `trust-dev` alias-removal-window behavior.
- Workspace target release is `0.24.28`.
- `Cargo.toml`, `editors/vscode/package.json`, and the root package entry in
  `editors/vscode/package-lock.json` are synchronized at `0.24.28`.

## Diagram Evidence

- Connector ownership and status-flow diagrams were updated in:
  - `docs/diagrams/architecture/runtime-execution.puml`
  - `docs/diagrams/architecture/full-software-map-generated.puml`
- Generated diagram outputs and `docs/diagrams/manifest.json` were refreshed.
- Phase 9 evidence records passing `scripts/render_diagrams.sh` and
  `python scripts/check_diagram_drift.py` on `trust-builder` after the latest
  diagram changes.

## Browser And VS Code Evidence

Phase 5 evidence covers the browser-visible connector status propagation:

- HMI Playwright proof:
  `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-05/screenshots/hmi-connector-status-summary.png`
- HMI JSON assertion:
  `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-05/screenshots/hmi-connector-status-summary.json`
- Network Canvas browser proof:
  `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-05/screenshots/network-canvas-connector-hover.png`
- Network Canvas DOM assertion:
  `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-05/screenshots/network-canvas-connector-hover-dom.json`

Phase 5 also records passing VS Code extension gates on `trust-builder`:
`npm run lint`, `npm run compile`, and Xvfb-backed `npm test`.

## Final Gate Status

Remote host: `trust-builder`

Remote validation copy:
`/home/johannes/projects/trust-platform-rtconn-validation`

The validation copy was synced from the local checkout without `.git`,
`target`, `fuzz/target`, `node_modules`, `.vscode-test`, or bulky internal
evidence artifacts.

Disk preflight before final gates:

- `/home/johannes`: 111G free.
- `/tmp`: 6.7G free.

Runtime vertical tests:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && export CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" && cargo test -p trust-runtime --test api_smoke --test debug_control --test complete_program --test runtime_reliability'
```

Result: pass.

- `api_smoke`: 3 passed.
- `complete_program`: 1 passed.
- `debug_control`: 18 passed, 2 ignored.
- `runtime_reliability`: 4 passed.

Communication-specific gate bundle:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && export CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" && cargo test -p trust-runtime --test opcua_integration --test opcua_client_runtime --test ads_cli_command --test ads_web_api --test modbus_driver --test ethercat_driver --test io_multidriver_live && ./scripts/runtime_comms_conformance_gate.sh'
```

Result: pass.

- `ads_cli_command`: 9 passed.
- `ads_web_api`: 6 passed.
- `ethercat_driver`: 3 passed, 2 ignored.
- `io_multidriver_live`: 2 passed.
- `modbus_driver`: 8 passed, 2 ignored.
- `opcua_client_runtime`: 2 passed.
- `opcua_integration`: 4 passed.
- `runtime_comms_conformance_gate.sh`: pass.

Networking gates:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && export CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" && ./scripts/runtime_mesh_tls_stability_gate.sh --iterations 8 && RUSTFLAGS=-Dwarnings cargo check -p trust-runtime --all-targets'
```

Result: pass.

- `runtime_mesh_tls_stability_gate.sh --iterations 8`: 8/8 passed.
- `RUSTFLAGS=-Dwarnings cargo check -p trust-runtime --all-targets`: pass.

Common final builder gate:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && export CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" && just fmt && just clippy && just test-all'
```

Result: pass.

- `just fmt`: pass.
- `just clippy`: pass.
- `just test-all`: pass through
  `./scripts/cargo_test_fast_link.sh test --all`.

Local final check:

```sh
git diff --check
```

Result: pass.

Remote cleanup after final gates:

```sh
ssh trust-builder 'rm -rf "$HOME/.cache/codex-targets/"* "$HOME/.cache/sccache" "$HOME/projects/trust-platform-rtconn-validation/target" "$HOME/projects/trust-platform-rtconn-validation/fuzz/target"'
```

Final builder disk state:

- `/home/johannes`: 98G free.
- `/tmp`: 6.7G free.
- `$HOME/.cache`: 133M.
- `$HOME/.cache/codex-targets`: 4.0K.

Still pending:

- tag/release proof, if this board is accepted for release from `main`
