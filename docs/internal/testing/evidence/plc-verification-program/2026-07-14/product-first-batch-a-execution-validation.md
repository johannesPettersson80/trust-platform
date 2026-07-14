# Product-First Batch A Execution Validation

Date: 2026-07-14

Validated report commit: `2c71d91302b779003bddd34c8c459e7d7ffdc6a3`

## Outcome

Batch A started from explicit runtime-safety debt and used the shortest honest
path for each item: write the missing contract when needed, identify the
smallest real product test, run it, and change product code only after a valid
red assertion.

| Vertical | Test result | Product result | Posture |
| --- | --- | --- | --- |
| Reload transaction | New trace failed before the fix and passed after it | A retain-store load error was returned only after live bytecode, storage, and scan state had already changed. The runtime now loads the retained snapshot before live reload mutation. | `RT_RELOAD_001` is `G2`; bytecode/resource-commit and retain-migration failure branches remain missing. |
| Panic containment | Four existing containment tests passed | The requested coverage already existed; no duplicate test and no product change were justified. | `RT_SAFE_PANIC_001` remains `S0` pending producer-bound status proof and a broad case-backed gate. |
| Automatic restart | Existing cold/warm tests and one new automatic-restart storage test passed | Non-panic automatic restart already applied warm-storage semantics; no product fix was justified. | `RT_SAFE_RESTART_001` remains `S0` pending a producer-bound restart trace and broad evidence. |
| Slow I/O | Existing delayed Modbus read/write tests passed | The Modbus worker path returned within the scan bound under a delayed peer; no product fix was justified. | `RT_SAFE_IO_001` remains `S0`; non-Modbus, hardware, and producer-bound latency proof remain missing. |

This batch found and fixed one product defect. It did not manufacture red proof
for the three passing verticals.

## Reload Defect Chain

| Step | Commit or evidence |
| --- | --- |
| Specify transactional reload | `79af5a25` |
| Add the case-backed runner | `bba0607c` |
| Producer-authentic red proof | `EVID_TEST_RUNTIME_RELOAD_TRANSACTION_001_RED` |
| Minimal runtime fix | `813b93e999` |
| Producer-authentic green proof | `EVID_TEST_RUNTIME_RELOAD_TRANSACTION_001_GREEN` |
| Close the written-spec gap | `dfdaeefb` |
| Broad PR gate | `EVID_BROAD_REMOTE_PR_20260714_1F7316C15C47` |
| Promote the proven slice to G2 | `31a4a691` |

Before the fix, the committed trace observed the rejected reload with the new
program and reset state already visible: the first post-error cycle had not
executed the old program, and the next cycle used the replacement executable
with reset storage and scan counter. `Runtime::reload` now loads the retained
snapshot before applying replacement bytecode, restarting storage, or applying
the snapshot. The same trace passes after the change.

The coverage cell stays `gap_open`. This proof covers retain-store read failure
before mutation, not every later bytecode/resource commit or retained-value
migration failure.

## Targeted Tests

On `trust-builder` at the corresponding clean commits:

- `cargo test -p trust-runtime --test reload_transaction_trace_cases reload_transaction_trace_cases -- --exact --nocapture`
  failed 1/1 before the fix and passed 1/1 after it.
- `cargo test -p trust-runtime --test runtime_safety_fail_closed panic_ -- --nocapture`
  passed 4/4.
- `cargo test -p trust-runtime --lib scheduler::runner_loop_poison_tests::cycle_execution_panic_faults_resource_visibly -- --exact`
  passed 1/1.
- `cargo test -p trust-runtime --lib scheduler::runner_loop_poison_tests::automatic_fault_restart_uses_warm_storage_semantics -- --exact`
  passed 1/1.
- `cargo test -p trust-runtime --test modbus_driver returns_within_scan_bound_while_response_delayed -- --nocapture`
  passed 2/2.

The panic unit command includes `--lib` deliberately. An earlier command that
omitted it attempted to link unrelated integration targets and exhausted the
builder filesystem; it was classified as infrastructure, generated output was
removed, and no product failure was claimed.

## Broad Validation

The reviewed producer ran once from clean local and remote commit
`e7e778218579d2c3fb6ce6116fdabba3fc9de181` with Rust 1.97.0 and Cargo 1.97.0:

```text
ssh trust-builder 'cd "$HOME/projects/trust-platform" && mkdir -p "$HOME/.cache/codex-targets/trust-platform-gate" "$HOME/.cache/codex-targets/trust-platform-gate-tmp" && export CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-gate" TMPDIR="$HOME/.cache/codex-targets/trust-platform-gate-tmp" && just fmt && just clippy && just test-all'
```

