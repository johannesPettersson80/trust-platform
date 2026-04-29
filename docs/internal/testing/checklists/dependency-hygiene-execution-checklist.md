# Dependency Hygiene Execution Checklist

Status: Done - dependency policy, workspace-member gates, and explicit MQTT TLS follow-up are closed; `cargo geiger` remains advisory-only because the installed tool cannot complete on this workspace/package graph.
Owner: Architecture automation / release engineering
Scope: address audit F4 dependency hygiene findings.

Completion evidence (2026-04-29):

- `cargo audit --json` baseline and after-fix artifacts are stored under `target/gate-artifacts/dependency-hygiene-v0.24.5/`; the follow-up MQTT TLS artifact is stored under `target/gate-artifacts/dependency-hygiene-v0.24.7/`. Policy runs pass with explicit ignores for documented legacy/optional transitive advisories after raising MSRV from Rust `1.85` to Rust `1.95`, updating patched dependencies where compatible, and documenting the optional `opcua-wire` `derivative` path.
- `cargo machete --with-metadata` is clean for workspace members when scoped to `xtask` and `crates/*`; the recursive root scan still reports `third_party/tiverse-mmap`, which is now an explicit workspace exclude instead of an ambiguous member.
- `cargo deny check` passes with project `deny.toml` policy and metadata-bearing advisory ignores/license policy.
- `cargo geiger` root run fails on the virtual workspace manifest, and the package-manifest retry fails with package matching errors recorded in `cargo-geiger*.stderr`; this is recorded as advisory-only, not a pass.
- `cargo test -p xtask` covers audit, machete, deny metadata, tiverse workspace membership, and failed-tool-status negative fixtures.
- Final `v0.24.7` gates passed: `cargo audit`, `cargo deny check`, MQTT/runtime focused tests, runtime vertical tests, `RUSTFLAGS=-Dwarnings cargo check -p trust-runtime --all-targets`, `./scripts/runtime_mesh_tls_stability_gate.sh --iterations 8` with Rust `1.95`, `cargo run -p xtask -- architecture-doctor --full-map`, `scripts/render_diagrams.sh`, `python scripts/check_diagram_drift.py`, `cargo test -p xtask`, `just fmt`, `just clippy`, and `just test-all`.
- MQTT security note: `v0.24.5` disabled `rumqttc` default features because the MQTT driver did not implement TLS transport yet. `v0.24.7` closes that gap with explicit `rumqttc` native-tls feature selection, vendored OpenSSL for Linux release cross-builds, `io.params.tls = true`, required `tls_ca_path`, optional mTLS client certificate/key paths, `mqtts://` / `ssl://` TLS scheme inference, and retained `allow_insecure_remote = true` gating for remote plaintext brokers. The Rustls MQTT backend was not used because `rumqttc 0.25.1` pulls the currently vulnerable `rustls-webpki 0.102.8` line.
- OPC UA advisory note: raw `cargo audit` reports `RUSTSEC-2024-0421` for transitive `idna 0.1.5` and `RUSTSEC-2024-0388` for transitive `derivative 2.2.0` through optional `opcua-wire` / `opcua 0.12.0`. `opcua 0.12.0` is the latest `opcua` crate release as of 2026-04-29, so `xtask/config/full_map_policy.json` records metadata-bearing audit exceptions until the OPC UA wire stack moves to a maintained dependency path.

## Targets

- [x] `DEPHYG-TARGET-01` `cargo audit` advisories and warnings.
- [x] `DEPHYG-TARGET-02` `cargo machete` unused-dependency findings.
- [x] `DEPHYG-TARGET-03` `cargo deny` policy.
- [x] `DEPHYG-TARGET-04` `third_party/tiverse-mmap` workspace membership/exclude mismatch.
- [x] `DEPHYG-TARGET-05` unsafe/dependency scans where supported.

## Stop Rules

- [x] `DEPHYG-STOP-01` Do not treat an unavailable dependency tool as a pass; record exact command, error, owner, and fallback.
- [x] `DEPHYG-STOP-02` Do not add an audit/deny/machete allowlist entry without advisory or finding ID, owner, rationale, review date, and removal condition.
- [x] `DEPHYG-STOP-03` Do not remove or move dependencies only to satisfy tooling unless the affected target still builds and tests.
- [x] `DEPHYG-STOP-04` Do not accept a dependency policy rule without a failing fixture, canned report, or equivalent parser/unit test.
- [x] `DEPHYG-STOP-05` Do not leave `third_party/tiverse-mmap` in ambiguous workspace membership state.

## Phase 1 - Baseline

- [x] `DEPHYG-P1-001` Run `cargo audit --json` and store artifact.
- [x] `DEPHYG-P1-002` Run `cargo machete --with-metadata` and store artifact.
- [x] `DEPHYG-P1-003` Run `cargo deny check` and store artifact.
- [x] `DEPHYG-P1-004` Run `cargo geiger` if reliable; otherwise record exact failure and mark advisory-only.
- [x] `DEPHYG-P1-005` Record current third-party workspace membership status.

