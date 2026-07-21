# Phase 18 Fact-Level Burn-Down Ledger Evidence

Date: 2026-07-21

Verdict: `CLEAR` for `VERIF-P18-002` only

This evidence accepts complete debt ownership and deterministic slice order. It
does not accept any product semantic oracle, test mapping, execution,
assertion-strength, coverage, mutation, campaign, environment, hardware, UX,
platform, release, or final-program result. Every generated slice remains open.

## Frozen identities

- canonical source checkout: `/home/johannes/projects/trust-platform`
- source branch: `plc-verification-program`
- artifact extraction source:
  `51e742cfc9dda41ed40d55ed6088f26482dd425f`
- exact committed artifact and validation producer:
  `103a619719da3f9cc2de2ce45d0b5b7f337042bc`
- validation host: `trust-builder` (`scena-rust-builder`)
- validation checkout:
  `/home/johannes/projects/trust-platform-p18-substrate-186b9359`
- validation posture: detached and clean

The two source identities are intentional. The generator ran only after all
producer, schema, specification, test, gate-inventory, catalog, code-map, and
reviewed contract inputs were committed and clean at `51e742cf`. The following
commit contains the generated artifact and its exact control-ledger digest. The
at-rest checker proved that every bound input byte still equals the asserted
extraction commit and that `51e742cf` is an ancestor of the exact validation
producer. The mutable execution board is an output/evidence surface and is not
part of this closure; the reviewed metadata contract is the semantic input.

After the remote checkout moved to `103a6197`, canonical agent inputs were
manually recopied and verified before validation:

| Input | Verification |
| --- | --- |
| `AGENTS.md` | SHA-256 `d0b35bc3edf19dc1d276202b6058ac6ad2835c6552fafbe71c04eb262a88530e` |
| complete `.codex/skills/**` tree | 25 files; aggregate SHA-256 `a595ab028d85241bc031148f44604245e027d9c320d858150bc460aaf740b25f` |
| `.claude/hooks/pre_tool_use_gate.py` | SHA-256 `be49797cabae8807d39c41e3c0d75b1dd527ada4e2c8889120aab6d7f7ec911d` |

The destination verification-architecture, test-authoring, SOLID, and remote
builder instructions were already read. The tracked checkout remained clean
after validation.

## Ledger contract and arithmetic

The compact canonical artifact is
`verification/p18/burn-down-ledger.json`. It is a generated overlay over the
authoritative live scanners and metadata, not another hand-maintained source
inventory. It contains exactly one open slice for each live identity:

| Population | Current count |
| --- | ---: |
| code facts | 15,274 |
| test facts | 4,040 |
| code `spec_gap` | 15,138 |
| code `registered_backlog` | 15,135 |
| test `reviewed_nonmapping` | 3,777 |
| ignored or conditional tests | 23 |
| unresolved test accounting area | 337 |
| unowned slices | 0 |
| duplicate identities | 0 |
| omitted identities | 0 |

The immutable Phase 17 historical baseline remains 15,229 code facts and 4,038
test facts. New Phase 18 tooling was specified and catalog-mapped immediately;
it increases the current denominators without increasing the original debt
populations. The 337 unresolved test areas retain all matrix candidates, are
owned by `verification` for remediation, and carry an explicit domain-area/
owner review blocker. They are not silently assigned or waived.

Every code and test row records all twelve Phase 18 budget identities, exact
dependency order, at least one environment state, an accountable owner, risk,
current authority/mapping state, and nonempty blockers. No row groups multiple
live identities. The first deterministic semantic slice is:

```text
P18-CODE-CODE_F72E3E7BC892973CF9344497078A35D93380745CDC5DB4820F5CFB302AAEE14A
crates/trust-runtime-core/src/numeric.rs
trust_runtime_core::numeric::signed_from_i128
```

It remains bound to the runtime-safety historical backlog and cannot proceed
beyond P18-003 until an authoritative numeric conversion/failure contract is
reviewed without consulting implementation behavior.

### Code-area census

| Area | Facts | `spec_gap` | `registered_backlog` |
| --- | ---: | ---: | ---: |
| runtime safety | 1,670 | 1,668 | 1,668 |
| control/security | 2,408 | 2,405 | 2,405 |
| bytecode/VM | 1,228 | 1,228 | 1,228 |
| compiler/IEC | 1,587 | 1,502 | 1,499 |
| protocols | 1,546 | 1,546 | 1,546 |
| editor safety | 3,305 | 3,305 | 3,305 |
| HMI/UI | 765 | 765 | 765 |
| PLCopen/devtools | 353 | 353 | 353 |
| supply chain/platform | 237 | 237 | 237 |
| release | 21 | 21 | 21 |
| verification | 2,154 | 2,108 | 2,108 |

