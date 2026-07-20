# Phase 16 Execution Slice 001 Timer Validation

Date: 2026-07-12
Branch: `plc-verification-program`
Frozen lifecycle foundation: `18e7da19ef9a63f5f7582910791a50fa0923662f`
Final report source: `52970664daff57a0686c1fe996422b6ba0b38d1e`
Validated report checkpoint: `cd96bba8d3830380078d504ca1be802016480ba6`
Proof posture: targeted red/green plus broad remote G2; independent closure pending

## Product And Written Contract

The product fix is commit `bea129001b4a2b6c511c01d6d0ce1a3d27097bd6`.
`Tof::step` now latches `ET = PT` when the off-delay expires and preserves that
plateau on later executed calls while `IN = FALSE`. An executed `IN = TRUE`
call rearms the instance and resets `ET`. The change is confined to the shipped
timer implementation and its direct expectations.

`docs/specs/08-standard-function-blocks.md` defines the executed-scan timer
contract and the IEC Figure 15(c) TOF plateau. `docs/IEC_DECISIONS.md` records
the reviewed implementation boundaries. Restart, PT-change, skipped-call,
nonmonotonic-clock, TP retrigger, and TP short-input assertions are deliberately
absent from the first trace vertical.

The workspace release version is `0.24.30`; the root and VS Code versions,
`Cargo.lock`, `CHANGELOG.md`, and standard-function coverage note are
synchronized. Tagging and public release remain post-merge release work.

## Trace And Proof Chain

`TEST_IEC_TIMER_TRACE_001` runs the committed hand-authored case file
`verification/cases/compiler_iec/IEC_TIMER_001.toml` through real Structured
Text compilation and the shipped runtime timer implementation.

- Case-file digest:
  `sha256:68567779c24d44fc1ce1cda7696086aac6284b5788305511d2f416f66e12be50`
- Trace-definition digest:
  `sha256:9099114b1a5cfdfe880972030b8b52530814e009ce193392721eb675ccd38521`
- Proof-contract digest:
  `sha256:f01907e34e0eb5acf6279ec45a6fe5927615b235ee538afbc374bb781e622842`
- Red evidence `EVID_TEST_IEC_TIMER_TRACE_001_RED` binds clean commit
  `05a328fd1fc83bdd85e504885377f584ef33d967`: only the TIME and LTIME TOF
  post-expiry cases fail; the TON and TP protective cases pass.
- Green evidence `EVID_TEST_IEC_TIMER_TRACE_001_GREEN` binds clean descendant
  `bea129001b4a2b6c511c01d6d0ce1a3d27097bd6`: all four cases pass with the
  unchanged case file and proof contract.
- Broad evidence `EVID_BROAD_REMOTE_PR_20260712_7108E6781D88` binds clean
  local and remote commit `accb3393ae727c28a440a28318a027367b9d7b90`,
  records all four cases passing, and qualifies `IEC_TIMER_001` for G2.

The timer specification gap is closed by the existing active, oracle-eligible
source. `IEC_TIMER_001` is `implemented/G2` with the `pr` gate. Its
`ordering_or_lifecycle` cell is covered; `time_or_clock_fault` remains
`gap_open`, and `unasserted_timer_boundary_cases` remains explicit debt.

## Targeted And Vertical Validation

The minimal timer checks passed after the product fix:

- `cargo test -p trust-runtime --test fb_timers`: 1 passed.
- `cargo test -p trust-runtime --test fb_timers_full`: 1 passed.
- `cargo test -p trust-runtime --test iec_timers`: 1 passed.
- `cargo test -p trust-runtime --test iec_timer_trace_cases timer_state_machine_trace_cases -- --exact`:
  1 passed, all four committed cases passed.

The required runtime verticals passed on `trust-builder` at clean commit
`accb3393ae727c28a440a28318a027367b9d7b90`:

- `cargo test -p trust-runtime --test api_smoke`: 3 passed.
- `cargo test -p trust-runtime --test debug_control`: 18 passed, 2 ignored.
- `cargo test -p trust-runtime --test complete_program`: 1 passed.
- `cargo test -p trust-runtime --test runtime_reliability`: 4 passed.

The producer-authentic broad command ran on `trust-builder` with the reviewed
shared target and temporary directories. `just fmt`, `just clippy`, and
`just test-all` all exited zero, followed by a stamped run of the cataloged
timer trace. The gate took 643,553 ms; preflight recorded 63,879,888 KiB
available under `/home/johannes` and 4,153,028 KiB under `/tmp`.

## Report Reproduction

All 14 pairs were regenerated from pristine detached worktrees at
`52970664daff57a0686c1fe996422b6ba0b38d1e` with timestamp
`2026-07-12T20:22:00+02:00`. Each generator was immediately followed by its
at-rest validator. The installed bytes were replayed at clean checkpoint
`cd96bba8d3830380078d504ca1be802016480ba6`.

