# Phase 17 Independent Review Protocol

review the complete plan before evaluating any Phase 17 evidence:
`docs/internal/testing/checklists/plc-verification-program/code-spec-coverage-plan.md`.
Then review the Phase 17 rows and stop rows in
`docs/internal/testing/checklists/plc-verification-program/implementation-board.md`.

This protocol is a review input, not acceptance evidence. Completing its steps
does not itself close a checklist row. The reviewer must be independent of the
agent that implemented or classified the reviewed facts, and must write a
separate, commit-bound verdict.

## Frozen Review Target

The review request must name all of the following before review begins:

- canonical repository: `/home/johannes/projects/trust-platform`;
- branch: `plc-verification-program`;
- one clean, full 40-character implementation SHA, recorded as
  `FROZEN_IMPLEMENTATION_SHA`;
- the immediate child SHA that first introduces the exhaustive debt baseline,
  recorded as `FROZEN_BASELINE_SHA`;
- the clean detached checkout used for reproduction;
- the SHA-256 values of the destination `AGENTS.md` and complete
  `.codex/skills/**` tree after they are manually copied from the canonical
  checkout;
- the exact private-input binding and live digest for
  `.claude/hooks/pre_tool_use_gate.py`.

The first commit that contains
`verification/code-spec-debt-baseline.toml` must be a single-parent immediate
child of `FROZEN_IMPLEMENTATION_SHA`, and its `created_from_commit` must equal
that parent. Later history may only delete complete baseline entries. A dirty
tree, abbreviated SHA, rebased baseline origin, appended entry, or edited field
is `BLOCKED`.

## P17-006 Pilot Acceptance

The reviewer must inspect the complete live partition for
`crates/trust-runtime/src/stdlib/time.rs`, not a hand-selected list. Acceptance
requires all of the following:

- exactly 88 live facts and exactly 88 exact map records for the reviewed file;
- `runtime_clock_value` and `current_dt` are present as parser-derived facts;
- all 32 risk-bearing facts have symbol-specific, body-digest-bound,
  owner-reviewed acknowledgements with a current expiry;
- `CURRENT_DT` resolves to the open
  `SPEC_GAP_CURRENT_DT_CLOCK_BOUNDARY_001` unless an independently reviewed,
  active oracle has superseded that gap;
- no name, comment, neighboring function, test, or current implementation
  result is treated as an oracle;
- no `non_behavioral`, inheritance, or broad suppression hides a risk signal;
- the implementation-side review in
  `docs/internal/testing/evidence/plc-verification-program/2026-07-21-p17-stdlib-time-risk-review.md`
  is treated only as review input, never as independent acceptance.

The reviewer must independently compare every pilot record with the eligible
specification, invariant, test-catalog, and open-gap metadata it cites. A
mechanically valid join is necessary but is not semantic acceptance.

## P17-012 Whole-Denominator Acceptance

The reviewer must reproduce the complete report from the clean frozen target
and verify that each scoped language is an exact one-to-one partition. The
verdict must record exact source and fact denominators for Rust,
TypeScript/JavaScript, Python, shell, and Rust verification tooling. It must
also record counts by canonical area, owner, disposition, risk signal, and
reviewable area-candidate set.

Acceptance requires:

- zero live facts missing from both the map and baseline;
- zero duplicate, invented, stale, overlapping, or out-of-scope identities;
- zero unresolved area/owner routing decisions;
- zero stale body digests and zero unacknowledged or expired high-risk facts;
- an empty frozen debt baseline whose history remains append- and edit-free;
- every `specified` chain validates in both directions through an active,
  oracle-eligible specification, invariant, executable catalog test, and suite
  route;
- every `spec_gap` resolves to a still-open, same-area gap that names the exact
  affected code identity;
- every inherited, generated, vendored, and non-behavioral disposition passes
  its closed semantic rules without a directory-, crate-, owner-, or
  name-derived blanket decision;
- the private hook resolves only through its exact tracked manifest entry and
  live digest;
- the authoritative gate and fuzz inventories join without duplication or an
  unbound executable entrypoint;
- the canonical JSON and Markdown reports reconstruct exactly and pass their
  at-rest validators with `proof_kind = "none"`;
- changed mode fails closed for new code, signature drift, body drift, removed
  catcher logic, and baseline mutation, independently of merge-base topology;
- full mode is green before whole-denominator enforcement is enabled;
- the existing verification workflow remains the sole CI route and the release
  dependency graph is unchanged;
- the real-browser evidence is reviewed with its recorded scope and
  limitations; HTTP or source-only checks do not replace rendered proof.

The reviewer must inspect the raw rows for each language rather than accepting
summary counts alone. Sampling may supplement but cannot replace the exact
machine-checked partition. Semantic classification remains a human review
obligation; the implementation and scanner output are not specification
oracles.

## Reproduction Commands

Run focused checks first from the clean detached review checkout. Store command,
exit status, stdout/stderr digest, checkout SHA, and environment identity in the
review evidence.

```bash
python -m unittest \
  scripts.verification.code_spec_cli_tests \
  scripts.verification.code_spec_contract_tests \
  scripts.verification.code_spec_debt_tests \
  scripts.verification.code_spec_gate_tests \
  scripts.verification.code_spec_live_tests \
  scripts.verification.code_spec_pilot_tests \
  scripts.verification.code_spec_provenance_tests \
  scripts.verification.code_spec_report_tests \
  scripts.verification.code_spec_rust_tests \
  scripts.verification.code_spec_semantic_contract_tests \
  scripts.verification.code_spec_tooling_join_tests \
  scripts.verification.code_spec_tooling_tests \
  scripts.verification.code_spec_typescript_adapter_tests \
  scripts.verification.code_spec_typescript_tests \
  scripts.verification.metadata_validator.code_spec_tests \
  scripts.verification.planner_tests \
  scripts.verification.report_gate_tests
python3 scripts/run_verification_focused_tests.py
scripts/verification_metadata_gate.sh
python3 scripts/check_verification_tooling_selftests.py
python3 scripts/check_test_catalog_staleness.py
python scripts/code_spec_coverage_gate.py --mode report --out-dir /tmp/p17-report
python scripts/code_spec_coverage_gate.py --mode changed --out-dir /tmp/p17-changed
python scripts/code_spec_coverage_gate.py --mode full --out-dir /tmp/p17-full
```

Verify the gate/fuzz join in the canonical report. Do not substitute an ad hoc
partial list for either registered campaign.

Only after the focused review is accepted, run the single final broad milestone
on `trust-builder` from the same frozen source:

```bash
just fmt
just clippy
just test-all
```

The broad milestone is final evidence, not a waiver for a code-spec failure. Do
not rerun it after every mapping edit; any source change after the milestone
invalidates the frozen review target and requires focused re-review before one
new final milestone.

## Required Verdict

The independent evidence must use exactly one verdict:

- `CLEAR`: every P17-006 or P17-012 acceptance condition in scope is met with
  zero findings;
- `CLEAR-WITH-FINDINGS`: no acceptance blocker remains, and each nonblocking
  finding has an owner, severity, exact affected identity, and disposition;
- `BLOCKED`: at least one acceptance condition is not met or cannot be
  reproduced.

The verdict must identify the reviewer and independence basis, frozen SHAs,
bootstrap hashes, exact denominators, command results, every finding, and the
specific checklist rows it authorizes. It must state that it creates no product
proof, closes no specification gap, and promotes no invariant. Only a later
reviewed enforcement commit may update `AGENTS.md`, `trust-test-authoring`, the
P17 checklist rows, and the two stop rows.
