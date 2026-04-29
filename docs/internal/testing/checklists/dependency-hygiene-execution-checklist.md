# Dependency Hygiene Execution Checklist

Status: Done - dependency policy and workspace-member gates added; `cargo geiger` remains advisory-only because the installed tool cannot complete on this workspace/package graph.
Owner: Architecture automation / release engineering
Scope: address audit F4 dependency hygiene findings.

Completion evidence (2026-04-29):

- `cargo audit --json` baseline and after-fix artifacts are stored under `target/gate-artifacts/dependency-hygiene-v0.24.4/`; the policy run passes with explicit ignores for the four remaining legacy transitive advisories.
- `cargo machete --with-metadata` is clean for workspace members when scoped to `xtask` and `crates/*`; the recursive root scan still reports `third_party/tiverse-mmap`, which is now an explicit workspace exclude instead of an ambiguous member.
- `cargo deny check` passes with project `deny.toml` policy and metadata-bearing advisory ignores/license policy.
- `cargo geiger` root run fails on the virtual workspace manifest, and the package-manifest retry fails with package matching errors recorded in `cargo-geiger*.stderr`; this is recorded as advisory-only, not a pass.
- `cargo test -p xtask` covers audit, machete, deny metadata, tiverse workspace membership, and failed-tool-status negative fixtures.

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