## Focused exact-commit proof

The remote toolchain was Python `3.14.4`, Node `v22.22.1`, npm `9.2.0`, and
lockfile-installed TypeScript `5.9.3`.

| Command | Result |
| --- | --- |
| `python3 scripts/p18_burn_down_ledger_gate.py` | passed; 15,274 code facts; 4,040 test facts; exact source ancestry, input bytes, canonical output, and arithmetic |
| `python3 -m unittest scripts.verification.p18_burn_down_ledger_tests scripts.verification.p18_zero_debt_tests` | passed 12/12 |
| `python3 scripts/p18_zero_debt_gate.py --mode report --out-dir target/gate-artifacts/p18-zero-debt/103a619719da3f9cc2de2ce45d0b5b7f337042bc` | passed; report-only; zero contract findings; every debt counter preserved |
| `python3 scripts/validate_verification_metadata.py` | passed; 884 records |
| `python3 scripts/check_test_catalog_staleness.py` | passed; 271 committed records against 4,040 scanner facts |
| `python3 scripts/check_diagram_drift.py` | passed after remote rendering |
| `TMPDIR=... RUSTC_WRAPPER= python3 scripts/code_spec_coverage_gate.py --mode full --out-dir target/gate-artifacts/verification/code-spec/p18-burn-down-103a6197` | passed; 15,274 facts; zero findings; zero blocking findings |

The focused catcher suite rejects missing and unknown nested fields, omitted or
duplicate live identities, noncontiguous order, summary drift, empty owner/
blocker/environment/budget/dependency obligations, omission of any proof
budget, an invented/unavailable source SHA, a non-ancestor source SHA, changed
input bytes, a dirty checker worktree, and noncanonical generated bytes. A
provenance dry run also exposed that binding the mutable board would create a
status-update cycle; the final contract instead binds the reviewed metadata
model, and the exact cycle-free artifact was regenerated and revalidated.

## Artifact identity

| Artifact | SHA-256 |
| --- | --- |
| `verification/p18/burn-down-ledger.json` | `b8a5078049d2c09e4b1a3831c9db86cbd3c0a9713409cfa114a47c0b7cdc741c` |
| `verification/p18/bootstrap/test_mapping.json` | `9fbdaffdbf835eaca9e5e69aa903ebf64b50f71f1b5506ba987ab9c3b4626491` |
| `verification/schemas/p18-burn-down-ledger.schema.json` | `0b13f9119765c3157bf1c4d3b2f1def4d8b28492fa395b9a89551eb60e413400` |
| `docs/diagrams/generated/verification-p18-zero-debt.svg` | `2b27eb7db7ae27c64181f0d102929841677dc702bbd52cc1ed09de289ef36ced` |
| P18 `p18-zero-debt.json` | `8857f6f24d05b65b76d7a70b3e40155c05765cba8a43195ff8fd10ebb1758a54` |
| P18 `p18-zero-debt.md` | `d4caa832e55f909036bd63fbaeadb062db2dd20bbdcd9ad40de2749e27c0a3ae` |
| P18 `artifact-index.json` | `b6005355b10ace0b077ea9e7939cd8f36d97b60d63163c11ebd7cd5c660c5137` |
| full `code-spec-coverage.json` | `36377fc6932349a19434878ca1e3acee5b276f0a47e39a3b64252deb1f7b5023` |
| full `code-spec-coverage.md` | `16ef155315c5bf18e0bab3fa987ccc58701739a21a3f42704deba94f814ea976` |

The P18 artifact index contains 20 rows and binds its closure to
`170fd774f075b5f1dadcc63bd8c58c6e960c6259003ce72326ea2193a7f1d8d5`.
Its producer source is exact `103a6197`; its frozen contract source remains
`a74efe093d4a9fbb60e3d4cf3fd89dbaf8c2ce35`.

No broad `fmt`, `clippy`, or `test-all` milestone ran. The accepted board
reserves that single frozen-candidate milestone for `VERIF-P18-016`.

## Acceptance

`VERIF-P18-002` is clear because every current code and test fact now has one
deterministic, owned, budget-bound, dependency-ordered open work slice and all
counts reconstruct from the raw identities. No debt counter decreased. The
next active work is `VERIF-P18-003` on the first exact semantic slice above.

