# Phase 17 Independent Final Acceptance

## Verdict

`BLOCKED`

The frozen Phase 17 implementation is a mechanically complete, fail-closed
denominator and debt-baseline audit, but it is not final zero-debt verification
coverage. At the reviewed SHA, 15,138 of 15,229 code facts (99.402%) terminate
in `spec_gap`, and 3,777 of 4,038 discovered tests (93.536%) are
`reviewed_nonmapping`. Those records make the missing semantics and proof
visible; they do not replace them. Under the final program objective, neither
population may remain at completion.

This evidence authorizes the focused pilot row `VERIF-P17-006`: its complete
88-fact partition and every cited semantic edge were independently accepted.
It does not authorize `VERIF-P17-012`, `VERIF-STOP-017A`, or
`VERIF-STOP-017B`, and it does not authorize overall PLC verification-program
closure. The historical code and test populations require a mandatory
zero-debt burn-down phase before those rows can close.

This review creates no product proof, closes no specification gap, and
promotes no invariant.

## Reviewer and frozen target

- Reviewer: Codex independent review agent, separate from the implementation
  and classification agent.
- Canonical repository: `/home/johannes/projects/trust-platform`.
- Branch: `plc-verification-program`.
- Frozen review SHA: `ee00258c42cadd98ac70a5c6523957f5ee44fe7c`.
- Frozen implementation SHA:
  `1aacc2094e66cc65019eddea14889f78269a2e05`.
- Frozen baseline SHA: `5ecc4626b0305d2b6d833c82f7fe9be29dc13bbe`.
- Reproduction checkout:
  `trust-builder:/home/johannes/projects/trust-platform-p17-review-ee00258c`,
  clean and detached at the frozen review SHA.
- Historical baseline checkout:
  `trust-builder:/home/johannes/projects/trust-platform-p17-baseline-review-5ecc4626`,
  clean and detached at the frozen baseline SHA.
- Environment: `scena-rust-builder`, Linux x86_64, Python 3.14.4,
  Node.js 22.22.1.

The destination bootstrap was manually verified against the canonical
checkout before review:

| Input | Canonical and destination SHA-256 |
| --- | --- |
| `AGENTS.md` | `2808d20386ee2604bd20136f4b9b767686a491ba27f981d5cc94f1f6ed4f3be3` |
| complete `.codex/skills/**` tree, 25 files | `763feca7520f0c1cc47bb5884d8c9a3140889b91e2e9dc807271e94df6fa6341` |
| `.claude/hooks/pre_tool_use_gate.py` | `be49797cabae8807d39c41e3c0d75b1dd527ada4e2c8889120aab6d7f7ec911d` |

The private-hook digest is the sole exact tracked entry in
`verification/code-spec-private-inputs.toml` and equals the live digest. Its
two parser-derived facts resolve through exact map records: both expose the
registered private-hook gap, and the main function's
`broad_error_suppression` signal has an exact risk acknowledgement.

## Reproduced command evidence

All commands ran in the clean detached review checkout on `trust-builder`.
Logs are under
`/home/johannes/p17-independent-review-ee00258c.ypS8bx/` on that host.

| Lane | Result | Log SHA-256 |
| --- | --- | --- |
| Protocol's 17 focused `unittest` modules | 92 tests passed in 25.408 s | `e4320174a634f46b8230e8d57f0e6c6b611513a0f36b209716bf0e374a18fc44` |
| `python3 scripts/run_verification_focused_tests.py` | 1,008 tests passed in 2,752.094 s | `2ba15dc6d827939fce349e45c764adce5f8591172bff413429de0528deb4e96f` |
| `scripts/verification_metadata_gate.sh` | passed; 880 records; Phase 16 fence found zero changed paths | `154843a32f5d38e938ea94cb03867a4feb56ed4b07e5aec7e73ebeec2a9663f8` |
| `python3 scripts/check_verification_tooling_selftests.py` | 33 of 33 fixtures passed | `ea9885c40bb9f15d420e426d267b85561304dd97d81c53abd7bea2334be6117c` |
| `python3 scripts/check_test_catalog_staleness.py` | 269 committed records checked against 4,038 discovered test facts; passed | `29a6106a0f1cd346678f2b5d755ae33261b193e473cb2f90bbc95bd390d6a32f` |
| report mode | 15,229 facts, zero findings, zero blocking findings | `5b3b4260477ffe98844477cb0735830228514c8981faf19760b082d231356703` |
| full mode | 15,229 facts, zero findings, zero blocking findings | `4b7cb68778f0c26669d78cb3b46c87b9b477f0913840edcef002f8374a68b7de` |
| historical changed mode, baseline script and explicit implementation/base SHAs | 15,228 facts, 2,177 visible historical findings, zero changed-mode blockers; passed | `a9b231b961d0375f2bd69f8895e0593a35e0b9a8e26273d17e2d0b76f59517c7` |
| independent adversarial changed-mode fixtures | six tests passed: missing map, identity/signature drift, body drift, new/changed/overlap/invented facts, baseline append/field mutation, and removed catcher | `bc8c486a892d98d17884aa6ae5f726264a613a9c2a3f9f4eec0d6a25472d0161` |
| independent raw-row and semantic-reference audit | all 15,229 rows passed exact identity/digest, semantic reference, risk acknowledgement, gap affected-set, and inheritance checks | `1878e59c419efb7228c53702c65ee9ed004bec20eb72a2488885dabb6124b024` |

