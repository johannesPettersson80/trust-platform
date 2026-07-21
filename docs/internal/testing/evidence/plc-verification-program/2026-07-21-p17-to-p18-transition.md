# Phase 17 to Phase 18 Transition Evidence

Date: 2026-07-21

Verdict: `CLEAR`

This evidence closes only the Phase 17 denominator and fail-closed enforcement
transition. It does not claim semantic, test-mapping, coverage, mutation,
hardware, UX, platform, or PLC verification-program completion. Those remain
owned by the accepted Phase 18 zero-debt board.

## Frozen candidate

- canonical source checkout: `/home/johannes/projects/trust-platform`
- validation host: `trust-builder` (`scena-rust-builder`)
- validation checkout:
  `/home/johannes/projects/trust-platform-p17-transition-a74efe09`
- branch posture: detached clean checkout
- exact HEAD: `a74efe093d4a9fbb60e3d4cf3fd89dbaf8c2ce35`
- accepted contract parent: `7c08f5a8fd31fb2f4c770d316656f0b78ff62673`

The canonical agent inputs were manually copied into the destination before
testing and matched exactly:

| Input | Verification |
| --- | --- |
| `AGENTS.md` | SHA-256 `d0b35bc3edf19dc1d276202b6058ac6ad2835c6552fafbe71c04eb262a88530e` |
| complete `.codex/skills/**` tree | 25 files; aggregate SHA-256 `a595ab028d85241bc031148f44604245e027d9c320d858150bc460aaf740b25f` |
| `.claude/hooks/pre_tool_use_gate.py` | SHA-256 `be49797cabae8807d39c41e3c0d75b1dd527ada4e2c8889120aab6d7f7ec911d` |

The destination `AGENTS.md`, `trust-test-authoring`, and
`trust-remote-builder` instructions were read after the copy verification.

## Focused suite and dependency repair

The first exact-SHA run of:

```bash
python3 scripts/run_verification_focused_tests.py
```

executed all 1,008 tests and reported one error plus five failures. All six
were the same invalid environment result: the isolated checkout could not load
the lockfile-pinned TypeScript compiler API. This did not count as a behavior
red or product failure.

The destination installed the exact VS Code lockfile dependency set with:

```bash
cd editors/vscode
npm ci --ignore-scripts --no-audit --no-fund
```

The installed tools were Node `v22.22.1`, npm `9.2.0`, and TypeScript `5.9.3`.
The two affected focused modules then passed 7/7 tests in 2.251 seconds. The
complete suite was rerun without source or metadata changes and passed:

```text
Ran 1008 tests in 2710.727s
OK
```

## Transition checks

All commands ran on the same clean detached HEAD with the warmed
`CARGO_TARGET_DIR=$HOME/.cache/codex-targets/trust-platform-gate` and a
home-backed `TMPDIR`.

| Command | Result |
| --- | --- |
| `scripts/verification_metadata_gate.sh` | passed; 880 records; Phase 16 fence found zero changed product paths; every registered generated case reproduced |
| `python3 scripts/check_verification_tooling_selftests.py` | passed 33/33 fixtures |
| `python3 scripts/check_test_catalog_staleness.py` | passed; 269 committed records checked against 4,038 scanner facts |
| `python3 scripts/code_spec_coverage_gate.py --mode full --out-dir target/gate-artifacts/p17-transition-a74efe09/code-spec-full` | passed; 15,229 facts; zero findings; zero blocking |

The full code-spec artifacts are:

| Artifact | SHA-256 |
| --- | --- |
| `code-spec-coverage.json` | `7271694c9f6de5254929f06321d68319d4f2353dde33c29bed0e5aa71f1118b8` |
| `code-spec-coverage.md` | `2420d378e911b6b3a23a027720be5d66040f6ac76fb63abbb58f47d2a373b7dc` |

An additional changed-mode probe was rejected with
`code-spec coverage refuses to weaken configured full enforcement`. That is
the required fail-closed behavior after the enforcement flip; it is not used
as passing product proof. The complete changed-path planner had already
returned `clear` for the transition commit's instruction, board, evidence,
test-guard, and metadata paths.

The remote worktree remained clean after validation. No Phase 17 broad
`fmt`/`clippy`/`test-all` milestone was run. The accepted Phase 18 contract
owns the single final broad candidate milestone at `VERIF-P18-016`.

## Authorized transition

- `VERIF-P17-012` may close as denominator/enforcement proof only.
- `VERIF-STOP-017A` and `VERIF-STOP-017B` may close with their policy preserved.
- `VERIF-P18-001` is the next executable row.
- The 15,138 live `spec_gap` code facts and 3,777
  `reviewed_nonmapping` test facts remain blocking Phase 18 debt.
