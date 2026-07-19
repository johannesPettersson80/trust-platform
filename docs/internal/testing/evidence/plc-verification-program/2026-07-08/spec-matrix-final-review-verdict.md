# Spec Matrix Final Review Verdict

Date: 2026-07-08
Status: clear for implementation
Scope: PLC verification-program document set

## Verdict

The follow-up review found the spec-matrix fold complete and correct. Phase 1,
`VERIF-P1A-010`, and Phase 1B may start under the existing stop gates.

## Verified By Review

- `spec-matrix-model.md` defines canonical machine areas, including
  `control_security`, `plcopen_devtools`, and `verification`.
- Required specification tags are mechanically checkable through active
  spec-source `covers` entries or open owned spec gaps.
- `expected_authority` is an allowlist and `blocks` has the three-value enum:
  `test_mapping`, `release_claim`, and `none_yet`.
- "Uninventoried area" has a machine definition for `plan_tests.py`.
- The matrix is explicitly a debt map, not a work order.
- Missing content classes are represented in the seed scope: PLCopen, harness
  and simulation semantics, config schemas, CLI/control-socket surfaces, GPIO,
  and runtime performance budgets.
- Earlier spec-first findings are folded: TOML container convention, structured
  delta semantics, producer allowlist, red-to-green pairing, decision-ref
  waivers, risk-change baseline, stable error-code precondition, mutation shard
  pull-forward, enforcing ratchet, artifact-stamping split, and invariant-name
  alignment.

## Folded Residuals

- Added a rule that `expected_delta` must be explained in oracle-cited notes or
  another structured field in the same record. The value is not self-describing.
- Fixed punctuation in the metadata self-test fixture list so the new fixtures
  are part of the same list.

## Local Hook Fix

The review also identified a local Claude hook robustness issue. The PreToolUse
hook command in `.claude/settings.json` was changed from a cwd-relative path to:

```json
"python3 \"$CLAUDE_PROJECT_DIR\"/.claude/hooks/pre_tool_use_gate.py"
```

This private local config remains ignored and is not part of public release
state. The issue is a useful future verification seed for platform/environment
variation and hook self-tests once `verification/spec-matrix.toml` lands.

## Implementation Start Recommendation

Start with:

1. `VERIF-P1-001..020`
2. `VERIF-P1A-010`
3. Phase 1B in row order

The first real behavior test of the system should be `VERIF-P1B-004`, where the
subrange, declared-type, and string-bound decisions force honest spec-gap
exit-3 behavior before tests or product code can invent the answer.
