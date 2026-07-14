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
source commit `c935e82b209b8dabab17f17e398b4dc5fc5ab5b6` with timestamp
`2026-07-14T14:35:59+02:00`. Each generated pair passed its at-rest validator
in its own worktree. The consolidated imported set then passed all 15 at-rest
validators again with 416 metadata records.

| Report | JSON SHA-256 |
| --- | --- |
| Test-class completeness | `802dcbbb1f987c8956764fa4758de04706ef04e05e2e22289050749085c65354` |
| Coverage-matrix gaps | `14a579448ca4726f32bd22c87ad21f676e9b0f3b3fffe175f0ae0cd274822510` |
| Malformed-input coverage | `838928ac6f4110dbab8a8923cfe019b25d2b7d79e72e9211037069b6e2dbd986` |
| Unmapped-test debt | `2cdff6dd33b24c33eaf762a378ebe366fa4659a9846b55cd73570dd3b2edfd90` |
| Test-refactor assessment | `9cc6a2f86fe4ce39abe4edc30afce765fe56dd676845dea4090cc06eacc2ee81` |
| Ignored-test inventory | `769a0915fe2eb8c56ff9d7a538609aa0c6e646d5cce696bfc75173cb16ddd9db` |
| Invariant-seed audit | `31bcb2c83f14814672cc3a405add052bb4697d5bf6bf35673d32a62c7962c510` |
| Specification completeness | `ccce74f6f34709bc24b5019615c512ac50e950561dbb3c50f6aadf8a10a111f1` |
| Phase 5 suite audit | `d33e95aad0e51ffeb92b535b05784ee79064f8bda98a608f45352169879dadaa` |
| Requirement/oracle audit | `c5976e7d24670670d5be9004879b8ab5b60904307654cebd9fad37e195e26535` |
| Conformance alignment | `5461b5109bbb937898a7d151fd95d9b14c8f5bc71b996d10635074b2f1c7420a` |
| Runtime-anomaly audit | `a5669fb97aca4c6598406651c24e13401165d0342ad1a5493458bf045a3cb75c` |
| Fuzz-program audit | `046cfeb1afb07b6914d57383ac5c9be7e223118d77483e1c755f42c0448f860b` |
| Mutation program | `0a3fdb311e683eb512439758cc7e8163df6cf169361683c322f2ca0d24ad666f` |
| Specification-source audit | `012f754e08b29fa0cc3cffb48e7b56123f5c162869c598bc2fe22f01633c6ae4` |

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