The report and full-mode raw facts are identical. At-rest artifact hashes are:

| Artifact | SHA-256 |
| --- | --- |
| report JSON | `d3a3971743c3a85d8f88c6176af862e60190e5c3bb9e315095068a1acdf6fb87` |
| report Markdown | `2f965e4535e95be514ebaaf012cbed03d924523b570b670c2abfc06dd9442db5` |
| full JSON | `5a8f6e9ad0c1b93e92044e912d5b136a9466c931f30a9ffea5beb523ce2085b7` |
| full Markdown | `b56497af9c300f77497fe2a11b6f4d338dbde877c204fbd45e4942f975fcd0bb` |

The broad `just fmt`, `just clippy`, and `just test-all` milestone was not run.
The protocol permits that milestone only after focused acceptance, and the
focused review found a closure blocker. Broad tests cannot waive the missing
specification and test-proof populations.

## Exact denominator

### Sources and facts

| Scoped language/source class | Source files | Facts |
| --- | ---: | ---: |
| Rust product | 1,059 | 9,722 |
| Rust verification tooling | 23 | 227 |
| TypeScript/JavaScript product | 196 | 2,256 TypeScript + 1,121 JavaScript |
| Python verification tooling | 259 | 1,822 |
| Shell verification tooling | 81 | 81 |
| **Total facts** |  | **15,229** |

### Disposition

| Disposition | Facts |
| --- | ---: |
| `spec_gap` | 15,138 |
| `specified` | 56 |
| `inherits_spec` | 30 |
| `generated` | 2 |
| `non_behavioral` | 2 |
| `vendored_external` | 1 |

### Owner and canonical area

| Owner | Facts | Area | Facts |
| --- | ---: | --- | ---: |
| `trust-runtime` | 6,852 | `editor_safety` | 3,305 |
| `editor` | 3,305 | `control_security` | 2,408 |
| `verification` | 2,109 | `verification` | 2,109 |
| `language` | 1,587 | `runtime_safety` | 1,670 |
| `hmi` | 765 | `compiler_iec` | 1,587 |
| `developer-tools` | 353 | `protocols` | 1,546 |
| `release-engineering` | 258 | `bytecode_vm` | 1,228 |
|  |  | `hmi_ui` | 765 |
|  |  | `plcopen_devtools` | 353 |
|  |  | `supply_chain_platform` | 237 |
|  |  | `release` | 21 |

### Risk signals

| Signal | Facts | Signal | Facts |
| --- | ---: | --- | ---: |
| `state_mutation` | 4,004 | `persistence_retain_reload_restart` | 207 |
| `fault_mapping` | 870 | `authorization` | 145 |
| `protocol_response_or_status` | 770 | `host_time` | 121 |
| `filesystem_or_environment` | 746 | `unsafe_or_ffi` | 87 |
| `broad_error_suppression` | 551 | `network_io` | 52 |
| `subprocess_or_thread` | 239 | `randomness` | 15 |
|  |  | `output_or_safe_state_commit` | 14 |

There are 5,763 risk-bearing facts and exactly 5,763 digest-bound risk
acknowledgement records. The independent audit found no missing, orphan,
duplicate, divergent, or expired acknowledgement.

### Reviewable area-candidate sets

All 15,229 facts resolve to a canonical area contained in their parser-derived
candidate set. There are zero unresolved routings. The 26 exact candidate-set
populations are:

| Candidate set | Facts | Candidate set | Facts |
| --- | ---: | --- | ---: |
| `editor_safety` | 2,855 | `control_security,editor_safety,runtime_safety` | 601 |
| `control_security` | 2,215 | `control_security,editor_safety,hmi_ui` | 566 |
| `verification` | 2,078 | `bytecode_vm,editor_safety,runtime_safety` | 360 |
| `compiler_iec` | 1,587 | `supply_chain_platform,verification` | 234 |
| `runtime_safety` | 1,581 | `control_security,hmi_ui` | 198 |
| `protocols` | 1,458 | `protocols,runtime_safety` | 88 |
| `bytecode_vm` | 925 | `control_security,supply_chain_platform` | 44 |
| `plcopen_devtools` | 350 | `bytecode_vm,control_security,supply_chain_platform` | 22 |
| `bytecode_vm,editor_safety,runtime_safety,verification` | 21 | `release,verification` | 21 |
| `bytecode_vm,runtime_safety` | 17 | `control_security,plcopen_devtools,supply_chain_platform` | 2 |
| `bytecode_vm,compiler_iec` | 1 | `compiler_iec,editor_safety` | 1 |
| `compiler_iec,hmi_ui` | 1 | `compiler_iec,plcopen_devtools` | 1 |
| `compiler_iec,runtime_safety` | 1 | `protocols,verification` | 1 |

The 18 multi-area candidate sets contain 2,180 facts. They are reviewable
ambiguities resolved by exact per-fact mapping, not unresolved routing debt.