- `just fmt`: passed.
- `just clippy --all-targets --all-features`: passed with one pre-existing
  advisory `clippy::question_mark` warning in `trust-lsp`.
- `just test-all`: passed.
- Fresh stamped reload trace: 1/1 passed.
- Gate duration: 797,875 ms.
- Preflight: 89,522,184 KiB available under `/home/johannes` and 7,270,792
  KiB under `/tmp`.

The required warmed runtime verticals then passed:

- `cargo test -p trust-runtime --test api_smoke`: 3/3.
- `cargo test -p trust-runtime --test debug_control`: 20/20.
- `cargo test -p trust-runtime --test complete_program`: 1/1.
- `cargo test -p trust-runtime --test runtime_reliability`: 4/4.

## Report Refresh

All 15 report generators ran in separate pristine `trust-builder` worktrees at
source commit `31a4a6911d4788e8578a67d0e1b1d0e4463bb7fe` with timestamp
`2026-07-14T14:13:19+02:00`. Each generated pair passed its at-rest validator
in its own worktree. The consolidated imported set then passed all 15 at-rest
validators again with 416 metadata records.

| Report | JSON SHA-256 |
| --- | --- |
| Test-class completeness | `219088e177fb7abe468dceab6122625a917785c5af1d35b631d252400c056ce0` |
| Coverage-matrix gaps | `dcce4c1c909668c522fb73bf1e70f988c26f5ea4e9f6e9eb1338419d68ccc3cd` |
| Malformed-input coverage | `37d44f50bf2b3e69b431e602bf3f8e186417d5fc7c66d1e3f9c1fbc7adb413de` |
| Unmapped-test debt | `b9a2e58528742cbdf7ba3083f775d41b03c72054a79bd064204b5e16c56db7e4` |
| Test-refactor assessment | `e0a42e75a909a2c6d0c217748f32242eb10e1f7b25751ed78c5e849ee9ed56c4` |
| Ignored-test inventory | `dd337eab1095d0a8e5fabc407de2b7ff3601fed8ac1ed1ea909becd8711ba831` |
| Invariant-seed audit | `978cf210b1fb1fa42e816128e9893f712612064ef3767d6fbda0949260161165` |
| Specification completeness | `c3dfe868b25fa9d3d2bd01e9d6960811c7e09c323a6c5e38126a67cfd4fa08ec` |
| Phase 5 suite audit | `7508745be1f1beffbb7e159ee4dd4553bc3abcfc115f61c0e6380df6aca31fc7` |
| Requirement/oracle audit | `ffc6550dbbc699fe2d4f9555f6458fe714885033da933e5583261dc31f7ec1ca` |
| Conformance alignment | `87835a0bd60f1f06929bd91a04a61ddc365ca43ef508880efb37ea60742c4228` |
| Runtime-anomaly audit | `1609f68eeb086f4d5c016f0605e7f764c96056a2b1835c5b5e89c9b371870f44` |
| Fuzz-program audit | `2cdd2fb7716b7e2810e71a96315dd3a5a60b2276140f458dc1463ebb623d9f1e` |
| Mutation program | `dddbbaadf9ae717b8d5678fca4e0298ef326662200222ddee6b13a77b0fb971a` |
| Specification-source audit | `f64dc25808fa3365ec33da44da3fa512af1d7c2bc0449109d37ba7b3c20e48e4` |

## Measured Remaining Work

- Specification gaps: 34 total; 28 open, 1 test-mapped, 5 closed.
- Invariants: 53 total; 45 at S0 and 8 at G2.
- Existing-test census: 3,886 facts; 83 classified and 3,803 unmapped.
- Test-class slots: 5/32 complete; 27 missing.
- Specification coverage: 37/53 invariants still lack a bound written spec.
- Oracle coverage: 15/53 eligible; 38 missing.
- Ignored register: 40 ignored plus 2 conditional observations.
- Conformance: 21/21 cases still lack explicit catalog links.
- Runtime anomalies: 9 test-gap classes remain.
- Fuzzing: 6 required surfaces remain gaps.
- Mutation: 1/6 shards measured; five remain planned.

`VERIF-P16-002` through `VERIF-P16-008` remain open. The next product batch
must select the highest-risk rows from these measured denominators, not expand
the control plane or rerun broad gates before another batch milestone.

## Boundaries

- No CI workflow, suite definition, approved proof producer, agent instruction,
  skill, or verification schema was changed.
- Panic, restart, and slow-I/O results created no red/green or broad proof row.
- No unspecified behavior was invented; unresolved behavior remains in the
  named gaps and invariant `missing` fields.
- The release-hygiene checkpoint synchronizes workspace and VS Code package
  metadata at `0.24.40`; no tag or public release is claimed on this branch.
