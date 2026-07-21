# Phase 18 Zero-Debt Substrate Evidence

Date: 2026-07-21

Verdict: `CLEAR` for `VERIF-P18-001` only

This evidence accepts the Phase 18 machine-readable control plane. It does not
close or waive any semantic, test-mapping, execution, adequacy, hardware, UX,
platform, release, or final-program debt. `VERIF-P18-002` is the next row.

## Frozen producer commit

- canonical source checkout: `/home/johannes/projects/trust-platform`
- source branch: `plc-verification-program`
- validation host: `trust-builder` (`scena-rust-builder`)
- validation checkout:
  `/home/johannes/projects/trust-platform-p18-substrate-186b9359`
- validation posture: detached and clean
- exact producer HEAD: `e23e113d6025bdf2767534e77e70d8fc57f46443`
- frozen Phase 18 contract source:
  `a74efe093d4a9fbb60e3d4cf3fd89dbaf8c2ce35`

The producer commit was transferred as a Git bundle. After the checkout moved
to that commit, the canonical agent inputs were manually recopied and verified
before validation:

| Input | Verification |
| --- | --- |
| `AGENTS.md` | SHA-256 `d0b35bc3edf19dc1d276202b6058ac6ad2835c6552fafbe71c04eb262a88530e` |
| complete `.codex/skills/**` tree | 25 files; aggregate SHA-256 `a595ab028d85241bc031148f44604245e027d9c320d858150bc460aaf740b25f` |
| `.claude/hooks/pre_tool_use_gate.py` | SHA-256 `be49797cabae8807d39c41e3c0d75b1dd527ada4e2c8889120aab6d7f7ec911d` |

The destination instructions for verification architecture, test authoring,
SOLID module ownership, and remote validation were already read for this
checkout. The destination remained clean after proof generation.

## Toolchain and environment

| Tool | Version |
| --- | --- |
| Python | `3.14.4` |
| Node.js | `v22.22.1` |
| npm | `9.2.0` |
| lockfile-installed TypeScript | `5.9.3` |

The exact lockfile dependencies had been installed with `npm ci` during the
parent substrate validation. An initial environment probe invoked `npx tsc`
from the repository root, where TypeScript is not declared; npm selected the
unrelated deprecated `tsc` package and stopped before any test or gate ran.
The probe was corrected to
`editors/vscode/node_modules/.bin/tsc --version`. No source or metadata changed,
and all proof commands below then ran from the same clean producer commit.

The earlier parent-commit full code-spec attempt had also exposed a builder
`/tmp` quota failure. The exact producer validation therefore used
`TMPDIR=/home/johannes/.cache/codex-tmp/p18-e23e113d` and an empty
`RUSTC_WRAPPER`; this is an environment correction, not a source change.

## Focused proof

| Command | Result |
| --- | --- |
| `python3 -m unittest scripts.verification.p18_zero_debt_tests` | passed 7/7 |
| `python3 scripts/p18_zero_debt_gate.py --mode report --out-dir target/gate-artifacts/p18-zero-debt/e23e113d6025bdf2767534e77e70d8fc57f46443` | exit 0; `report_only`; zero contract findings |
| `python3 scripts/p18_zero_debt_gate.py --mode strict --out-dir target/gate-artifacts/p18-zero-debt/strict-probe-e23e113d` | required exit 1; `open_debt`; zero contract findings |
| `python3 scripts/validate_verification_metadata.py` | passed; 882 records |
| `python3 scripts/check_test_catalog_staleness.py` | passed; 270 committed records against 4,039 scanner facts |
| `python3 scripts/check_diagram_drift.py` | passed |
| `TMPDIR=... RUSTC_WRAPPER= python3 scripts/code_spec_coverage_gate.py --mode full --out-dir target/gate-artifacts/verification/code-spec/p18-substrate-e23e113d` | passed; 15,258 facts; zero findings; zero blocking findings |

The focused negative suite proves rejection of required-field removal and
unknown schema fields, missing or stale inputs and raw artifacts, wrong
validator ownership, disabled catchers, unclosed prerequisites, weakened
budgets, duplicate identities, zero execution, failed producers/validators,
arithmetic inconsistency, stale source identity, open strict-mode debt, and a
dirty producer tree.

## Canonical artifact identity

The report independently records producer identity
`e23e113d6025bdf2767534e77e70d8fc57f46443` and frozen contract identity
`a74efe093d4a9fbb60e3d4cf3fd89dbaf8c2ce35`. Its artifact index contains 20
rows and binds the normalized input closure to
`fe98c106c83c467286221854bed2fe095a897e5350a5717fce318414d658c098`.

| Artifact | SHA-256 |
| --- | --- |
| `target/gate-artifacts/p18-zero-debt/e23e113d6025bdf2767534e77e70d8fc57f46443/p18-zero-debt.json` | `62004de2bf85e72a3d37be6e5967cfc04367babe2d8b11246e1b1d944fd70dfb` |
| `target/gate-artifacts/p18-zero-debt/e23e113d6025bdf2767534e77e70d8fc57f46443/p18-zero-debt.md` | `fe3670dec2916de689bb1f0aafff30473762603db3e03ef13b3428702da60fae` |
| `target/gate-artifacts/p18-zero-debt/e23e113d6025bdf2767534e77e70d8fc57f46443/artifact-index.json` | `97b198e747d6485d52e6dffdefb6b2ed6e5a45b5f429d08869eef147c991b11a` |
| `target/gate-artifacts/verification/code-spec/p18-substrate-e23e113d/code-spec-coverage.json` | `a8ae1f2bfe112efb11970327160e5bfd68203658141d993c6caf7128a5cb61f7` |
| `target/gate-artifacts/verification/code-spec/p18-substrate-e23e113d/code-spec-coverage.md` | `c8b6c5dcff24341037e6cbb29889e505cb613a536454dba26300895c02bf9e38` |

## Debt remains visible

The exact report status is `report_only`, not complete. It reconstructed these
open populations from the supplied raw artifacts:

| Debt population | Count |
| --- | ---: |
| `spec_gap` | 15,138 |
| `registered_backlog` | 15,135 |
| `reviewed_nonmapping` | 3,777 |
| ignored, skipped, filtered, or unrun | 23 |
| unmeasured proof classes | 10 |

The report has zero *contract* findings because the new control plane and its
inputs are internally valid. The counts above remain blocking debt and make
strict mode return `open_debt`. Structural code-spec enforcement similarly
accounts for every fact without converting the 15,138 historical `spec_gap`
dispositions into authoritative specifications.

No broad `fmt`, `clippy`, or `test-all` milestone ran. The accepted board
reserves that single frozen-candidate milestone for `VERIF-P18-016` after rows
`P18-001..015` close.

## Acceptance

`VERIF-P18-001` is clear: the closed schemas, ledgers, claim matrix, thin CLI,
separate proof-class modules, canonical source/input/artifact provenance,
report/strict modes, and focused catchers execute correctly on their exact
clean producer commit. No Phase 18 debt equation is closed by this verdict.