## P17-006 pilot review

The complete live partition of
`crates/trust-runtime/src/stdlib/time.rs` contains exactly 88 scanner facts and
88 exact map records:

- 55 `specified`, 30 `inherits_spec`, and 3 `spec_gap` facts;
- 32 risk-bearing facts, each with one exact acknowledgement expiring
  2026-10-21;
- parser-derived `runtime_clock_value` and `current_dt` facts present;
- `current_dt`, `runtime_clock_value`, and `is_runtime_clock_name` bind exactly
  to open gap `SPEC_GAP_CURRENT_DT_CLOCK_BOUNDARY_001` and are named in its
  `affected_code_ids` set;
- all 30 registrations inherit the exact `register` parent and terminate in a
  reviewed specified chain;
- 47 function facts bind the time-arithmetic source, invariant, and executable
  catalog test; seven component facts bind the component source, same
  invariant, and split test; the registration parent binds both tests;
- no risk signal is hidden by `non_behavioral`, inheritance, or a broad
  suppression.

This accepts the pilot's honest classification, not the completeness of its
product semantics. `IEC_STANDARD_TIME_FUNCTIONS_001` itself records open
`boundary_low`, `boundary_high`, and `time_or_clock_fault` coverage cells and
missing overflow/invalid-component coverage. Those cells and the three
`CURRENT_DT` code gaps remain mandatory burn-down work.

## P17-012 mechanical audit and blockers

The exact partition mechanism passed:

- zero missing, duplicate, invented, stale, overlapping, or out-of-scope live
  identities;
- zero stale body digests and zero unresolved owner/area decisions;
- exact two-way specified-chain references, exact same-area gap affected sets,
  acyclic inheritance terminating in `specified` or `spec_gap`, and valid
  generated/vendored/non-behavioral boundaries;
- 63 authoritative gate inventory records with 30 exact gate-script joins;
- 17 fuzz inventory targets with 11 exact Cargo-fuzz joins and no unbound or
  duplicate executable entrypoint;
- report/full JSON and Markdown reconstruct with `proof_kind = "none"`;
- the existing verification workflow remains the sole code-spec CI route, and
  the release workflow dependency graph is unchanged;
- browser screenshots visibly render the HMI overview and IDE shell. Their
  recorded scope is asset consolidation only: the deeper IDE project-open
  journey received HTTP 403, and optional WASM asset retries were aborted.
  They are not evidence of complete HMI or IDE behavior.

The baseline lineage also passed its mechanical freeze contract:

| Commit | Entries | Mutation from parent |
| --- | ---: | --- |
| `5ecc4626b0305d2b6d833c82f7fe9be29dc13bbe` | 15,135 | created directly from the frozen implementation SHA |
| `5f522ea574ef5088ed88a90e19b1db2d92bc67ec` | 15,133 | two complete entries deleted |
| `84af65851f186a187c79ef6367edab5d45301915` | 0 | 15,133 complete entries deleted |
| `83b7a8549b7accd4d308df29ee7c8dacecb1b0bd` | 0 | no entry mutation; enforcement changed to full |

No baseline entry was appended or field-edited, and the current baseline is
empty. Converting the baseline entries into coarse historical gaps, however,
did not supply the expected behavior or executable proof that the final goal
requires.

### Blocking findings

1. **Historical code semantics and proof remain absent.** Exactly 15,138 code
   facts bind 13 gap records. Of these, 15,133 bind 11 area-wide historical
   backlog gaps, three bind the open `CURRENT_DT` clock-boundary gap, and two
   bind the private-hook contract gap. The largest historical populations are
   `editor_safety` 3,305, `control_security` 2,405, `verification` 2,106,
   `runtime_safety` 1,668, `protocols` 1,546, `compiler_iec` 1,499, and
   `bytecode_vm` 1,228. Exact registration is valuable audit evidence, but it
   is not an authoritative specification, accepted invariant, or executable
   oracle.
2. **Most discovered tests do not yet prove an accepted contract.** The test
   denominator contains 4,038 exact reviews: 261 `catalog_mapped` and 3,777
   `reviewed_nonmapping`. Of the latter, 3,687 say
   `no_reviewed_spec_or_invariant_binding`; 59 are gate-inventory-owned, 23 are
   ignored-test-registry-owned, and eight are fuzz-program-owned. The final
   objective requires each to bind to accepted proof or be removed with
   reviewed no-loss evidence.
3. **The accepted pilot deliberately leaves boundary proof open.** Its three
   clock-boundary code facts and the invariant's lower-bound, upper-bound,
   overflow/invalid-component, and clock-fault cells must be resolved without
   deriving an oracle from the current implementation.
4. **Phase sequencing must preserve these as mandatory debt.** Emptying the
   frozen baseline and enabling full scanner mode must not be interpreted as
   zero semantic or proof debt. The next board phase must make both populations
   mandatory burn-down work and prevent final stop-gate closure while either
   remains.

No product implementation should be inferred from this audit. Each unresolved
behavior needs an authoritative source decision first; only then may an
invariant and independent executable oracle be added.
