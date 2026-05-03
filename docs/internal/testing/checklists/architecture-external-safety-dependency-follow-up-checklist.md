# Architecture External Safety / Dependency Follow-Up Checklist

Status: Planned follow-up
Owner: Architecture/runtime/security
Scope: external-tool and upstream-dependency residuals left after the architecture post-closeout gap closure. This board does not reopen the completed architecture-program umbrella; it owns the remaining third-party unsafe cross-check and raw audit upgrade paths.

## Source Residuals

- [x] `ARCHEXT-BASE-01` First-party unsafe/concurrency enforcement remains active through `FULLMAP-CHECK-09`; BOARD-11 is complete with registered unsafe, panic-like, and concurrency-boundary sites.
- [x] `ARCHEXT-BASE-02` `cargo geiger 0.13.0` is advisory-partial on this workspace because the root virtual manifest/package graph cannot be scanned cleanly in the current tool version.
- [x] `ARCHEXT-BASE-03` Raw `cargo audit` advisories remain policy-owned by `deny.toml` for OPC UA, tiny_http TLS, and Zenoh dependency paths.

## Phase 1 - External Unsafe Cross-Check

- [ ] `ARCHEXT-GEIGER-01` Re-test `cargo geiger` with the current installed version and a per-package fallback for `trust-runtime`, `trust-runtime-core`, `trust-plcopen`, and `trust-dev`.
- [ ] `ARCHEXT-GEIGER-02` If geiger still fails, evaluate a replacement third-party unsafe scanner or pin a known-working geiger invocation in a repo script.
- [ ] `ARCHEXT-GEIGER-03` Store the external unsafe scan artifact under `target/gate-artifacts/architecture-external-safety-*` and link it from BOARD-11 or this follow-up board.
- [ ] `ARCHEXT-GEIGER-04` Keep `FULLMAP-CHECK-09` as the enforced gate until the external scanner is reliable in CI.

## Phase 2 - Raw Audit Upgrade Paths

- [ ] `ARCHEXT-AUDIT-01` OPC UA path: remove or replace the `opcua`/`opcua-wire` transitive advisory path for `idna` and `derivative`, or keep the current `deny.toml` exception with a refreshed owner/date/removal condition if no maintained upstream exists.
- [ ] `ARCHEXT-AUDIT-02` tiny_http TLS path: replace `tiny_http` `ssl-rustls` / `rustls 0.20` / `ring 0.16` / `rustls-pemfile` usage with a maintained TLS stack or a patched upstream release.
- [ ] `ARCHEXT-AUDIT-03` Zenoh path: move the runtime mesh baseline to a Zenoh transport stack that no longer pulls the advisory-bearing `rsa`/`paste` path, or document the exact upstream blocker and renewal date.
- [ ] `ARCHEXT-AUDIT-04` Remove each `cargo audit --ignore` / `deny.toml` advisory exception only after `cargo tree -i <crate>` proves the fixed dependency version is the one used by the workspace.

## Exit Criteria

- [ ] `ARCHEXT-EXIT-01` External unsafe scan is either reliable in CI or has an exact replacement tool with artifacts.
- [ ] `ARCHEXT-EXIT-02` Raw audit advisory paths are upgraded, removed, or renewed with owner/date/removal metadata.
- [ ] `ARCHEXT-EXIT-03` `cargo deny check`, `cargo audit --ignore ...`, and `cargo run -p xtask -- architecture-doctor --full-map` pass after any policy changes.
