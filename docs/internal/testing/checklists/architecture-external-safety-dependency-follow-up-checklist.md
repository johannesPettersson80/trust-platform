# Architecture External Safety / Dependency Follow-Up Checklist

Status: In progress. Phase 1 geiger retest completed on 2026-05-03; raw audit upgrade paths remain open.
Owner: Architecture/runtime/security
Scope: external-tool and upstream-dependency residuals left after the architecture post-closeout gap closure. This board does not reopen the completed architecture-program umbrella; it owns the remaining third-party unsafe cross-check and raw audit upgrade paths.

## Source Residuals

- [x] `ARCHEXT-BASE-01` First-party unsafe/concurrency enforcement remains active through `FULLMAP-CHECK-09`; BOARD-11 is complete with registered unsafe, panic-like, and concurrency-boundary sites.
- [x] `ARCHEXT-BASE-02` `cargo geiger 0.13.0` is advisory-partial on this workspace because the root virtual manifest/package graph cannot be scanned cleanly in the current tool version.
- [x] `ARCHEXT-BASE-03` Raw `cargo audit` advisories remain policy-owned by `deny.toml` for OPC UA, tiny_http TLS, and Zenoh dependency paths.

## Phase 1 - External Unsafe Cross-Check

- [x] `ARCHEXT-GEIGER-01` Re-test `cargo geiger` with the current installed version and a per-package fallback for `trust-runtime`, `trust-runtime-core`, `trust-plcopen`, and `trust-dev`. Evidence: `cargo-geiger 0.13.0` still rejects the root virtual manifest; package-manifest full scans were re-tested and proved unsafe for routine gates because `trust-runtime` scanning cleaned `target/` and started a full rebuild; `--forbid-only` package probes are also advisory-partial because they emit repeated dependency parse failures and do not terminate cleanly without a timeout.
- [x] `ARCHEXT-GEIGER-02` If geiger still fails, evaluate a replacement third-party unsafe scanner or pin a known-working geiger invocation in a repo script. Evidence: `scripts/architecture_external_safety_geiger_gate.sh` pins bounded geiger probes with `timeout` and records the result as advisory-partial instead of allowing destructive/unbounded geiger runs in the normal gate path.
- [x] `ARCHEXT-GEIGER-03` Store the external unsafe scan artifact under `target/gate-artifacts/architecture-external-safety-*` and link it from BOARD-11 or this follow-up board. Evidence: `scripts/architecture_external_safety_geiger_gate.sh` writes `target/gate-artifacts/architecture-external-safety-<commit>/cargo-geiger.txt`.
- [x] `ARCHEXT-GEIGER-04` Keep `FULLMAP-CHECK-09` as the enforced gate until the external scanner is reliable in CI. Evidence: geiger remains advisory-partial; the enforced gate remains `cargo run -p xtask -- architecture-doctor --full-map` / `FULLMAP-CHECK-09`.

## Phase 2 - Raw Audit Upgrade Paths

- [ ] `ARCHEXT-AUDIT-01` OPC UA path: remove or replace the `opcua`/`opcua-wire` transitive advisory path for `idna` and `derivative`, or keep the current `deny.toml` exception with a refreshed owner/date/removal condition if no maintained upstream exists.
- [ ] `ARCHEXT-AUDIT-02` tiny_http TLS path: replace `tiny_http` `ssl-rustls` / `rustls 0.20` / `ring 0.16` / `rustls-pemfile` usage with a maintained TLS stack or a patched upstream release.
- [ ] `ARCHEXT-AUDIT-03` Zenoh path: move the runtime mesh baseline to a Zenoh transport stack that no longer pulls the advisory-bearing `rsa`/`paste` path, or document the exact upstream blocker and renewal date.
- [ ] `ARCHEXT-AUDIT-04` Remove each `cargo audit --ignore` / `deny.toml` advisory exception only after `cargo tree -i <crate>` proves the fixed dependency version is the one used by the workspace.

## Exit Criteria

- [ ] `ARCHEXT-EXIT-01` External unsafe scan is either reliable in CI or has an exact replacement tool with artifacts. Current blocker: geiger is bounded and artifact-producing but still advisory-partial, not a reliable CI replacement for `FULLMAP-CHECK-09`.
- [ ] `ARCHEXT-EXIT-02` Raw audit advisory paths are upgraded, removed, or renewed with owner/date/removal metadata.
- [ ] `ARCHEXT-EXIT-03` `cargo deny check`, `cargo audit --ignore ...`, and `cargo run -p xtask -- architecture-doctor --full-map` pass after any policy changes.
