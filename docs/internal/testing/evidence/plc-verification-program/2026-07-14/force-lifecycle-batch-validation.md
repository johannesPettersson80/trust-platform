# Force lifecycle batch validation

Date: 2026-07-14

Final source bytes: `cb1ece912a4274a7e436f2b92a046cb8882dd9ca`

## Result

Six previously missing force-lifecycle boundaries now have focused product
tests: pause/resume, non-terminating disconnect, release, deliberate stop,
fault recovery, and authorization-token change. All six focused tests passed.
No product defect was observed, so this batch contains no product behavior
change and no manufactured red proof.

## Focused validation

On a clean `trust-builder` worktree at
`4c97a8461032df65d3e10f04d1e9c14ff7422b5e`:

```text
CARGO_TARGET_DIR=$HOME/.cache/codex-targets/trust-platform-gate \
  cargo test -p trust-runtime --test force_lifecycle_boundaries
```

Result: 5 passed, 0 failed.

```text
CARGO_TARGET_DIR=$HOME/.cache/codex-targets/trust-platform-gate \
  cargo test -p trust-runtime --lib \
  control::tests::auth_token_change_preserves_force_until_authorized_release \
  -- --exact
```

Result: 1 passed, 0 failed.

The unchanged producer-bound runner also passed 1/1:

```text
CARGO_TARGET_DIR=$HOME/.cache/codex-targets/trust-platform-gate \
  cargo test -p trust-runtime --test force_lifecycle \
  force_lifecycle_trace_cases -- --exact
```

## Broad validation

After clearing only the generated shared target, the builder preflight reported
87 GiB available under `/home/johannes` and 6.7 GiB under `/tmp`. This command
then exited zero:

```text
ssh trust-builder 'cd "$HOME/projects/trust-platform" && \
  export CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-gate" \
  TMPDIR="$HOME/.cache/codex-targets/trust-platform-gate-tmp" && \
  just fmt && just clippy && just test-all'
```

- `just fmt`: completed and exposed one formatting-only delta in the new
  integration-test file.
- `just clippy --all-targets --all-features`: passed after that formatting step,
  with the pre-existing `trust-lsp` `clippy::question_mark` advisory.
- `just test-all`: passed after that formatting step, including all six new
  force-lifecycle tests.
- The formatting-only delta was committed as `cb1ece91`; a subsequent clean
  builder checkout at that commit ran `just fmt` with no change. Therefore the
  source bytes compiled and tested by `clippy` and `test-all` are byte-identical
  to the final committed source.

This run is broad validation, not approved broad proof. The reviewed broad
producer correctly requires every linked test to emit a bound case artifact;
the six new ordinary Rust tests do not. No `broad-remote-gate.py v1` evidence
row was created, and `RT_SAFE_FORCE_001` remains S0.

## Verification tripwires

The complete focused Python suite ran 766 tests. Its only three failures were
the deliberate live-census tripwires caused by these six new Rust facts:

- Rust facts: 3,091 to 3,097;
- all scanner facts: 3,886 to 3,892;
- mapped scanner facts: 83 to 89;
- unmapped scanner facts: unchanged at 3,803.

After refreshing only those measured constants, the three owning modules ran
43 tests in 406 seconds with zero failures. This was a baseline refresh for
new product tests, not a scanner or validator behavior change.

## Report refresh

All 15 existing report generators ran from clean source commit
`a0943c22aae73de427f50e730303a6d54c19ade2` with timestamp
`2026-07-14T16:23:25+02:00`. The worktree was restored to clean before every
generator. Every generated pair passed its at-rest validator before staging,
and all 15 imported pairs passed their validators again in the consolidated
checkout.

| Report | JSON SHA-256 |
| --- | --- |
| Test-class completeness | `542532f387fdcc0ffd5ec7f6c7bbc5a11f91a37731bdabc0df200f054ee88d64` |
| Coverage-matrix gaps | `94e1dc2190b782de621204db333c478cf60b8faa963931955a49ee61edd7e364` |
| Malformed-input coverage | `2107e5f638706fbfe7e186ce3eb78ab31802e78e2262f744a338d25e0f089fc1` |
| Unmapped-test debt | `e796faa15388e4292efaa965f888227f3d0c834ccdbac27ca937ad45d9fd1800` |
| Test-refactor assessment | `15ff05df1fddd83503c24b6821a65b99909f8504f34d012caf3ca363ecc48430` |
| Ignored-test inventory | `2c489a18f90b1f48f04a81922901e67bafc6bcae4047abc500b750bbc5025e89` |
| Invariant-seed audit | `8f8061a40b157e0abd0322c92b4441b389d98c73c07fa43175035fe7da276456` |
| Specification completeness | `e1b6d7f8ef11fe18398953b91a36da4973c98aee2487d310d1991f7cae37ffca` |
| Phase 5 suite audit | `c1dd3b4b1158975db50d47902062accbe01fc4e7cb223685132c4f4db1ee8427` |
| Requirement/oracle audit | `4c8c458136d1b4323b23e21e212735122699c31e7c98961a2a7ff385b893e579` |
| Conformance alignment | `b8947374f99c0757393a19befb07a1753d571116276331f68449f64c22cba254` |
| Runtime-anomaly audit | `4b6761cbc4a90ba8b3f1c5e8c78caf74e37730f9c0602524c2c8dab6a1d342cb` |
| Fuzz-program audit | `0b0093c0631c14b76097bf7286500afe1568a47d18960603b86794165b20c95d` |
| Mutation program | `3e8ce55624abe58aa69aa699886daa51fd4af18eb239922a0b14cfce7bbfdbff` |
| Specification-source audit | `b7095dd32fc4cf5c94353355bd590117da6431cca98e3a81f640f00ca201143a` |

## Remaining blocker

The written source oracle cannot replace the current gap oracle without
changing the proof-contract digest and invalidating the historical authentic
red/green pair. The frozen verification control plane has no reviewed migration
for that transition. `SPEC_GAP_RUNTIME_FORCE_LIFECYCLE_001` therefore remains
`test_mapped`, with `source_oracle_proof_contract_transition` and a case-backed
broad gate recorded as explicit debt.

## Boundaries

- No runtime product behavior changed.
- No suite, approved proof producer, CI workflow, validator, schema, skill, or
  agent instruction changed.
- The existing proof-bound case file and its red/green evidence are unchanged.
- The new tests use existing runtime/debug/control APIs and add no production
  abstraction or ownership change.

## SOLID/KISS/DRY review

- The integration file owns runtime lifecycle boundaries; the control unit
  test owns token-rotation request semantics.
- Shared setup stays in one small integration-test harness function.
- No production module grew and no duplicate product implementation was added.