## Phase 2 - Policy

- [x] `DEPHYG-P2-001` Add or update `deny.toml`.
- [x] `DEPHYG-P2-002` Add audit allowlist with advisory ID, owner, rationale, review date, and removal condition.
- [x] `DEPHYG-P2-003` Add machete allowlist for false positives only; none were needed after removing real unused workspace dependencies.
- [x] `DEPHYG-P2-004` Decide `third_party/tiverse-mmap` ownership: workspace member, workspace exclude, or standalone workspace.
- [x] `DEPHYG-P2-005` Add architecture-doctor summary of dependency hygiene status.

## Phase 3 - Fixes

- [x] `DEPHYG-P3-001` Remove unused dependencies that are real positives.
- [x] `DEPHYG-P3-002` Move dev-only dependencies to dev-dependencies where appropriate; no real move remained after the unused dependency removals.
- [x] `DEPHYG-P3-003` Upgrade or replace dependencies with actionable advisories where feasible.
- [x] `DEPHYG-P3-004` Apply explicit allowlist only when upgrade/removal is not currently feasible.
- [x] `DEPHYG-P3-005` Fix third-party workspace metadata mismatch.

## Phase 4 - Policy Fixtures And Gates

- [x] `DEPHYG-P4-001` Add a canned `cargo audit --json` report with an unallowlisted advisory and assert the policy check fails.
- [x] `DEPHYG-P4-002` Add a canned audit allowlist entry missing owner, rationale, review date, or removal condition and assert policy validation fails.
- [x] `DEPHYG-P4-003` Add a canned `cargo machete` unused-dependency finding and assert unallowlisted findings fail.
- [x] `DEPHYG-P4-004` Add a deny-policy validation test that fails for missing policy sections or malformed allowlist metadata.
- [x] `DEPHYG-P4-005` Add a workspace-membership fixture or unit test that catches the `third_party/tiverse-mmap` include/exclude mismatch class.
- [x] `DEPHYG-P4-006` Add a full-map doctor fixture proving failed dependency hygiene cannot be reported as `pass`.

## Exit Criteria

- [x] `DEPHYG-EXIT-01` `cargo audit` findings are fixed or explicitly allowlisted.
- [x] `DEPHYG-EXIT-02` `cargo machete` findings are fixed or explicitly allowlisted.
- [x] `DEPHYG-EXIT-03` `cargo deny check` uses project policy, not accidental default behavior.
- [x] `DEPHYG-EXIT-04` `third_party/tiverse-mmap` workspace status is intentional and tool-friendly.
- [x] `DEPHYG-EXIT-05` Dependency hygiene appears in full-map doctor output.
- [x] `DEPHYG-EXIT-06` Dependency hygiene policy has at least one failing fixture/canned-report test for audit, machete, deny metadata, and workspace membership.

## Security Follow-Ups

- [x] `DEPHYG-FOLLOW-01` Implement explicit MQTT TLS/mTLS support before claiming secure remote MQTT support. Required detail:
  - Version context: `v0.24.5` changed `rumqttc` from default features to `rumqttc = { version = "0.25", default-features = false }`; `v0.24.7` keeps default features disabled and explicitly enables only `use-native-tls` for MQTT TLS, with vendored OpenSSL for Linux release cross-builds, because the `rumqttc 0.25.1` Rustls path pulls the vulnerable `rustls-webpki 0.102.8` line.
  - Scope closed: `io.params.tls = true`, `tls_ca_path`, optional `tls_client_cert_path` / `tls_client_key_path`, optional `tls_alpn`, and `mqtts://` / `ssl://` broker scheme inference are implemented in `crates/trust-runtime/src/io/mqtt/config.rs`, `parsing.rs`, and `session.rs`.
  - Security rule closed: remote MQTT brokers are accepted without `allow_insecure_remote = true` only when TLS is configured; remote plaintext brokers still require the explicit insecure override.
  - SNI behavior: MQTT TLS uses the broker host name as the TLS server name/SNI value; public docs require a DNS host name rather than relying on a raw IP address with a self-signed certificate.
  - Coverage evidence: `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime --lib mqtt -- --nocapture` passed 10 MQTT unit tests, including TLS transport construction and security validation. `RUSTUP_TOOLCHAIN=1.95 cargo test -p trust-runtime mqtt -- --nocapture` also passed the broader filtered runtime test surface, including `io_multidriver_live` and the web MQTT probe.
  - Release gate evidence: final `cargo audit`, `cargo deny check`, runtime vertical tests, diagram drift check, `just fmt`, `just clippy`, and `just test-all` passed before release closeout.