| Report | JSON SHA-256 | Markdown SHA-256 |
| --- | --- | --- |
| P2 test-class completeness | `7ef033a4f8f669fea96e429f40e18159806857e30af046568a621041d5c33a0c` | `69c3576c725f5e82bba3df853167bd4f083f2391e468bbd4f581d69f5b3f7b77` |
| P2 coverage matrix | `d654ced54ff4963487145ac865a07ff09fb93174f988cce33e1c5479f133cb4d` | `bdd0b18b01e1fee0da64ed8b729fe59134774e5cde643a3d16749dbde0348c1c` |
| P2 malformed input | `613ec66219721b8dfe285f1f55827ee9805247e687e0fbea432a7fe92cd47777` | `5d15dff433b6c9a3c90b9b9e0948bbb530f085fb0c8e6eebef6168f8f36d4ad7` |
| P2 unmapped debt | `bd1d34b18b9aba1cdb11a0d968481c2e10e7c95410604843af7f9e4248c2dfd3` | `058a309217887f627b6c00befc81910074b5bada68f73580c43f863c3f6ccbd7` |
| P2A refactor assessment | `5f051dd4c65cd1258237c6ee50122e3d144f7df862826ddc458d7db3d0609240` | `bcc3e305e49a1116c30a6d7c59725620bed0fce5757d6cc5d75694e634b150dd` |
| P3 ignored inventory | `e78aec35906192d1cba72227b2eb088b85e27cc875eed4a17272507f12414b83` | `3dcf8048c45ed8788cde9f62524fc4d9d33bf60eb881b7f484d5490f927a0278` |
| P4 invariant seeds | `2ba0bb7ed680cbc3c9a7f4302fc4ffb9dee3df42a6d9bef369e4d56085a04eae` | `005aeb701b5815e0c49be20463f800be4ce7691d18d9944e555cc588073b9fe0` |
| P4A specification | `3fbd28379ef55546213bb424ebb47efe11da35baee0eb536d8191da69d51d474` | `5a251ef0f4f8fe5c5fa5578779a1ebbf26e869ada7fbc753d7444e5239855501` |
| P5 suites | `3339cf243ffbc518f4267c438c65ae2c283016ff1a401d1b3da2f5e3919c0a02` | `e264f58d43dbd14f49cff4b1df6cd4ce2b09912980e28addff6b964b546e1d18` |
| P6 oracle audit | `2cbedcb0661f4768a396b8f9c4b1722a29cc7edd9f38082460d25082f1451c83` | `ac1d8c8d9a4271903ba00de2a24e7762f9f80425d6a2364c52d1cb5cde4942a6` |
| P7 conformance | `918e2d88f318b739f8cd1094a9fa7f2a9930ddd535250aa69848b84f51f7227e` | `35a133c11d210466089feec5056835e3d5be80696c667642a0e97dc08db7f645` |
| P8 anomaly audit | `faa1f2b9394f0df03ba72c4f8e91e54103715d56478d3bd7896c14e103af90fe` | `cbb4b8aba685df0e960d662df76221d3f2514b90edf8bb0db7b5ba1382f62eb8` |
| P9 fuzz audit | `750f8adfe1a3a109edae774ed34304fd80db2fd6d7d2a08a9898c237f33683d5` | `272546e60e7f7bf2070dabe4a9c6dbb692dbfae7ed827ea6632d41e69d673545` |
| P10 mutation program | `77c970a570b91e235afae9d7fba6f6663b1a120c17e231d0fe5c76bef13c0067` | `412afa61fd02d0330f39a5f40c0a96f88545fb5775eff5e575bbefcf34304a7d` |

## Final Verification Posture

At clean checkpoint `cd96bba8d3830380078d504ca1be802016480ba6`:

- `python3 scripts/run_verification_focused_tests.py`: 716/716 passed in
  815.819 s.
- `scripts/verification_metadata_gate.sh`: 347 records; Phase 16 fence green
  with zero changed paths.
- `python3 scripts/check_verification_tooling_selftests.py`: 27/27 fixtures
  passed.
- All 14 installed report pairs passed their at-rest validators.
- `git diff --check` passed and the detached validation worktree stayed clean.

The program now has 34 specification-gap records: 33 open and the timer gap
closed. Of 52 invariants, 51 remain S0 and only `IEC_TIMER_001` is G2. No
invariant is marked validated. `VERIF-E1-010` and `VERIF-P16-001` remain open
until independent review accepts this slice and authorizes atomic checklist,
board, and guard-pin closure.

No suite or `approved_proof_producers` change occurred after frozen commit
`18e7da19e`. This execution work added no validator, module, schema, board row,
lifecycle phase, workflow, skill, or agent-instruction change.
