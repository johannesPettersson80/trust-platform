# Dependency Hygiene Execution Checklist

Status: Planned
Owner: Architecture automation / release engineering
Scope: address audit F4 dependency hygiene findings.

## Targets

- [ ] `DEPHYG-TARGET-01` `cargo audit` advisories and warnings.
- [ ] `DEPHYG-TARGET-02` `cargo machete` unused-dependency findings.
- [ ] `DEPHYG-TARGET-03` `cargo deny` policy.
- [ ] `DEPHYG-TARGET-04` `third_party/tiverse-mmap` workspace membership/exclude mismatch.
- [ ] `DEPHYG-TARGET-05` unsafe/dependency scans where supported.

## Stop Rules

- [ ] `DEPHYG-STOP-01` Do not treat an unavailable dependency tool as a pass; record exact command, error, owner, and fallback.
- [ ] `DEPHYG-STOP-02` Do not add an audit/deny/machete allowlist entry without advisory or finding ID, owner, rationale, review date, and removal condition.
- [ ] `DEPHYG-STOP-03` Do not remove or move dependencies only to satisfy tooling unless the affected target still builds and tests.
- [ ] `DEPHYG-STOP-04` Do not accept a dependency policy rule without a failing fixture, canned report, or equivalent parser/unit test.
- [ ] `DEPHYG-STOP-05` Do not leave `third_party/tiverse-mmap` in ambiguous workspace membership state.

## Phase 1 - Baseline

- [ ] `DEPHYG-P1-001` Run `cargo audit --json` and store artifact.
- [ ] `DEPHYG-P1-002` Run `cargo machete --with-metadata` and store artifact.
- [ ] `DEPHYG-P1-003` Run `cargo deny check` and store artifact.
- [ ] `DEPHYG-P1-004` Run `cargo geiger` if reliable; otherwise record exact failure and mark advisory-only.
- [ ] `DEPHYG-P1-005` Record current third-party workspace membership status.

## Phase 2 - Policy

- [ ] `DEPHYG-P2-001` Add or update `deny.toml`.
- [ ] `DEPHYG-P2-002` Add audit allowlist with advisory ID, owner, rationale, review date, and removal condition.
- [ ] `DEPHYG-P2-003` Add machete allowlist for false positives only.
- [ ] `DEPHYG-P2-004` Decide `third_party/tiverse-mmap` ownership: workspace member, workspace exclude, or standalone workspace.
- [ ] `DEPHYG-P2-005` Add architecture-doctor summary of dependency hygiene status.

## Phase 3 - Fixes

- [ ] `DEPHYG-P3-001` Remove unused dependencies that are real positives.
- [ ] `DEPHYG-P3-002` Move dev-only dependencies to dev-dependencies where appropriate.
- [ ] `DEPHYG-P3-003` Upgrade or replace dependencies with actionable advisories where feasible.
- [ ] `DEPHYG-P3-004` Apply explicit allowlist only when upgrade/removal is not currently feasible.
- [ ] `DEPHYG-P3-005` Fix third-party workspace metadata mismatch.

## Phase 4 - Policy Fixtures And Gates

- [ ] `DEPHYG-P4-001` Add a canned `cargo audit --json` report with an unallowlisted advisory and assert the policy check fails.
- [ ] `DEPHYG-P4-002` Add a canned audit allowlist entry missing owner, rationale, review date, or removal condition and assert policy validation fails.
- [ ] `DEPHYG-P4-003` Add a canned `cargo machete` unused-dependency finding and assert unallowlisted findings fail.
- [ ] `DEPHYG-P4-004` Add a deny-policy validation test that fails for missing policy sections or malformed allowlist metadata.
- [ ] `DEPHYG-P4-005` Add a workspace-membership fixture or unit test that catches the `third_party/tiverse-mmap` include/exclude mismatch class.
- [ ] `DEPHYG-P4-006` Add a full-map doctor fixture proving failed dependency hygiene cannot be reported as `pass`.

## Exit Criteria

- [ ] `DEPHYG-EXIT-01` `cargo audit` findings are fixed or explicitly allowlisted.
- [ ] `DEPHYG-EXIT-02` `cargo machete` findings are fixed or explicitly allowlisted.
- [ ] `DEPHYG-EXIT-03` `cargo deny check` uses project policy, not accidental default behavior.
- [ ] `DEPHYG-EXIT-04` `third_party/tiverse-mmap` workspace status is intentional and tool-friendly.
- [ ] `DEPHYG-EXIT-05` Dependency hygiene appears in full-map doctor output.
- [ ] `DEPHYG-EXIT-06` Dependency hygiene policy has at least one failing fixture/canned-report test for audit, machete, deny metadata, and workspace membership.
