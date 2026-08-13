# Phase 17 full-enforcement readiness

Status: implementation-side readiness evidence. This is not the independent
`VERIF-P17-006` or `VERIF-P17-012` verdict, and it does not authorize either
stop row to close.

## Frozen revisions

- canonical repository: `/home/johannes/projects/trust-platform`
- branch: `plc-verification-program`
- pre-baseline implementation SHA:
  `1aacc2094e66cc65019eddea14889f78269a2e05`
- immediate-child baseline SHA:
  `5ecc4626b0305d2b6d833c82f7fe9be29dc13bbe`
- zero-debt conversion SHA:
  `84af65851f186a187c79ef6367edab5d45301915`
- full-enforcement candidate SHA:
  `83b7a8549b7accd4d308df29ee7c8dacecb1b0bd`
- clean detached reproduction checkout:
  `/home/johannes/projects/trust-platform-p17-full-83b7a854`

The debt baseline still names the pre-baseline implementation SHA, contains
`entries = []`, retains `append_policy = "delete_only"`, and now uses
`enforcement_mode = "full"`. This record is retained only as historical
implementation-side readiness. Its former Phase 17 baseline-freeze companion
belonged to the later per-function campaign and has been removed; this document
does not provide current verification authority.

## Bootstrap and private input

The canonical source was `/home/johannes/projects/trust-platform`. The root
`AGENTS.md`, complete `.codex/skills/**` tree, and manifest-bound private hook
were copied manually into the detached checkout and compared with the source
before evidence generation.

- destination `AGENTS.md` SHA-256:
  `2808d20386ee2604bd20136f4b9b767686a491ba27f981d5cc94f1f6ed4f3be3`
- destination relative-path `.codex/skills/**` aggregate SHA-256:
  `fda1aaf8ceed69e8208f75fdc6484328ac8d63fea846de1f8f7f618cdd66ab68`
- `.claude/hooks/pre_tool_use_gate.py` SHA-256:
  `be49797cabae8807d39c41e3c0d75b1dd527ada4e2c8889120aab6d7f7ec911d`

The first report attempt in the new detached checkout failed closed with
`missing tooling source root .claude/hooks`. The report was rerun only after
the exact manifest-bound hook was copied and its digest verified. The detached
Git worktree remained clean because these bootstrap inputs are canonical
ignored files.

## Enforcement red and green

Before the configuration flip, the new repository contract test failed for
the expected assertion:

```text
python3 -m unittest \
  scripts.verification.code_spec_debt_tests.CodeSpecDebtTests.test_committed_zero_debt_baseline_enables_full_enforcement

AssertionError: 'full' != 'changed'
```

After changing only the empty baseline's configured mode, the same test passed,
and the complete debt-contract module passed five tests:

```text
python3 -m unittest \
  scripts.verification.code_spec_debt_tests.CodeSpecDebtTests.test_committed_zero_debt_baseline_enables_full_enforcement
Ran 1 test ... OK

python3 -m unittest scripts.verification.code_spec_debt_tests
Ran 5 tests ... OK
```

The focused CLI, debt, enforcement-policy, and report-contract selection then
passed 15 tests. The core metadata validator reported `879` valid records and
the Phase 16 product fence reported zero changed paths. A duplicate local run
of the full metadata wrapper was stopped while it was sequentially
regenerating every case table; the complete wrapper remains a required command
for the independent frozen-target reproduction and is not claimed green here.

## Exact full partition

The clean full-enforcement report contains 15,228 live facts and 15,228 exact
mappings with:

- 0 findings and 0 blocking findings;
- 0 frozen debt entries;
- 0 missing specification mappings, new missing mappings, missing tests,
  stale reviews, or unmapped facts;
- 5,762 risk acknowledgement references and 5,762 exact ledger records;
- 0 unresolved area-candidate sets.

Source-kind facts are 9,949 Rust, 2,256 TypeScript, 1,121 JavaScript, 1,821
Python, and 81 shell. Dispositions are 15,138 `spec_gap`, 55 `specified`, 30
`inherits_spec`, 2 `generated`, 2 `non_behavioral`, and 1
`vendored_external`. The large `spec_gap` count is intentional and does not
claim that historical behavior is specified; every such fact remains bound to
an exact same-area open gap for independent semantic review.

Canonical-area counts are:

| Area | Facts |
|---|---:|
| `bytecode_vm` | 1,227 |
| `compiler_iec` | 1,592 |
| `control_security` | 2,408 |
| `editor_safety` | 3,304 |
| `hmi_ui` | 764 |
| `plcopen_devtools` | 352 |
| `protocols` | 1,546 |
| `release` | 21 |
| `runtime_safety` | 1,669 |
| `supply_chain_platform` | 237 |
| `verification` | 2,108 |

## Canonical report and full gate

The commands were launched from the canonical checkout only so the
lockfile-pinned TypeScript installation remained available; every scanner,
input, provenance check, commit, and output root was explicitly bound to the
clean detached checkout.

```text
python3 scripts/code_spec_coverage_gate.py --root \
  /home/johannes/projects/trust-platform-p17-full-83b7a854 \
  --mode report \
  --out-dir target/gate-artifacts/p17-full-enforcement/report
code-spec coverage generated: 15228 facts; 0 findings; 0 blocking in report mode

python3 scripts/code_spec_coverage_gate.py --root \
  /home/johannes/projects/trust-platform-p17-full-83b7a854 \
  --mode full \
  --out-dir target/gate-artifacts/p17-full-enforcement/full
code-spec coverage generated: 15228 facts; 0 findings; 0 blocking in full mode
```

Both commands exited 0. The report input-closure digest was
`sha256:c20fd18907ee667e8fba0c0db7d3adbd2872f908e79d572b0c84ef22ed727a58`.
Artifact SHA-256 values were:

| Mode | Artifact | SHA-256 |
|---|---|---|
| report | JSON | `7da68ba17f6866e74d0a3621409bab95f93806826b24b8498f5e89d0d686debb` |
| report | Markdown | `a90f28ddf0a5d4afd01fd645385b33ba604f0663c0e00918ffd54e352f1a14f1` |
| full | JSON | `916e25720ee6c7faa9db5ed07a75dce7819141414832c9116fefce834d911c05` |
| full | Markdown | `71b60bfc7aa0aa3195ee3408221721ae4979e14203efa9f6871213256ea8b953` |

Both reports retain `proof_kind = "none"`, do not infer expected behavior,
create no product proof, close no specification gap, and promote no invariant.

## Changed-mode interpretation

The authoritative clean changed-mode reproduction remains the historical
baseline proof at `5ecc4626b0305d2b6d833c82f7fe9be29dc13bbe`, where the mode was configured
as `changed`. As an additional fail-closed check, changed mode at the later
zero-debt conversion SHA rejected six changed functions in
`scripts/verification/code_spec_debt.py` because those functions remained
explicitly mapped to the open verification historical gap. That nonzero result
is expected changed-mode policy behavior, not an unresolved map omission and
not a substitute for the protocol's baseline-SHA reproduction.

## Remaining acceptance work

- An independent reviewer must issue separate commit-bound `VERIF-P17-006` and
  `VERIF-P17-012` verdicts under `p17-independent-review-protocol.md`.
- The reviewer must inspect the raw rows; these implementation-side counts do
  not establish semantic acceptance.
- `AGENTS.md`, `trust-test-authoring`, the P17 checklist rows, and the stop rows
  remain unchanged until an independent verdict authorizes the reviewed
  enforcement commit.
- The single remote-builder `just fmt`, `just clippy`, and `just test-all`
  milestone remains deliberately deferred until focused independent review is
  accepted.
